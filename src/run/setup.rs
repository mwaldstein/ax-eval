use crate::fixture::TestEnv;
use crate::run::artifacts::RunArtifacts;
use crate::scenario::{Scenario, Setup};
use crate::target_env::TargetEnvironment;
use crate::transcript::TranscriptWriter;
use std::path::Path;

pub struct ScenarioWorkspace {
    pub env: TestEnv,
    pub scenario_yaml: String,
    pub prompt: String,
}

impl ScenarioWorkspace {
    pub fn cache_key(&self, tool: &str, model: &str) -> anyhow::Result<crate::results::CacheKey> {
        crate::run::cache::compute_cache_key_with_fixture(
            &self.scenario_yaml,
            &self.prompt,
            &self.env.root,
            tool,
            model,
        )
    }
}

pub struct PreparedRunContext {
    pub workspace: ScenarioWorkspace,
    pub target_env: TargetEnvironment,
    pub artifacts: RunArtifacts,
}

impl PreparedRunContext {
    pub fn new(
        scenario: &Scenario,
        scenario_path: &std::path::Path,
        results_dir: &Path,
    ) -> anyhow::Result<Self> {
        let workspace = setup_scenario_env(scenario, scenario_path, results_dir)?;
        let target_env = TargetEnvironment::expanded_from_config(
            scenario.target.env.as_ref(),
            &workspace.env.root,
            results_dir,
        );
        let artifacts = RunArtifacts::new(results_dir, &workspace.env);

        Ok(Self {
            workspace,
            target_env,
            artifacts,
        })
    }

    pub fn cache_key(&self, tool: &str, model: &str) -> anyhow::Result<crate::results::CacheKey> {
        self.workspace.cache_key(tool, model)
    }
}

