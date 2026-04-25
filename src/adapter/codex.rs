use super::ToolAdapter;
use crate::scenario::Scenario;
use crate::session::SessionRunner;
use serde_json::Value;
use std::path::Path;

pub struct CodexAdapter;

fn extract_json_lines(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.starts_with('{'))
        .collect()
}

fn extract_tokens_from_turn_completed(json: &Value) -> Option<(u64, u64)> {
    let usage = json.get("usage")?;
    let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let cached_input = usage
        .get("cached_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some((input + cached_input, output))
}

fn accumulate_token_usage(lines: &[&str]) -> (u64, u64) {
    let mut total_input = 0u64;
    let mut total_output = 0u64;

    for line in lines {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if json.get("type") == Some(&Value::String("turn.completed".to_string())) {
                if let Some((input, output)) = extract_tokens_from_turn_completed(&json) {
                    total_input += input;
                    total_output += output;
                }
            }
        }
    }

    (total_input, total_output)
}

fn parse_token_usage_from_json(output: &str) -> Option<super::TokenUsage> {
    let lines = extract_json_lines(output);
    let (total_input, total_output) = accumulate_token_usage(&lines);

    if total_input > 0 || total_output > 0 {
        Some(super::TokenUsage {
            input: total_input as usize,
            output: total_output as usize,
        })
    } else {
        None
    }
}

/// Extract the item type from a codex JSON event, handling both
/// `item.type` (stable --json) and `item.item_type` (--experimental-json).
fn item_type(item: &Value) -> Option<&str> {
    item.get("type")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("item_type").and_then(|v| v.as_str()))
}

