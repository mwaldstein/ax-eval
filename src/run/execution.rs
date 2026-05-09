use crate::adapter::{TokenUsage, ToolAdapter, ToolRunOutput};
use crate::evaluation::EvaluationMetrics;
use crate::fixture::TestEnv;
use crate::interaction_profile::AdapterEvidenceCapability;
use crate::run::artifacts::RunArtifacts;
use crate::scenario::Scenario;
use crate::transcript::TranscriptWriter;

pub struct EvaluationFlowInput<'a> {
    pub adapter: &'a dyn ToolAdapter,
    pub scenario: &'a Scenario,
    pub env: &'a TestEnv,
    pub tool: &'a str,
    pub model: &'a str,
    pub effective_timeout: u64,
    pub no_judge: bool,
    pub judge_model: Option<&'a str>,
    pub judge_tool: Option<&'a str>,
    pub writer: &'a TranscriptWriter,
    pub artifacts: &'a RunArtifacts,
}

pub struct EvaluationFlowResult {
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

pub fn create_adapter_and_check(tool: &str) -> anyhow::Result<Box<dyn ToolAdapter>> {
    use crate::adapter::{
        claude_code::ClaudeCodeAdapter, codex::CodexAdapter, mock::MockAdapter,
        opencode::OpenCodeAdapter,
    };
    let adapter: Box<dyn ToolAdapter> = match tool {
        "claude" | "claude-code" => Box::new(ClaudeCodeAdapter),
        "codex" => Box::new(CodexAdapter),
        "mock" => Box::new(MockAdapter),
        "opencode" => Box::new(OpenCodeAdapter),
        _ => anyhow::bail!("Unknown tool: {}", tool),
    };

    println!("Checking availability for tool: {}", tool);
    adapter.check_availability()?;

    Ok(adapter)
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
        }
    }
    // Also copy transcript to fixture directory for gate evaluators that read from env_root.
    input.artifacts.write_fixture_transcript(output);

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

fn run_post_scripts(
    scenario: &Scenario,
    tool: &str,
    model: &str,
    artifacts: &RunArtifacts,
    writer: &TranscriptWriter,
) -> anyhow::Result<()> {
    if let Some(scripts) = &scenario.scripts {
        println!("Running {} post-execution script(s)...", scripts.post.len());
        let runner = artifacts.script_runner(scenario, tool, model);

        for entry in &scripts.post {
            let report = runner.run_report(&entry.command, entry.timeout_secs)?;
            writer.append_event(&report.event("post_script"))?;

            if !report.succeeded() {
                eprintln!("Warning: post script failed: {}", entry.command);
            }
        }
    }

    Ok(())
}

pub fn run_evaluation_flow(input: EvaluationFlowInput<'_>) -> anyhow::Result<EvaluationFlowResult> {
    let start = std::time::Instant::now();
    println!(
        "Running tool '{}' with model '{}'...",
        input.tool, input.model
    );
    let run_output: ToolRunOutput = input.adapter.run(
        input.scenario,
        &input.env.root,
        Some(input.model),
        input.effective_timeout,
    )?;
    let duration = start.elapsed();
    let exit_code = run_output.exit_code;
    let cost = run_output.cost_usd;
    let token_usage = run_output.token_usage.clone();

    persist_execution_transcript(ExecutionTranscriptInput {
        writer: input.writer,
        artifacts: input.artifacts,
        tool: input.tool,
        run_output: &run_output,
    })?;

    // Run post-execution scripts after transcript writing, before evaluation
    run_post_scripts(
        input.scenario,
        input.tool,
        input.model,
        input.artifacts,
        input.writer,
    )?;

    // Create script runner for evaluation (used by script gates)
    let script_runner = input
        .artifacts
        .script_runner(input.scenario, input.tool, input.model);

    println!("Running evaluation...");
    let completed = exit_code == 0;
    let metrics = crate::evaluation::evaluate(
        input.scenario,
        &input.env.root,
        input.no_judge,
        Some(&script_runner),
        input.judge_model,
        input.judge_tool,
        &run_output.interaction_input,
        AdapterEvidenceCapability::from_supports_structured_tool_calls(
            input.adapter.supports_structured_tool_calls(),
        ),
        input.artifacts.fixture_transcript_path(),
        completed,
    )?;
    println!("Evaluation metrics: {:?}", metrics);

    Ok(EvaluationFlowResult {
        cost,
        token_usage,
        duration,
        metrics,
    })
}

pub fn determine_outcome(metrics: &EvaluationMetrics) -> String {
    if metrics.gates_passed < metrics.gates_total {
        format!(
            "Fail: {}/{} gates passed",
            metrics.gates_passed, metrics.gates_total
        )
    } else if let Some(judge_passed) = metrics.judge_passed {
        if !judge_passed {
            format!(
                "Fail: judge score {:.2} below threshold",
                metrics.judge_score.unwrap_or(0.0)
            )
        } else {
            "Pass".to_string()
        }
    } else {
        "Pass".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{CommandEvent, InteractionInput};

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
    }
}
