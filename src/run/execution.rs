use crate::adapter::{TokenUsage, ToolAdapter, ToolRunOutput};
use crate::evaluation::{EvaluationInput, EvaluationMetrics};
use crate::interaction_profile::AdapterEvidenceCapability;
use crate::mcp_auth::preflight_auth;
use crate::run::artifacts::RunArtifacts;
use crate::run::setup::{PreparedRunContext, PreparedScenarioRun};
use crate::run::status;
use crate::scenario::Scenario;
use crate::target_env::TargetEnvironment;
use crate::transcript::redact_sensitive;
use crate::transcript::TranscriptWriter;
use std::path::Path;

pub struct RunAttemptInput<'a> {
    pub adapter: &'a dyn ToolAdapter,
    pub scenario: &'a Scenario,
    pub scenario_path: &'a Path,
    pub context: &'a PreparedRunContext,
    pub prepared: &'a PreparedScenarioRun,
    pub tool: &'a str,
    pub model: &'a str,
    pub effective_timeout: u64,
    pub no_judge: bool,
    pub judge_model: Option<&'a str>,
    pub judge_tool: Option<&'a str>,
}

pub struct RunAttemptResult {
    pub cost: Option<f64>,
    pub token_usage: Option<TokenUsage>,
    pub duration: std::time::Duration,
    pub metrics: EvaluationMetrics,
}

struct ExecutionTranscriptInput<'a> {
    writer: &'a TranscriptWriter,
    artifacts: &'a RunArtifacts,
    tool: &'a str,
    run_output: &'a ToolRunOutput,
}

struct PostScriptRunInput<'a> {
    scenario: &'a Scenario,
    tool: &'a str,
    model: &'a str,
    artifacts: &'a RunArtifacts,
    writer: &'a TranscriptWriter,
    target_env: &'a TargetEnvironment,
}

fn persist_execution_transcript(input: ExecutionTranscriptInput<'_>) -> anyhow::Result<()> {
    let output = &input.run_output.transcript;

    // Write transcript immediately after execution so evaluation can read it.
    input.writer.write_raw(output)?;
    if let Some(raw_output) = &input.run_output.raw_output {
        input.writer.write_tool_output(raw_output)?;
    }
    if let Some(command_events) = input.run_output.command_events() {
        if !command_events.is_empty() {
            input.writer.write_command_events(command_events)?;
            input
                .writer
                .write_interaction_events(&input.run_output.interaction_input)?;
        }
    }
    if let Some(mcp_events) = input.run_output.mcp_tool_call_events() {
        if !mcp_events.is_empty() {
            input.writer.write_mcp_events(mcp_events)?;
            input
                .writer
                .write_interaction_events(&input.run_output.interaction_input)?;
        }
    }
    // Also copy transcript to fixture directory for gate evaluators that read from env_root.
    input.artifacts.write_fixture_transcript(output)?;

    let event = if let Some(c) = input.run_output.cost_usd {
        serde_json::json!({
            "type": "execution",
            "tool": input.tool,
            "output": output,
            "exit_code": input.run_output.exit_code,
            "cost_usd": c
        })
    } else {
        serde_json::json!({
            "type": "execution",
            "tool": input.tool,
            "output": output,
            "exit_code": input.run_output.exit_code
        })
    };
    input.writer.append_event(&event)?;

    Ok(())
}

fn run_post_scripts(input: PostScriptRunInput<'_>) -> anyhow::Result<()> {
    if let Some(scripts) = &input.scenario.scripts {
        println!("Running {} post-execution script(s)...", scripts.post.len());
        let runner = input.artifacts.script_runner(
            input.scenario,
            input.tool,
            input.model,
            input.target_env,
        );

        for entry in &scripts.post {
            let report = runner.run_report(&entry.command, entry.timeout_secs)?;
            input.writer.append_event(&report.event("post_script"))?;

            if !report.succeeded() {
                eprintln!("Warning: post script failed: {}", entry.command);
            }
        }
    }

    Ok(())
}