/// Synthesize a plain-text transcript from codex's JSONL event stream.
/// The transcript analyzer expects shell commands with exit codes in plain text.
fn synthesize_transcript_from_json(output: &str) -> String {
    let mut transcript = String::new();

    for line in output.lines() {
        if !line.starts_with('{') {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        // Only process item.completed events
        if event.get("type") != Some(&Value::String("item.completed".to_string())) {
            continue;
        }

        let Some(item) = event.get("item") else {
            continue;
        };

        let Some(item_ty) = item_type(item) else {
            continue;
        };

        if item_ty != "command_execution" {
            continue;
        }

        let command = item
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let cmd_output = item
            .get("aggregated_output")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let exit_code = item
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let status = item
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("completed");

        if !command.is_empty() {
            transcript.push_str("$ ");
            transcript.push_str(command);
            transcript.push('\n');
            if !cmd_output.is_empty() {
                transcript.push_str(cmd_output);
                transcript.push('\n');
            }
            // Map failed status to a non-zero exit code for the analyzer
            let effective_exit = if status == "failed" && exit_code == 0 {
                1
            } else {
                exit_code
            };
            transcript.push_str(&format!("exit code: {}\n", effective_exit));
            transcript.push('\n');
        }
    }

    transcript
}

impl ToolAdapter for CodexAdapter {
    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        let runner = SessionRunner::new();
        let available = match runner.run_command("codex", &["--version"], Path::new("."), 10) {
            Ok(_) => true,
            Err(e) => {
                return Err(super::AdapterError::NotAvailable(format!(
                    "Codex tool not found: {}",
                    e
                )))
            }
        };

        // Codex requires OPENAI_API_KEY to be authenticated
        let authenticated = std::env::var("OPENAI_API_KEY").is_ok();

        Ok(super::ToolStatus {
            available,
            authenticated,
        })
    }

    fn check_availability(&self) -> anyhow::Result<()> {
        let runner = SessionRunner::new();
        match runner.run_command("codex", &["--version"], Path::new("."), 10) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Codex tool not found: {}", e)),
        }
    }

    fn run(
        &self,
        scenario: &Scenario,
        cwd: &Path,
        model: Option<&str>,
        timeout_secs: u64,
    ) -> anyhow::Result<(String, i32, Option<f64>, Option<super::TokenUsage>)> {
        let runner = SessionRunner::new();

        // Use 'codex exec' with JSON format for structured output extraction
        let mut args = vec!["exec", "--json", "--full-auto", "--skip-git-repo-check"];
        if let Some(model) = model {
            args.push("--model");
            args.push(model);
        }
        args.push(&scenario.task.prompt);

        let target_env = scenario
            .target
            .env
            .as_ref()
            .map(|vars| {
                vars.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Vec<(String, String)>>()
            })
            .unwrap_or_default();

        let (output, exit_code) =
            runner.run_command_with_env("codex", &args, cwd, timeout_secs, &target_env)?;

        let token_usage = parse_token_usage_from_json(&output);

        // Synthesize a plain-text transcript for the analyzer to parse
        let transcript = synthesize_transcript_from_json(&output);

        // Codex does not emit cost in its JSONL stream; leave as None
        Ok((transcript, exit_code, None, token_usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_codex_jsonl() -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            // thread start
            serde_json::json!({"type": "thread.started", "thread_id": "t1"}),
            // turn start
            serde_json::json!({"type": "turn.started"}),
            // command started
            serde_json::json!({
                "type": "item.started",
                "item": {"id": "i1", "type": "command_execution", "command": "bash -lc ls", "status": "in_progress"}
            }),
            // command completed
            serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "i1",
                    "type": "command_execution",
                    "command": "bash -lc ls",
                    "aggregated_output": "docs\nsrc\n",
                    "exit_code": 0,
                    "status": "completed"
                }
            }),
            // turn completed with usage
            serde_json::json!({
                "type": "turn.completed",
                "usage": {"input_tokens": 100, "cached_input_tokens": 50, "output_tokens": 30}
            }),
            // another turn: failed command
            serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "i2",
                    "type": "command_execution",
                    "command": "bash -lc false",
                    "aggregated_output": "",
                    "exit_code": 1,
                    "status": "completed"
                }
            }),
            // turn completed with usage
            serde_json::json!({
                "type": "turn.completed",
                "usage": {"input_tokens": 20, "cached_input_tokens": 0, "output_tokens": 10}
            }),
        )
    }

    #[test]
    fn test_parse_token_usage_from_jsonl() {
        let jsonl_output = sample_codex_jsonl();
        let result = parse_token_usage_from_json(&jsonl_output);
        assert!(result.is_some(), "Should parse token usage from JSONL");
        let usage = result.unwrap();
        // (100+50) + (20+0) = 170 input, 30 + 10 = 40 output
        assert_eq!(usage.input, 170, "Expected 170 input tokens");
        assert_eq!(usage.output, 40, "Expected 40 output tokens");
    }

    #[test]
    fn test_synthesize_transcript_from_jsonl() {
        let jsonl_output = sample_codex_jsonl();
        let transcript = synthesize_transcript_from_json(&jsonl_output);

        assert!(
            transcript.contains("$ bash -lc ls"),
            "Should include ls command"
        );
        assert!(
            transcript.contains("docs\nsrc\n"),
            "Should include ls output"
        );
        assert!(
            transcript.contains("exit code: 0"),
            "Should include success exit code"
        );

        assert!(
            transcript.contains("$ bash -lc false"),
            "Should include false command"
        );
        assert!(
            transcript.contains("exit code: 1"),
            "Should include failure exit code"
        );
    }

    #[test]
    fn test_synthesize_transcript_skips_non_command_items() {
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {"id": "i1", "type": "agent_message", "text": "Hello"}
        })
        .to_string();

        let transcript = synthesize_transcript_from_json(&jsonl_output);
        assert!(
            !transcript.contains("Hello"),
            "Should skip non-command items"
        );
        assert!(transcript.is_empty(), "Transcript should be empty");
    }

    #[test]
    fn test_experimental_json_item_type() {
        // --experimental-json uses item.item_type instead of item.type
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "i1",
                "item_type": "command_execution",
                "command": "bash -lc pwd",
                "aggregated_output": "/home/user\n",
                "exit_code": 0,
                "status": "completed"
            }
        })
        .to_string();

        let transcript = synthesize_transcript_from_json(&jsonl_output);
        assert!(
            transcript.contains("$ bash -lc pwd"),
            "Should handle experimental-json item_type"
        );
        assert!(
            transcript.contains("exit code: 0"),
            "Should handle experimental-json exit code"
        );
    }

    #[test]
    fn test_extraction_handles_empty_and_non_json_input() {
        assert!(parse_token_usage_from_json("just plain text").is_none());
        assert!(synthesize_transcript_from_json("just plain text").is_empty());
    }

    #[test]
    fn test_extraction_handles_missing_fields_gracefully() {
        let jsonl_output = serde_json::json!({
            "type": "turn.completed",
            "usage": {}
        })
        .to_string();

        assert!(parse_token_usage_from_json(&jsonl_output).is_none());
    }

    #[test]
    fn test_failed_status_maps_to_nonzero_exit() {
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "i1",
                "type": "command_execution",
                "command": "bash -lc rm /root",
                "aggregated_output": "Permission denied",
                "exit_code": 0,
                "status": "failed"
            }
        })
        .to_string();

        let transcript = synthesize_transcript_from_json(&jsonl_output);
        assert!(
            transcript.contains("exit code: 1"),
            "Should map failed status to exit code 1 even when exit_code is 0"
        );
    }
}
