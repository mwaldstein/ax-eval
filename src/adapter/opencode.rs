use super::ToolAdapter;
use crate::scenario::Scenario;
use crate::session::SessionRunner;
use serde_json::Value;
use std::path::Path;

pub struct OpenCodeAdapter;

fn extract_json_lines(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.starts_with('{'))
        .collect()
}

fn is_step_finish_event(json: &Value) -> bool {
    json.get("type") == Some(&Value::String("step_finish".to_string()))
}

fn extract_tokens_from_event(json: &Value) -> Option<(u64, u64)> {
    let tokens = json.get("part").and_then(|p| p.get("tokens"))?;
    let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
    let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
    let reasoning = tokens
        .get("reasoning")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some((input + reasoning, output))
}

fn accumulate_token_usage(lines: &[&str]) -> (u64, u64) {
    let mut total_input = 0u64;
    let mut total_output = 0u64;

    for line in lines {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if is_step_finish_event(&json) {
                if let Some((input, output)) = extract_tokens_from_event(&json) {
                    total_input += input;
                    total_output += output;
                }
            }
        }
    }

    (total_input, total_output)
}

fn extract_cost_from_event(json: &Value) -> Option<f64> {
    json.get("part")?.get("cost")?.as_f64()
}

fn accumulate_cost(lines: &[&str]) -> f64 {
    let mut total = 0.0;

    for line in lines {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if is_step_finish_event(&json) {
                if let Some(cost) = extract_cost_from_event(&json) {
                    total += cost;
                }
            }
        }
    }

    total
}

