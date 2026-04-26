use crate::adapter::{TokenUsage, ToolAdapter};
use crate::evaluation::EvaluationMetrics;
use crate::fixture::TestEnv;
use crate::scenario::Scenario;
use crate::script_runner::ScriptRunner;
use crate::transcript::TranscriptWriter;
use std::path::Path;

pub fn execute_tool(
    adapter: &dyn ToolAdapter,
    s: &Scenario,
    env: &TestEnv,
    tool: &str,
    model: &str,
    effective_timeout: u64,
) -> anyhow::Result<(String, i32, Option<f64>, Option<TokenUsage>)> {
    let start_time = std::time::Instant::now();
    println!("Running tool '{}' with model '{}'...", tool, model);
    let (output, exit_code, cost_opt, token_usage) =
        adapter.run(s, &env.root, Some(model), effective_timeout)?;
    let _duration = start_time.elapsed();

    Ok((output, exit_code, cost_opt, token_usage))
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

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn run_evaluation_flow(
    adapter: &dyn ToolAdapter,
    s: &Scenario,
    env: &TestEnv,
    tool: &str,
    model: &str,
    effective_timeout: u64,
    no_judge: bool,
    writer: &TranscriptWriter,
    transcript_dir: &Path,
    results_dir: &Path,
) -> anyhow::Result<(
    String,
    i32,
    Option<f64>,
    Option<TokenUsage>,
    std::time::Duration,
    EvaluationMetrics,
)> {
    let start = std::time::Instant::now();
    let (output, exit_code, cost, token_usage) =
        execute_tool(adapter, s, env, tool, model, effective_timeout)?;
    let duration = start.elapsed();

    // Write transcript immediately after execution so evaluation can read it
    writer.write_raw(&output)?;
    // Also copy transcript to fixture directory for gate evaluators that read from env_root
    let fixture_transcript = env.root.join("transcript.raw.txt");
    std::fs::write(&fixture_transcript, &output).ok();
    let event = if let Some(c) = cost {
        serde_json::json!({
            "type": "execution",
            "tool": tool,
            "output": &output,
            "exit_code": exit_code,
            "cost_usd": c
        })
    } else {
        serde_json::json!({
            "type": "execution",
            "tool": tool,
            "output": &output,
            "exit_code": exit_code
        })
    };
    writer.append_event(&event)?;

    // Run post-execution scripts after transcript writing, before evaluation
    let transcript_path = transcript_dir.join("transcript.raw.txt");
    let events_path = writer.base_dir.join("events.jsonl");
    run_post_scripts(
        s,
        env,
        tool,
        model,
        results_dir,
        Some(&transcript_path),
        writer,
    )?;

    // Create script runner for evaluation (used by script gates)
    let script_runner = ScriptRunner::new(
        env.root.clone(),
        results_dir.to_path_buf(),
        s.name.clone(),
        tool.to_string(),
        model.to_string(),
        Some(transcript_path),
        Some(events_path),
        s.target.env.clone().unwrap_or_default(),
    );

    println!("Running evaluation...");
    let metrics = crate::evaluation::evaluate(s, &env.root, no_judge, Some(&script_runner))?;
    println!("Evaluation metrics: {:?}", metrics);

    Ok((output, exit_code, cost, token_usage, duration, metrics))
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
