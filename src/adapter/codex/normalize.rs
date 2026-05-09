use crate::adapter::normalize::{json_lines, output_from_parts};
use crate::adapter::{TokenUsage, ToolRunOutput};
use crate::transcript::CommandEvent;
use serde_json::Value;

pub(crate) fn normalize(output: String, exit_code: i32) -> ToolRunOutput {
    let token_usage = parse_token_usage(&output);
    let command_events = extract_command_events(&output);
    let transcript = synthesize_transcript(&output);

    output_from_parts(
        output,
        exit_code,
        transcript,
        None,
        token_usage,
        command_events,
    )
}

fn tokens_from_turn_completed(json: &Value) -> Option<(u64, u64)> {
    let usage = json.get("usage")?;
    let input = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cached_input = usage
        .get("cached_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some((input + cached_input, output))
}

fn parse_token_usage(output: &str) -> Option<TokenUsage> {
    let mut total_input = 0u64;
    let mut total_output = 0u64;

    for line in json_lines(output) {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if json.get("type") == Some(&Value::String("turn.completed".to_string())) {
                if let Some((input, output)) = tokens_from_turn_completed(&json) {
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

fn item_type(item: &Value) -> Option<&str> {
    item.get("type")
        .and_then(Value::as_str)
        .or_else(|| item.get("item_type").and_then(Value::as_str))
}

fn effective_exit_code(item: &Value) -> i32 {
    let exit_code = item.get("exit_code").and_then(Value::as_i64).unwrap_or(0);
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");

    if status == "failed" && exit_code == 0 {
        1
    } else {
        exit_code as i32
    }
}

fn command_item(event: &Value) -> Option<&Value> {
    if event.get("type") != Some(&Value::String("item.completed".to_string())) {
        return None;
    }
    let item = event.get("item")?;
    if item_type(item)? != "command_execution" {
        return None;
    }
    Some(item)
}

fn extract_command_events(output: &str) -> Vec<CommandEvent> {
    let mut events = Vec::new();

    for line in json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(item) = command_item(&event) else {
            continue;
        };
        let command = item.get("command").and_then(Value::as_str).unwrap_or("");
        if command.is_empty() {
            continue;
        }
        events.push(CommandEvent {
            command: command.to_string(),
            exit_code: Some(effective_exit_code(item)),
        });
    }

    events
}

fn synthesize_transcript(output: &str) -> String {
    let mut transcript = String::new();

    for line in json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(item) = command_item(&event) else {
            continue;
        };

        let command = item.get("command").and_then(Value::as_str).unwrap_or("");
        let command_output = item
            .get("aggregated_output")
            .and_then(Value::as_str)
            .unwrap_or("");
        if command.is_empty() {
            continue;
        }

        transcript.push_str("$ ");
        transcript.push_str(command);
        transcript.push('\n');
        if !command_output.is_empty() {
            transcript.push_str(command_output);
            transcript.push('\n');
        }
        transcript.push_str(&format!("exit code: {}\n\n", effective_exit_code(item)));
    }

    transcript
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_codex_jsonl() -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            serde_json::json!({"type": "thread.started", "thread_id": "t1"}),
            serde_json::json!({"type": "turn.started"}),
            serde_json::json!({
                "type": "item.started",
                "item": {"id": "i1", "type": "command_execution", "command": "bash -lc ls", "status": "in_progress"}
            }),
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
            serde_json::json!({
                "type": "turn.completed",
                "usage": {"input_tokens": 100, "cached_input_tokens": 50, "output_tokens": 30}
            }),
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
            serde_json::json!({
                "type": "turn.completed",
                "usage": {"input_tokens": 20, "cached_input_tokens": 0, "output_tokens": 10}
            }),
        )
    }

    #[test]
    fn normalizes_codex_jsonl() {
        let output = normalize(sample_codex_jsonl(), 0);
        let command_events = output.command_events().expect("structured command events");

        assert_eq!(output.token_usage.as_ref().unwrap().input, 170);
        assert_eq!(command_events.len(), 2);
        assert_eq!(command_events[0].command, "bash -lc ls");
        assert_eq!(command_events[1].exit_code, Some(1));
        assert!(output.transcript.contains("docs\nsrc\n"));
        assert!(output.transcript.contains("exit code: 1"));
    }

    #[test]
    fn normalizes_codex_experimental_json_item_type() {
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

        let output = normalize(jsonl_output, 0);
        let command_events = output.command_events().expect("structured command events");
        assert!(output.transcript.contains("$ bash -lc pwd"));
        assert_eq!(command_events[0].exit_code, Some(0));
    }

    #[test]
    fn normalizes_codex_failed_status_to_nonzero_exit() {
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

        let output = normalize(jsonl_output, 0);
        let command_events = output.command_events().expect("structured command events");
        assert!(output.transcript.contains("exit code: 1"));
        assert_eq!(command_events[0].exit_code, Some(1));
    }

    #[test]
    fn handles_plain_text_without_panicking() {
        let output = normalize("just plain text".to_string(), 0);

        assert!(output.token_usage.is_none());
        assert!(output.transcript.is_empty());
    }
}
