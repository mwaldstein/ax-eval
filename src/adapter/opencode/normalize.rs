use crate::adapter::normalize::{json_lines, output_from_parts};
use crate::adapter::{TokenUsage, ToolRunOutput};
use crate::interaction_evidence::CommandEvent;
use serde_json::Value;

pub(crate) fn normalize(output: String, exit_code: i32) -> ToolRunOutput {
    let token_usage = parse_token_usage(&output);
    let cost = parse_cost(&output);
    let command_events = extract_command_events(&output);
    let transcript = synthesize_transcript(&output);

    output_from_parts(
        output,
        exit_code,
        transcript,
        cost,
        token_usage,
        command_events,
    )
}

fn step_finish_event(json: &Value) -> bool {
    json.get("type") == Some(&Value::String("step_finish".to_string()))
}

fn tokens_from_event(json: &Value) -> Option<(u64, u64)> {
    let tokens = json.get("part").and_then(|p| p.get("tokens"))?;
    let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
    let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
    let reasoning = tokens
        .get("reasoning")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some((input + reasoning, output))
}

fn parse_token_usage(output: &str) -> Option<TokenUsage> {
    let mut total_input = 0u64;
    let mut total_output = 0u64;

    for line in json_lines(output) {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if step_finish_event(&json) {
                if let Some((input, output)) = tokens_from_event(&json) {
                    total_input += input;
                    total_output += output;
                }
            }
        }
    }

    if total_input > 0 || total_output > 0 {
        Some(TokenUsage {
            input: total_input as usize,
            output: total_output as usize,
        })
    } else {
        None
    }
}

fn parse_cost(output: &str) -> Option<f64> {
    let mut total = 0.0;

    for line in json_lines(output) {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if step_finish_event(&json) {
                if let Some(cost) = json
                    .get("part")
                    .and_then(|p| p.get("cost"))
                    .and_then(Value::as_f64)
                {
                    total += cost;
                }
            }
        }
    }

    if total > 0.0 {
        Some(total)
    } else {
        None
    }
}

fn command_event(event: &Value) -> Option<CommandEvent> {
    let part = event.get("part")?;
    if part.get("type").and_then(Value::as_str)? != "tool" {
        return None;
    }
    if part.get("tool").and_then(Value::as_str)? != "bash" {
        return None;
    }

    let state = part.get("state");
    let input = state.and_then(|s| s.get("input"));
    let metadata = state.and_then(|s| s.get("metadata"));

    let command = input
        .and_then(|i| i.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if command.is_empty() {
        return None;
    }

    let exit_code = metadata
        .and_then(|m| m.get("exit"))
        .and_then(Value::as_i64)
        .map(|code| code as i32)
        .unwrap_or(0);

    Some(CommandEvent {
        command: command.to_string(),
        exit_code: Some(exit_code),
    })
}

fn extract_command_events(output: &str) -> Vec<CommandEvent> {
    json_lines(output)
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|event| command_event(&event))
        .collect()
}

fn synthesize_transcript(output: &str) -> String {
    let mut transcript = String::new();

    for line in json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(command_event) = command_event(&event) else {
            continue;
        };
        let output = event
            .get("part")
            .and_then(|p| p.get("state"))
            .and_then(|s| s.get("metadata"))
            .and_then(|m| m.get("output"))
            .and_then(Value::as_str)
            .unwrap_or("");

        transcript.push_str("$ ");
        transcript.push_str(&command_event.command);
        transcript.push('\n');
        if !output.is_empty() {
            transcript.push_str(output);
            transcript.push('\n');
        }
        transcript.push_str(&format!(
            "exit code: {}\n\n",
            command_event.exit_code.unwrap_or(0)
        ));
    }

    transcript
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_opencode_json() -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n",
            serde_json::json!({
                "type": "step_finish",
                "part": {
                    "type": "step-finish",
                    "tokens": {"input": 10, "output": 20, "reasoning": 5, "total": 35},
                    "cost": 0.042
                }
            }),
            serde_json::json!({
                "type": "step_finish",
                "part": {
                    "type": "step-finish",
                    "tokens": {"input": 5, "output": 15, "reasoning": 0, "total": 20},
                    "cost": 0.058
                }
            }),
            serde_json::json!({
                "type": "step_finish",
                "part": {
                    "type": "step-finish",
                    "tokens": {"input": 8, "output": 25, "reasoning": 0, "total": 33},
                    "cost": 0.067
                }
            }),
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
    fn normalizes_opencode_json() {
        let output = normalize(sample_opencode_json(), 0);
        let command_events = output.command_events().expect("structured command events");

        let usage = output.token_usage.as_ref().unwrap();
        assert_eq!(usage.input, 28);
        assert_eq!(usage.output, 60);
        assert!((output.cost_usd.unwrap() - 0.167).abs() < 0.001);
        assert_eq!(command_events.len(), 3);
        assert_eq!(command_events[2].command, "./notes badcmd");
        assert_eq!(command_events[2].exit_code, Some(1));
        assert!(output.transcript.contains("Error: unknown command"));
    }

    #[test]
    fn skips_non_bash_tools() {
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

        let output = normalize(json_output, 0);
        assert!(output.transcript.is_empty());
        assert!(output
            .command_events()
            .expect("structured command events")
            .is_empty());
    }

    #[test]
    fn handles_plain_text_without_panicking() {
        let output = normalize("just plain text".to_string(), 0);

        assert!(output.token_usage.is_none());
        assert!(output.cost_usd.is_none());
        assert!(output.transcript.is_empty());
    }
}
