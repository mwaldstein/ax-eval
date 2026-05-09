use crate::adapter::{TokenUsage, ToolAdapter, ToolRunOutput};
use crate::evaluation::EvaluationMetrics;
use crate::fixture::TestEnv;
use crate::scenario::Scenario;
use crate::script_runner::ScriptRunner;
use crate::transcript::TranscriptWriter;
use std::path::Path;

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
    pub transcript_dir: &'a Path,
    pub results_dir: &'a Path,
}

pub struct EvaluationFlowResult {
    pub cost: Option<f64>,
    pub token_usage: Option<TokenUsage>,
    pub duration: std::time::Duration,
    pub metrics: EvaluationMetrics,
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

fn run_post_scripts(
    scenario: &Scenario,
    env: &TestEnv,
    tool: &str,
    model: &str,
    results_dir: &Path,
    transcript_path: Option<&Path>,
    writer: &TranscriptWriter,
) -> anyhow::Result<()> {
    if let Some(scripts) = &scenario.scripts {
        println!("Running {} post-execution script(s)...", scripts.post.len());
        let runner = ScriptRunner::new(
            env.root.clone(),
            results_dir.to_path_buf(),
            scenario.name.clone(),
            tool.to_string(),
            model.to_string(),
            transcript_path.map(|p| p.to_path_buf()),
            Some(writer.base_dir.join("events.jsonl")),
            scenario.target.env.clone().unwrap_or_default(),
        );

        for entry in &scripts.post {
            let result = runner.run(&entry.command, entry.timeout_secs)?;
            let event = serde_json::json!({
                "type": "post_script",
                "command": entry.command,
                "exit_code": result.exit_code,
                "timed_out": result.timed_out,
                "stdout": result.stdout,
                "stderr": result.stderr,
            });
            writer.append_event(&event)?;

            if result.exit_code != 0 {
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
    let output = run_output.transcript;
    let exit_code = run_output.exit_code;
    let cost = run_output.cost_usd;
    let token_usage = run_output.token_usage;
    let metrics_source = run_output.metrics_source;
    let command_events = run_output.command_events;

    // Write transcript immediately after execution so evaluation can read it
    input.writer.write_raw(&output)?;
    if let Some(raw_output) = run_output.raw_output {
        input.writer.write_tool_output(&raw_output)?;
    }
    if !command_events.is_empty() {
        input.writer.write_command_events(&command_events)?;
    }
    // Also copy transcript to fixture directory for gate evaluators that read from env_root
    let fixture_transcript = input.env.root.join("transcript.raw.txt");
    std::fs::write(&fixture_transcript, &output).ok();
    let event = if let Some(c) = cost {
        serde_json::json!({
            "type": "execution",
            "tool": input.tool,
            "output": &output,
            "exit_code": exit_code,
            "cost_usd": c
        })
    } else {
        serde_json::json!({
            "type": "execution",
            "tool": input.tool,
            "output": &output,
            "exit_code": exit_code
        })
    };
    input.writer.append_event(&event)?;

    // Run post-execution scripts after transcript writing, before evaluation
    let transcript_path = input.transcript_dir.join("transcript.raw.txt");
    let events_path = input.writer.base_dir.join("events.jsonl");
    run_post_scripts(
        input.scenario,
        input.env,
        input.tool,
        input.model,
        input.results_dir,
        Some(&transcript_path),
        input.writer,
    )?;

    // Create script runner for evaluation (used by script gates)
    let script_runner = ScriptRunner::new(
        input.env.root.clone(),
        input.results_dir.to_path_buf(),
        input.scenario.name.clone(),
        input.tool.to_string(),
        input.model.to_string(),
        Some(transcript_path),
        Some(events_path),
        input.scenario.target.env.clone().unwrap_or_default(),
    );

    println!("Running evaluation...");
    let completed = exit_code == 0;
    let metrics = crate::evaluation::evaluate(
        input.scenario,
        &input.env.root,
        input.no_judge,
        Some(&script_runner),
        input.judge_model,
        input.judge_tool,
        metrics_source,
        &command_events,
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