fn parse_cost_from_json(output: &str) -> Option<f64> {
    let lines = extract_json_lines(output);
    let cost = accumulate_cost(&lines);
    if cost > 0.0 {
        Some(cost)
    } else {
        None
    }
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

/// Synthesize a plain-text transcript from opencode's JSON event stream.
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

        // Only process bash tool_use events
        let Some(part) = event.get("part") else {
            continue;
        };
        let Some(tool) = part.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        if tool != "tool" {
            continue;
        }
        let Some(tool_name) = part.get("tool").and_then(|v| v.as_str()) else {
            continue;
        };
        if tool_name != "bash" {
            continue;
        }

        let state = part.get("state");
        let input = state.and_then(|s| s.get("input"));
        let metadata = state.and_then(|s| s.get("metadata"));

        let command = input
            .and_then(|i| i.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let cmd_output = metadata
            .and_then(|m| m.get("output"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let exit_code = metadata
            .and_then(|m| m.get("exit"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if !command.is_empty() {
            transcript.push_str("$ ");
            transcript.push_str(command);
            transcript.push('\n');
            if !cmd_output.is_empty() {
                transcript.push_str(cmd_output);
                transcript.push('\n');
            }
            transcript.push_str(&format!("exit code: {}\n", exit_code));
            transcript.push('\n');
        }
    }

    transcript
}

impl ToolAdapter for OpenCodeAdapter {
    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        let runner = SessionRunner::new();
        match runner.run_command("opencode", &["--version"], Path::new("."), 10) {
            Ok(_) => {
                Ok(super::ToolStatus {
                    available: true,
                    authenticated: true, // opencode doesn't require auth check
                })
            }
            Err(e) => Err(super::AdapterError::NotAvailable(format!(
                "OpenCode tool not found: {}",
                e
            ))),
        }
    }

    fn check_availability(&self) -> anyhow::Result<()> {
        let runner = SessionRunner::new();
        match runner.run_command("opencode", &["--version"], Path::new("."), 10) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("OpenCode tool not found: {}", e)),
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

        // Use 'opencode run' with JSON format for token extraction
        let mut args = vec!["run", "--format", "json"];
        if let Some(model) = model {
            args.push("--model");
            args.push(model);
        }
        args.push(&scenario.task.prompt);

        // Isolate opencode from global AGENTS.md by using a temp XDG_CONFIG_HOME
        // This ensures test results aren't skewed by global prompts/rules/tools
        // while still allowing authentication to work
        // Use absolute path for XDG_CONFIG_HOME to ensure opencode respects it
        let xdg_config_dir = cwd
            .canonicalize()
            .unwrap_or_else(|_| cwd.to_path_buf())
            .join(".opencode_config");
        std::fs::create_dir_all(&xdg_config_dir).ok(); // Create if doesn't exist, ignore errors
        let mut env_vars: Vec<(String, String)> = vec![(
            "XDG_CONFIG_HOME".to_string(),
            xdg_config_dir.to_string_lossy().to_string(),
        )];
        if let Some(target_env) = &scenario.target.env {
            env_vars.extend(target_env.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        let (output, exit_code) =
            runner.run_command_with_env("opencode", &args, cwd, timeout_secs, &env_vars)?;
        let token_usage = parse_token_usage_from_json(&output);
        let cost = parse_cost_from_json(&output);

        // Synthesize a plain-text transcript for the analyzer to parse
        let transcript = synthesize_transcript_from_json(&output);

        Ok((transcript, exit_code, cost, token_usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_opencode_json() -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n",
            // Step 1: init (no error)
            serde_json::json!({
                "type": "step_finish",
                "part": {
                    "type": "step-finish",
                    "tokens": {"input": 10, "output": 20, "reasoning": 5, "total": 35},
                    "cost": 0.042
                }
            }),
            // Step 2: add note (no error)
            serde_json::json!({
                "type": "step_finish",
                "part": {
                    "type": "step-finish",
                    "tokens": {"input": 5, "output": 15, "reasoning": 0, "total": 20},
                    "cost": 0.058
                }
            }),
            // Step 3: failed command then retry
            serde_json::json!({
                "type": "step_finish",
                "part": {
                    "type": "step-finish",
                    "tokens": {"input": 8, "output": 25, "reasoning": 0, "total": 33},
                    "cost": 0.067
                }
            }),
            // Bash tool_use: init
            serde_json::json!({
                "type": "tool_use",
                "part": {
                    "type": "tool",
                    "tool": "bash",
                    "state": {
                        "input": {"command": "./notes init", "description": "init db"},
                        "metadata": {"output": "Initialized", "exit": 0, "description": "init db"}
                    }
                }
            }),
            // Bash tool_use: add (success)
            serde_json::json!({
                "type": "tool_use",
                "part": {
                    "type": "tool",
                    "tool": "bash",
                    "state": {
                        "input": {"command": "./notes add \"Hello\"", "description": "add"},
                        "metadata": {"output": "Created note 1", "exit": 0, "description": "add"}
                    }
                }
            }),
            // Bash tool_use: bad command (error)
            serde_json::json!({
                "type": "tool_use",
                "part": {
                    "type": "tool",
                    "tool": "bash",
                    "state": {
                        "input": {"command": "./notes badcmd", "description": "bad"},
                        "metadata": {"output": "Error: unknown command", "exit": 1, "description": "bad"}
                    }
                }
            }),
        )
    }

    #[test]
    fn test_parse_token_usage_from_json_with_realistic_input() {
        let json_output = sample_opencode_json();
        let result = parse_token_usage_from_json(&json_output);
        assert!(
            result.is_some(),
            "Should parse token usage from realistic JSON"
        );
        let usage = result.unwrap();
        // 10+5 + 5+0 + 8+0 = 28 input, 20+15+25 = 60 output
        assert_eq!(usage.input, 28, "Expected 28 input tokens");
        assert_eq!(usage.output, 60, "Expected 60 output tokens");
    }

    #[test]
    fn test_parse_cost_from_json_with_realistic_input() {
        let json_output = sample_opencode_json();
        let result = parse_cost_from_json(&json_output);
        assert!(result.is_some(), "Should parse cost from realistic JSON");
        let cost = result.unwrap();
        // 0.042 + 0.058 + 0.067 = 0.167
        assert!(
            (cost - 0.167).abs() < 0.001,
            "Expected ~$0.167 cost, got {}",
            cost
        );
    }

    #[test]
    fn test_synthesize_transcript_from_json_with_realistic_input() {
        let json_output = sample_opencode_json();
        let transcript = synthesize_transcript_from_json(&json_output);

        assert!(
            transcript.contains("$ ./notes init"),
            "Should include init command"
        );
        assert!(
            transcript.contains("Initialized"),
            "Should include init output"
        );
        assert!(
            transcript.contains("exit code: 0"),
            "Should include init exit code"
        );

        assert!(
            transcript.contains("$ ./notes add \"Hello\""),
            "Should include add command"
        );
        assert!(
            transcript.contains("Created note 1"),
            "Should include add output"
        );

        assert!(
            transcript.contains("$ ./notes badcmd"),
            "Should include bad command"
        );
        assert!(
            transcript.contains("Error: unknown command"),
            "Should include error output"
        );
        assert!(
            transcript.contains("exit code: 1"),
            "Should include error exit code"
        );
    }

    #[test]
    fn test_synthesize_transcript_skips_non_bash_tools() {
        let json_output = serde_json::json!({
            "type": "tool_use",
            "part": {
                "type": "tool",
                "tool": "read",
                "state": {
                    "input": {"filePath": "/foo"},
                    "metadata": {"output": "bar"}
                }
            }
        })
        .to_string();

        let transcript = synthesize_transcript_from_json(&json_output);
        assert!(!transcript.contains("read"), "Should skip non-bash tools");
        assert!(transcript.is_empty(), "Transcript should be empty");
    }

    #[test]
    fn test_extraction_handles_empty_and_non_json_input() {
        assert!(parse_token_usage_from_json("just plain text").is_none());
        assert!(parse_cost_from_json("just plain text").is_none());
        assert!(synthesize_transcript_from_json("just plain text").is_empty());
    }

    #[test]
    fn test_extraction_handles_missing_fields_gracefully() {
        // step_finish without tokens or cost
        let json_output = serde_json::json!({
            "type": "step_finish",
            "part": {"type": "step-finish"}
        })
        .to_string();

        assert!(parse_token_usage_from_json(&json_output).is_none());
        assert!(parse_cost_from_json(&json_output).is_none());
    }
}