fn redact_retained_mcp_configs(
    workspace: &Path,
    target: &crate::scenario::TargetConfig,
) -> anyhow::Result<()> {
    // opencode and claude-code must read resolved static headers from workspace
    // files during the live run. The workspace is retained under results/fixture,
    // so scrub those inspectable copies immediately after the agent exits.
    //
    // Two passes: pattern-based redaction for well-known credential shapes, then
    // exact-value redaction of the secrets ax-eval itself resolved — the latter
    // catches custom-named auth headers no pattern would recognize.
    let secrets = crate::mcp_auth::resolved_auth_secrets(target);
    for path in [
        workspace.join(".mcp.json"),
        workspace.join(".opencode_config").join("opencode.json"),
    ] {
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        let mut redacted = redact_sensitive(&content);
        for secret in &secrets {
            redacted = redacted.replace(secret.as_str(), "[REDACTED]");
        }
        if redacted != content {
            std::fs::write(path, redacted)?;
        }
    }

    Ok(())
}

pub fn run_attempt(input: RunAttemptInput<'_>) -> anyhow::Result<RunAttemptResult> {
    let start = std::time::Instant::now();
    println!(
        "Running tool '{}' with model '{}'...",
        input.tool, input.model
    );
    preflight_auth(&input.scenario.target)?;
    let provision = input
        .adapter
        .provision_target(&input.scenario.target, &input.context.workspace.env.root)?;
    let run_result = input.adapter.run(
        input.scenario,
        &input.context.workspace.env.root,
        Some(input.model),
        input.effective_timeout,
        &input.context.target_env,
    );
    let cleanup_result = provision.cleanup();
    let scrub_result =
        redact_retained_mcp_configs(&input.context.workspace.env.root, &input.scenario.target);
    // A cleanup failure after a successful (paid) agent run must not discard
    // the run's evidence; warn and continue so results are persisted.
    let run_output: ToolRunOutput = match (run_result, cleanup_result, scrub_result) {
        (Ok(run_output), Ok(()), Ok(())) => run_output,
        (Err(run_error), Ok(()), Ok(())) => return Err(run_error),
        (Ok(run_output), Err(cleanup_error), _) => {
            eprintln!(
                "warning: target cleanup failed after a completed run; \
                 host config may need manual restoration: {cleanup_error:#}"
            );
            run_output
        }
        (Err(run_error), Err(cleanup_error), _) => {
            return Err(run_error.context(format!("target cleanup also failed: {cleanup_error:#}")))
        }
        (Err(run_error), Ok(()), Err(scrub_error)) => {
            return Err(run_error.context(format!(
                "retained MCP config redaction also failed: {scrub_error:#}"
            )))
        }
        (Ok(run_output), Ok(()), Err(scrub_error)) => {
            eprintln!(
                "warning: retained MCP config redaction failed after a completed run; \
                 results may need manual review before sharing: {scrub_error:#}"
            );
            run_output
        }
    };
    let duration = start.elapsed();
    let exit_code = run_output.exit_code;
    let cost = run_output.cost_usd;
    let token_usage = run_output.token_usage.clone();

    persist_execution_transcript(ExecutionTranscriptInput {
        writer: &input.prepared.writer,
        artifacts: &input.prepared.artifacts,
        tool: input.tool,
        run_output: &run_output,
    })?;

    // Run post-execution scripts after transcript writing, before evaluation
    run_post_scripts(PostScriptRunInput {
        scenario: input.scenario,
        tool: input.tool,
        model: input.model,
        artifacts: &input.prepared.artifacts,
        writer: &input.prepared.writer,
        target_env: &input.context.target_env,
    })?;

    // Create script runner for evaluation (used by script gates)
    let script_runner = input.prepared.artifacts.script_runner(
        input.scenario,
        input.tool,
        input.model,
        &input.context.target_env,
    );

    println!("Running evaluation...");
    let completed = exit_code == 0;
    let metrics = crate::evaluation::evaluate(EvaluationInput {
        scenario: input.scenario,
        env_root: &input.context.workspace.env.root,
        scenario_path: input.scenario_path,
        no_judge: input.no_judge,
        script_runner: Some(&script_runner),
        judge_model: input.judge_model,
        judge_tool: input.judge_tool,
        interaction_input: &run_output.interaction_input,
        adapter_capability: AdapterEvidenceCapability::from_supports_structured_tool_calls(
            input.adapter.supports_structured_tool_calls(),
        ),
        transcript_path: input.prepared.artifacts.fixture_transcript_path(),
        completed,
        target_env: &input.context.target_env,
    })?;
    println!(
        "Evaluation profile built: {} commands, {:.0}% first-try success, {} errors",
        metrics.efficiency.total_commands,
        metrics.efficiency.first_try_success_rate * 100.0,
        metrics.efficiency.error_count
    );

    Ok(RunAttemptResult {
        cost,
        token_usage,
        duration,
        metrics,
    })
}