pub struct PreparedScenarioRun {
    pub artifacts: RunArtifacts,
    pub writer: TranscriptWriter,
    pub setup_success: bool,
    pub setup_commands: Vec<SetupCommandReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCommandReport {
    pub command: String,
    pub success: bool,
    pub output: String,
}

pub fn setup_scenario_env(
    s: &Scenario,
    scenario_path: &std::path::Path,
    results_dir: &Path,
) -> anyhow::Result<ScenarioWorkspace> {
    let scenario_yaml = std::fs::read_to_string(scenario_path)?;
    let prompt = s.task.prompt.clone();

    println!(
        "Setting up environment for template folder: {}",
        s.template_folder
    );
    let env_root = results_dir.join("fixture");
    let env = TestEnv::new(env_root)?;
    env.setup_fixture(&s.template_folder)?;

    println!("Environment created at: {:?}", env.root);

    Ok(ScenarioWorkspace {
        env,
        scenario_yaml,
        prompt,
    })
}

pub fn execute_setup_commands(
    setup: &Setup,
    env: &TestEnv,
    writer: &TranscriptWriter,
    effective_timeout: u64,
    target_env: &TargetEnvironment,
) -> anyhow::Result<(bool, Vec<SetupCommandReport>)> {
    println!("Running {} setup command(s)...", setup.commands.len());
    let mut setup_success = true;
    let mut setup_commands: Vec<SetupCommandReport> = Vec::new();

    for (i, cmd) in setup.commands.iter().enumerate() {
        println!("  Command {}/{}: {}", i + 1, setup.commands.len(), cmd);
        let result = crate::command_execution::run_shell_with_session(
            cmd,
            &env.root,
            effective_timeout,
            target_env.as_map(),
        )?;
        let output = result.output();
        let exit_code = result.exit_code;

        let success = exit_code == 0;
        setup_commands.push(SetupCommandReport {
            command: cmd.to_string(),
            success,
            output: output.clone(),
        });

        writer.append_event(&serde_json::json!({
            "type": "setup_command",
            "index": i,
            "command": cmd,
            "exit_code": exit_code,
            "output": output,
            "success": success,
        }))?;

        if !success {
            setup_success = false;
            println!("  Command failed with exit code {}", exit_code);
        }
    }
    println!("Setup complete.");

    Ok((setup_success, setup_commands))
}

pub fn execute_health_check(
    command: &str,
    env: &TestEnv,
    writer: &TranscriptWriter,
    effective_timeout: u64,
    target_env: &TargetEnvironment,
) -> anyhow::Result<()> {
    if command.trim().is_empty() {
        anyhow::bail!("target.health_check cannot be empty");
    }

    println!("Running target health check: {}", command);
    let result = crate::command_execution::run_shell_with_session(
        command,
        &env.root,
        effective_timeout,
        target_env.as_map(),
    )?;
    let output = result.output();
    let exit_code = result.exit_code;
    let success = exit_code == 0;

    writer.append_event(&serde_json::json!({
        "type": "health_check",
        "command": command,
        "exit_code": exit_code,
        "output": output,
        "success": success,
    }))?;

    if !success {
        anyhow::bail!(
            "target health check failed with exit code {}: {}",
            exit_code,
            output.trim()
        );
    }

    Ok(())
}

pub fn prepare_writer_and_setup(
    context: &PreparedRunContext,
    s: &Scenario,
    effective_timeout: u64,
) -> anyhow::Result<PreparedScenarioRun> {
    let artifacts = context.artifacts.clone();
    let writer = artifacts.writer()?;

    let (setup_success, setup_commands) = if let Some(setup) = &s.setup {
        execute_setup_commands(
            setup,
            &context.workspace.env,
            &writer,
            effective_timeout,
            &context.target_env,
        )?
    } else {
        (true, vec![])
    };

    if let Some(health_check) = &s.target.health_check {
        execute_health_check(
            health_check,
            &context.workspace.env,
            &writer,
            effective_timeout,
            &context.target_env,
        )?;
    }

    Ok(PreparedScenarioRun {
        artifacts,
        writer,
        setup_success,
        setup_commands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Setup;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn setup_commands_receive_target_env_vars() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");

        let artifacts_dir = dir.path().join("artifacts");
        let results_dir = dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");
        let writer =
            TranscriptWriter::new(artifacts_dir, results_dir.clone()).expect("create writer");

        let setup = Setup {
            commands: vec!["test \"$TARGET_ENV_TEST\" = \"works\"".to_string()],
        };
        let mut target_env = HashMap::new();
        target_env.insert("TARGET_ENV_TEST".to_string(), "works".to_string());

        let target_env =
            TargetEnvironment::expanded_from_config(Some(&target_env), &env.root, &results_dir);

        let (setup_success, commands) =
            execute_setup_commands(&setup, &env, &writer, 10, &target_env)
                .expect("run setup commands");

        assert!(setup_success);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "test \"$TARGET_ENV_TEST\" = \"works\"");
        assert!(commands[0].success);
        assert!(commands[0].output.is_empty());
    }

    #[test]
    fn target_env_expands_fixture_and_results_placeholders() {
        let dir = tempdir().expect("create temp dir");
        let fixture_dir = dir.path().join("results").join("fixture");
        let results_dir = dir.path().join("results");

        let mut target_env = HashMap::new();
        target_env.insert(
            "MYTOOL_ROOT_DIR".to_string(),
            "${LLM_TOOL_TEST_FIXTURE_DIR}".to_string(),
        );
        target_env.insert(
            "MYTOOL_EXPORT".to_string(),
            "${LLM_TOOL_TEST_RESULTS_DIR}/export.json".to_string(),
        );

        let expanded =
            TargetEnvironment::expanded_from_config(Some(&target_env), &fixture_dir, &results_dir);

        assert_eq!(
            expanded.as_map().get("MYTOOL_ROOT_DIR"),
            Some(&fixture_dir.to_string_lossy().to_string())
        );
        assert_eq!(
            expanded.as_map().get("MYTOOL_EXPORT"),
            Some(&format!("{}/export.json", results_dir.to_string_lossy()))
        );
    }

    #[test]
    fn setup_commands_receive_expanded_fixture_dir_env_var() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");

        let artifacts_dir = dir.path().join("artifacts");
        let results_dir = dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");
        let writer = TranscriptWriter::new(artifacts_dir, results_dir.clone()).expect("writer");

        let setup = Setup {
            commands: vec!["test \"$MYTOOL_ROOT_DIR\" = \"$PWD\"".to_string()],
        };
        let mut target_env = HashMap::new();
        target_env.insert(
            "MYTOOL_ROOT_DIR".to_string(),
            "${LLM_TOOL_TEST_FIXTURE_DIR}".to_string(),
        );
        let expanded =
            TargetEnvironment::expanded_from_config(Some(&target_env), &env.root, &results_dir);

        let (success, reports) = execute_setup_commands(&setup, &env, &writer, 10, &expanded)
            .expect("run setup commands");

        assert!(success);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].success);
    }

    #[test]
    fn scenario_workspace_cache_key_includes_workspace_inputs() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");
        std::fs::write(env.root.join("AGENTS.md"), "first").expect("write fixture file");

        let workspace = ScenarioWorkspace {
            env,
            scenario_yaml: "name: cache-key-test".to_string(),
            prompt: "Create a note".to_string(),
        };

        let first = workspace.cache_key("mock", "model-a").expect("cache key");
        let same = workspace.cache_key("mock", "model-a").expect("cache key");
        let different_tool = workspace.cache_key("other", "model-a").expect("cache key");
        let different_model = workspace.cache_key("mock", "model-b").expect("cache key");

        std::fs::write(workspace.env.root.join("AGENTS.md"), "second")
            .expect("update fixture file");
        let different_fixture = workspace.cache_key("mock", "model-a").expect("cache key");

        assert_eq!(first, same);
        assert_ne!(first, different_tool);
        assert_ne!(first, different_model);
        assert_ne!(first, different_fixture);
    }

    #[test]
    fn prepared_run_context_owns_workspace_artifacts_and_expanded_target_env() {
        let dir = tempdir().expect("create temp dir");
        let scenario_path = dir.path().join("scenario.yaml");
        std::fs::write(
            &scenario_path,
            r#"
name: prepared-context-test
description: Test prepared run context
template_folder: example_basic
target:
  binary: taskmgr
  env:
    TARGET_ROOT: "${LLM_TOOL_TEST_FIXTURE_DIR}"
    TARGET_EXPORT: "${LLM_TOOL_TEST_RESULTS_DIR}/export.json"
task:
  prompt: "Create a task"
evaluation:
  gates: []
"#,
        )
        .expect("write scenario");
        let scenario: Scenario =
            yaml_serde::from_str(&std::fs::read_to_string(&scenario_path).expect("read scenario"))
                .expect("parse scenario");
        let results_dir = dir.path().join("results");

        let context =
            PreparedRunContext::new(&scenario, &scenario_path, &results_dir).expect("context");

        assert_eq!(context.workspace.env.root, results_dir.join("fixture"));
        assert_eq!(
            context.artifacts.artifacts_dir(),
            results_dir.join("artifacts")
        );
        assert_eq!(
            context.target_env.as_map().get("TARGET_ROOT"),
            Some(&context.workspace.env.root.to_string_lossy().to_string())
        );
        assert_eq!(
            context.target_env.as_map().get("TARGET_EXPORT"),
            Some(&format!("{}/export.json", results_dir.to_string_lossy()))
        );
    }

    #[test]
    fn prepared_run_context_expands_target_env_paths_to_absolute_when_results_dir_is_relative() {
        let dir = tempdir().expect("create temp dir");
        let scenario_path = dir.path().join("scenario.yaml");
        std::fs::write(
            &scenario_path,
            r#"
name: prepared-context-relative-results-test
description: Test target env path expansion with relative results dir
template_folder: example_basic
target:
  binary: taskmgr
  env:
    TARGET_ROOT: "${LLM_TOOL_TEST_FIXTURE_DIR}"
    TARGET_EXPORT: "${LLM_TOOL_TEST_RESULTS_DIR}/export.json"
task:
  prompt: "Create a task"
evaluation:
  gates: []
"#,
        )
        .expect("write scenario");
        let scenario: Scenario =
            yaml_serde::from_str(&std::fs::read_to_string(&scenario_path).expect("read scenario"))
                .expect("parse scenario");
        let relative_results_dir = std::path::PathBuf::from(format!(
            "target/prepared-context-relative-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        let context = PreparedRunContext::new(&scenario, &scenario_path, &relative_results_dir)
            .expect("context");
        let expected_fixture = std::env::current_dir()
            .expect("current dir")
            .join(relative_results_dir.join("fixture"));
        let expected_results = std::env::current_dir()
            .expect("current dir")
            .join(&relative_results_dir);

        assert_eq!(
            context.target_env.as_map().get("TARGET_ROOT"),
            Some(&expected_fixture.to_string_lossy().to_string())
        );
        assert_eq!(
            context.target_env.as_map().get("TARGET_EXPORT"),
            Some(&format!(
                "{}/export.json",
                expected_results.to_string_lossy()
            ))
        );
    }

    #[test]
    fn health_check_runs_in_fixture_dir_with_target_env_vars() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");

        let artifacts_dir = dir.path().join("artifacts");
        let results_dir = dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");
        let writer =
            TranscriptWriter::new(artifacts_dir, results_dir.clone()).expect("create writer");

        let mut target_env = HashMap::new();
        target_env.insert("TARGET_ENV_TEST".to_string(), "works".to_string());

        execute_health_check(
            "test \"$TARGET_ENV_TEST\" = \"works\"",
            &env,
            &writer,
            10,
            &TargetEnvironment::expanded_from_config(Some(&target_env), &env.root, &results_dir),
        )
        .expect("run health check");
    }

    #[test]
    fn health_check_fails_on_nonzero_exit() {
        let dir = tempdir().expect("create temp dir");
        let env = TestEnv::new(dir.path().join("fixture")).expect("create test env");
        std::fs::create_dir_all(&env.root).expect("create fixture root");

        let artifacts_dir = dir.path().join("artifacts");
        let results_dir = dir.path().join("results");
        std::fs::create_dir_all(&results_dir).expect("create results dir");
        let writer = TranscriptWriter::new(artifacts_dir, results_dir).expect("create writer");

        let result =
            execute_health_check("exit 7", &env, &writer, 10, &TargetEnvironment::default());

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("target health check failed"));
    }
}