pub fn determine_outcome(metrics: &EvaluationMetrics) -> String {
    status::determine_outcome(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{AdapterError, TargetProvision, ToolStatus};
    use crate::fixture::TestEnv;
    use crate::interaction_evidence::{CommandEvent, InteractionInput, McpToolCallEvent};
    use crate::scenario::{
        Evaluation, McpAuth, McpTarget, McpTransport, Scenario, ScriptEntry, ScriptsConfig,
        TargetConfig, Task,
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn scenario_with_scripts(scripts: Option<ScriptsConfig>) -> Scenario {
        Scenario {
            name: "execution-test".to_string(),
            description: "Test execution behavior".to_string(),
            template_folder: "fixture".to_string(),
            target: TargetConfig::cli_target("target"),
            task: Task {
                prompt: "Do the task".to_string(),
            },
            evaluation: Evaluation {
                gates: vec![],
                judge: None,
                composite: None,
            },
            tier: 0,
            tool_matrix: None,
            setup: None,
            tags: vec![],
            run: None,
            scripts,
            interaction: Default::default(),
        }
    }

    fn mcp_scenario_with_auth(auth: McpAuth) -> Scenario {
        Scenario {
            name: "mcp-auth-test".to_string(),
            description: "Test MCP auth behavior".to_string(),
            template_folder: "fixture".to_string(),
            target: TargetConfig::Mcp(McpTarget {
                name: "search".to_string(),
                transport: McpTransport::Http {
                    url: "https://mcp.example.com/mcp".to_string(),
                    headers: None,
                },
                auth: Some(auth),
                tools: vec!["query".to_string()],
                env: None,
                health_check: None,
            }),
            task: Task {
                prompt: "Do the task".to_string(),
            },
            evaluation: Evaluation {
                gates: vec![],
                judge: None,
                composite: None,
            },
            tier: 0,
            tool_matrix: None,
            setup: None,
            tags: vec![],
            run: None,
            scripts: None,
            interaction: Default::default(),
        }
    }

    struct FailingProvisionedAdapter {
        cleaned: Arc<AtomicBool>,
    }

    impl ToolAdapter for FailingProvisionedAdapter {
        fn is_available(&self) -> Result<ToolStatus, AdapterError> {
            Ok(ToolStatus {
                available: true,
                authenticated: true,
            })
        }

        fn provision_target(
            &self,
            _target: &TargetConfig,
            _workspace: &Path,
        ) -> anyhow::Result<TargetProvision> {
            let cleaned = Arc::clone(&self.cleaned);
            Ok(TargetProvision::with_cleanup(move || {
                cleaned.store(true, Ordering::SeqCst);
                Ok(())
            }))
        }

        fn run(
            &self,
            _scenario: &Scenario,
            _cwd: &Path,
            _model: Option<&str>,
            _timeout_secs: u64,
            _target_env: &TargetEnvironment,
        ) -> anyhow::Result<ToolRunOutput> {
            anyhow::bail!("agent run failed")
        }
    }

    #[test]
    fn run_attempt_cleans_up_when_adapter_run_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("test env");
        std::fs::create_dir_all(&env.root).expect("fixture dir");
        let results_dir = dir.path().join("results");
        let artifacts = RunArtifacts::new(&results_dir, &env);
        let writer = artifacts.writer().expect("writer");
        let context = PreparedRunContext {
            workspace: crate::run::setup::ScenarioWorkspace {
                env,
                scenario_yaml: "scenario yaml".to_string(),
                prompt: "prompt".to_string(),
            },
            target_env: TargetEnvironment::default(),
            artifacts: artifacts.clone(),
        };
        let prepared = PreparedScenarioRun {
            artifacts,
            writer,
            setup_success: true,
            setup_commands: vec![],
        };
        let scenario = scenario_with_scripts(None);
        let cleaned = Arc::new(AtomicBool::new(false));
        let adapter = FailingProvisionedAdapter {
            cleaned: Arc::clone(&cleaned),
        };

        let result = run_attempt(RunAttemptInput {
            adapter: &adapter,
            scenario: &scenario,
            scenario_path: Path::new("scenario.yaml"),
            context: &context,
            prepared: &prepared,
            tool: "failing",
            model: "model",
            effective_timeout: 1,
            no_judge: true,
            judge_model: None,
            judge_tool: None,
        });

        let error = match result {
            Ok(_) => panic!("run should fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("agent run failed"));
        assert!(cleaned.load(Ordering::SeqCst));
    }

    struct SecretWritingAdapter {
        env_name: String,
        ran: Arc<AtomicBool>,
    }

    impl ToolAdapter for SecretWritingAdapter {
        fn is_available(&self) -> Result<ToolStatus, AdapterError> {
            Ok(ToolStatus {
                available: true,
                authenticated: true,
            })
        }

        fn provision_target(
            &self,
            _target: &TargetConfig,
            workspace: &Path,
        ) -> anyhow::Result<TargetProvision> {
            let token = std::env::var(&self.env_name)?;
            std::fs::write(
                workspace.join(".mcp.json"),
                format!(
                    r#"{{
  "mcpServers": {{
    "search": {{
      "type": "http",
      "url": "https://mcp.example.com/mcp",
      "headers": {{
        "Authorization": "Bearer {token}"
      }}
    }}
  }}
}}
"#
                ),
            )?;
            Ok(TargetProvision::none())
        }

        fn run(
            &self,
            _scenario: &Scenario,
            _cwd: &Path,
            _model: Option<&str>,
            _timeout_secs: u64,
            _target_env: &TargetEnvironment,
        ) -> anyhow::Result<ToolRunOutput> {
            self.ran.store(true, Ordering::SeqCst);
            Ok(ToolRunOutput {
                transcript: "done".to_string(),
                raw_output: None,
                exit_code: 0,
                cost_usd: None,
                token_usage: None,
                interaction_input: InteractionInput::TranscriptRegex,
            })
        }
    }

    #[test]
    fn run_attempt_redacts_retained_mcp_config_after_live_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("results").join("fixture")).expect("test env");
        std::fs::create_dir_all(&env.root).expect("fixture dir");
        let results_dir = dir.path().join("results");
        let artifacts = RunArtifacts::new(&results_dir, &env);
        let writer = artifacts.writer().expect("writer");
        let context = PreparedRunContext {
            workspace: crate::run::setup::ScenarioWorkspace {
                env,
                scenario_yaml: "scenario yaml".to_string(),
                prompt: "prompt".to_string(),
            },
            target_env: TargetEnvironment::default(),
            artifacts: artifacts.clone(),
        };
        let prepared = PreparedScenarioRun {
            artifacts,
            writer,
            setup_success: true,
            setup_commands: vec![],
        };
        let env_name = "AX_EVAL_RUN_SECRET_RETAINED_ARTIFACT_UNIQUE";
        let token = "retained-artifact-secret-token";
        std::env::set_var(env_name, token);
        let scenario = mcp_scenario_with_auth(McpAuth::BearerEnv {
            env: env_name.to_string(),
        });
        let ran = Arc::new(AtomicBool::new(false));
        let adapter = SecretWritingAdapter {
            env_name: env_name.to_string(),
            ran: Arc::clone(&ran),
        };

        run_attempt(RunAttemptInput {
            adapter: &adapter,
            scenario: &scenario,
            scenario_path: Path::new("scenario.yaml"),
            context: &context,
            prepared: &prepared,
            tool: "secret-writer",
            model: "model",
            effective_timeout: 1,
            no_judge: true,
            judge_model: None,
            judge_tool: None,
        })
        .expect("run attempt");

        assert!(ran.load(Ordering::SeqCst));
        let retained =
            std::fs::read_to_string(context.workspace.env.root.join(".mcp.json")).expect("config");
        assert!(!retained.contains(token));
        assert!(retained.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn redact_retained_scrubs_custom_header_secret_by_exact_value() {
        // A headers-mode secret under a custom header name matches none of the
        // pattern-based redaction rules; exact-value redaction must still scrub it.
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path();
        let env_name = "AX_EVAL_CUSTOM_HEADER_SECRET_UNIQUE";
        let token = "opaque-custom-header-secret-9f8e7d6c5b4a";
        std::env::set_var(env_name, token);

        let config = format!(
            r#"{{"mcpServers":{{"s":{{"type":"http","url":"https://x","headers":{{"X-Weird-Auth":"{token}"}}}}}}}}"#
        );
        std::fs::write(workspace.join(".mcp.json"), &config).expect("write config");

        let target = TargetConfig::Mcp(McpTarget {
            name: "s".to_string(),
            transport: McpTransport::Http {
                url: "https://x".to_string(),
                headers: None,
            },
            auth: Some(McpAuth::Headers {
                headers: std::collections::HashMap::from([(
                    "X-Weird-Auth".to_string(),
                    format!("${{env:{env_name}}}"),
                )]),
            }),
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        redact_retained_mcp_configs(workspace, &target).expect("scrub");
        std::env::remove_var(env_name);

        let retained = std::fs::read_to_string(workspace.join(".mcp.json")).expect("read retained");
        assert!(
            !retained.contains(token),
            "custom-header secret must be scrubbed: {retained}"
        );
        assert!(retained.contains("[REDACTED]"));
    }

    #[test]
    fn run_attempt_fails_static_auth_preflight_before_adapter_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("results").join("fixture")).expect("test env");
        std::fs::create_dir_all(&env.root).expect("fixture dir");
        let results_dir = dir.path().join("results");
        let artifacts = RunArtifacts::new(&results_dir, &env);
        let writer = artifacts.writer().expect("writer");
        let context = PreparedRunContext {
            workspace: crate::run::setup::ScenarioWorkspace {
                env,
                scenario_yaml: "scenario yaml".to_string(),
                prompt: "prompt".to_string(),
            },
            target_env: TargetEnvironment::default(),
            artifacts: artifacts.clone(),
        };
        let prepared = PreparedScenarioRun {
            artifacts,
            writer,
            setup_success: true,
            setup_commands: vec![],
        };
        let env_name = "AX_EVAL_RUN_PREFLIGHT_UNSET_UNIQUE";
        std::env::remove_var(env_name);
        let scenario = mcp_scenario_with_auth(McpAuth::BearerEnv {
            env: env_name.to_string(),
        });
        let ran = Arc::new(AtomicBool::new(false));
        let adapter = SecretWritingAdapter {
            env_name: env_name.to_string(),
            ran: Arc::clone(&ran),
        };

        let err = match run_attempt(RunAttemptInput {
            adapter: &adapter,
            scenario: &scenario,
            scenario_path: Path::new("scenario.yaml"),
            context: &context,
            prepared: &prepared,
            tool: "secret-writer",
            model: "model",
            effective_timeout: 1,
            no_judge: true,
            judge_model: None,
            judge_tool: None,
        }) {
            Ok(_) => panic!("preflight should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains(env_name));
        assert!(!ran.load(Ordering::SeqCst));
        assert!(!context.workspace.env.root.join(".mcp.json").exists());
    }

    #[test]
    fn execution_transcript_input_persists_tool_output_artifacts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("test env");
        std::fs::create_dir_all(&env.root).expect("fixture dir");
        let artifacts = RunArtifacts::new(&dir.path().join("results"), &env);
        let writer = artifacts.writer().expect("writer");
        let run_output = ToolRunOutput {
            transcript: "agent transcript".to_string(),
            raw_output: Some("raw tool output".to_string()),
            exit_code: 0,
            cost_usd: Some(0.25),
            token_usage: None,
            interaction_input: InteractionInput::StructuredToolCalls(vec![CommandEvent {
                command: "target --version".to_string(),
                exit_code: Some(0),
            }]),
        };

        persist_execution_transcript(ExecutionTranscriptInput {
            writer: &writer,
            artifacts: &artifacts,
            tool: "mock",
            run_output: &run_output,
        })
        .expect("persist execution transcript");

        assert_eq!(
            std::fs::read_to_string(artifacts.transcript_path()).expect("artifact transcript"),
            "agent transcript"
        );
        assert_eq!(
            std::fs::read_to_string(artifacts.fixture_transcript_path())
                .expect("fixture transcript"),
            "agent transcript"
        );
        assert_eq!(
            std::fs::read_to_string(artifacts.artifacts_dir().join("tool-output.raw.txt"))
                .expect("raw output"),
            "raw tool output"
        );

        let events = writer.read_events().expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "execution");
        assert_eq!(events[0]["tool"], "mock");
        assert_eq!(events[0]["exit_code"], 0);
        assert_eq!(events[0]["cost_usd"], 0.25);

        assert!(artifacts
            .artifacts_dir()
            .join("interaction-events.json")
            .exists());
        assert!(artifacts
            .artifacts_dir()
            .join("command-events.json")
            .exists());
    }

    #[test]
    fn execution_transcript_input_persists_mcp_events() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("test env");
        std::fs::create_dir_all(&env.root).expect("fixture dir");
        let artifacts = RunArtifacts::new(&dir.path().join("results"), &env);
        let writer = artifacts.writer().expect("writer");
        let run_output = ToolRunOutput {
            transcript: "agent transcript".to_string(),
            raw_output: None,
            exit_code: 0,
            cost_usd: None,
            token_usage: None,
            interaction_input: InteractionInput::StructuredMcpToolCalls(vec![McpToolCallEvent {
                server: "todo".to_string(),
                tool: "add".to_string(),
                arguments: serde_json::json!({"text":"hello"}),
                is_error: false,
                duration_ms: Some(15),
            }]),
        };

        persist_execution_transcript(ExecutionTranscriptInput {
            writer: &writer,
            artifacts: &artifacts,
            tool: "mock",
            run_output: &run_output,
        })
        .expect("persist execution transcript");

        assert!(artifacts
            .artifacts_dir()
            .join("interaction-events.json")
            .exists());
        let mcp_events = std::fs::read_to_string(artifacts.artifacts_dir().join("mcp-events.json"))
            .expect("mcp events");
        assert!(mcp_events.contains("\"server\": \"todo\""));
        assert!(mcp_events.contains("\"tool\": \"add\""));
    }

    #[test]
    fn execution_transcript_input_reports_fixture_transcript_write_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("test env");
        std::fs::create_dir_all(&env.root).expect("fixture dir");
        std::fs::create_dir_all(env.root.join("transcript.raw.txt"))
            .expect("blocking transcript directory");
        let artifacts = RunArtifacts::new(&dir.path().join("results"), &env);
        let writer = artifacts.writer().expect("writer");
        let run_output = ToolRunOutput {
            transcript: "agent transcript".to_string(),
            raw_output: None,
            exit_code: 0,
            cost_usd: None,
            token_usage: None,
            interaction_input: InteractionInput::TranscriptRegex,
        };

        let result = persist_execution_transcript(ExecutionTranscriptInput {
            writer: &writer,
            artifacts: &artifacts,
            tool: "mock",
            run_output: &run_output,
        });

        assert!(result.is_err());
    }

    #[test]
    fn post_script_input_records_post_script_events() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("test env");
        std::fs::create_dir_all(&env.root).expect("fixture dir");
        let artifacts = RunArtifacts::new(&dir.path().join("results"), &env);
        let writer = artifacts.writer().expect("writer");
        let scenario = scenario_with_scripts(Some(ScriptsConfig {
            post: vec![ScriptEntry {
                command: "echo post-ok".to_string(),
                timeout_secs: 10,
            }],
            evaluators: vec![],
        }));

        run_post_scripts(PostScriptRunInput {
            scenario: &scenario,
            tool: "mock",
            model: "model",
            artifacts: &artifacts,
            writer: &writer,
            target_env: &TargetEnvironment::default(),
        })
        .expect("run post scripts");

        let events = writer.read_events().expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "post_script");
        assert_eq!(events[0]["command"], "echo post-ok");
        assert_eq!(events[0]["exit_code"], 0);
        assert!(events[0]["stdout"]
            .as_str()
            .expect("stdout")
            .contains("post-ok"));
    }
}
