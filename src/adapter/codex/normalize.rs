use crate::adapter::normalize::{exit_code_text, json_lines, output_from_structured_parts};
use crate::adapter::{TokenUsage, ToolRunOutput};
use crate::interaction_evidence::{CommandEvent, McpToolCallEvent};
use serde_json::Value;

pub(crate) fn normalize(output: String, exit_code: i32) -> ToolRunOutput {
    let token_usage = parse_token_usage(&output);
    let command_events = extract_command_events(&output);
    let mcp_events = extract_mcp_events(&output);
    let transcript = synthesize_transcript(&output);

    output_from_structured_parts(
        output,
        exit_code,
        transcript,
        None,
        token_usage,
        command_events,
        mcp_events,
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

fn effective_exit_code(item: &Value) -> Option<i32> {
    if let Some(exit_code) = item.get("exit_code").and_then(Value::as_i64) {
        if item.get("status").and_then(Value::as_str) == Some("failed") && exit_code == 0 {
            return Some(1);
        }
        return Some(exit_code as i32);
    }

    match item.get("status").and_then(Value::as_str) {
        Some("failed" | "error") => Some(1),
        Some("success" | "succeeded") => Some(0),
        _ => None,
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

fn mcp_item(event: &Value) -> Option<&Value> {
    if event.get("type") != Some(&Value::String("item.completed".to_string())) {
        return None;
    }
    let item = event.get("item")?;
    if item_type(item)? != "mcp_tool_call" {
        return None;
    }
    Some(item)
}

fn mcp_item_is_error(item: &Value) -> bool {
    if let Some(success) = item.get("success").and_then(Value::as_bool) {
        return !success;
    }
    if let Some(is_error) = item.get("is_error").and_then(Value::as_bool) {
        return is_error;
    }

    matches!(
        item.get("status")
            .and_then(Value::as_str)
            .unwrap_or("completed"),
        "failed" | "error"
    ) || item.get("error").is_some_and(|error| !error.is_null())
}

fn mcp_event_from_item(item: &Value) -> Option<McpToolCallEvent> {
    // upstream shape: Codex emits completed MCP calls as item.completed events
    // whose item type/item_type is "mcp_tool_call" with server, tool,
    // arguments/input, and success/is_error/status/error fields.
    let server = item.get("server").and_then(Value::as_str)?;
    let tool = item.get("tool").and_then(Value::as_str)?;
    if server.trim().is_empty() || tool.trim().is_empty() {
        return None;
    }

    Some(McpToolCallEvent {
        server: server.to_string(),
        tool: tool.to_string(),
        arguments: item
            .get("arguments")
            .or_else(|| item.get("input"))
            .cloned()
            .unwrap_or(Value::Null),
        is_error: mcp_item_is_error(item),
        duration_ms: item.get("duration_ms").and_then(Value::as_u64),
    })
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
            exit_code: effective_exit_code(item),
        });
    }

    events
}

fn extract_mcp_events(output: &str) -> Vec<McpToolCallEvent> {
    let mut events = Vec::new();

    for line in json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(item) = mcp_item(&event) else {
            continue;
        };
        if let Some(event) = mcp_event_from_item(item) {
            events.push(event);
        }
    }

    events
}

fn text_fragments(value: &Value, fragments: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if !text.trim().is_empty() {
                fragments.push(text.to_string());
            }
        }
        Value::Array(values) => {
            for value in values {
                text_fragments(value, fragments);
            }
        }
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    fragments.push(text.to_string());
                    return;
                }
            }
            if let Some(text) = object.get("content").and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    fragments.push(text.to_string());
                    return;
                }
            }
            for key in ["content", "parts"] {
                if let Some(value) = object.get(key) {
                    text_fragments(value, fragments);
                }
            }
        }
        _ => {}
    }
}

fn text_from_value(value: &Value) -> Option<String> {
    let mut fragments = Vec::new();
    text_fragments(value, &mut fragments);
    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n"))
    }
}

fn assistant_text_from_event(event: &Value) -> Option<String> {
    match event.get("type").and_then(Value::as_str)? {
        "agent_message" | "assistant_message" | "message" => text_from_value(event),
        "item.completed" | "item.updated" => {
            let item = event.get("item")?;
            match item_type(item)? {
                "agent_message" | "message" | "assistant_message" => text_from_value(item),
                _ => None,
            }
        }
        _ => None,
    }
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn append_mcp_transcript_item(transcript: &mut String, item: &Value) -> bool {
    let Some(event) = mcp_event_from_item(item) else {
        return false;
    };

    transcript.push_str("[mcp ");
    transcript.push_str(&event.server);
    transcript.push('.');
    transcript.push_str(&event.tool);
    transcript.push_str("]\n");
    if !event.arguments.is_null() {
        transcript.push_str("arguments: ");
        transcript.push_str(&compact_json(&event.arguments));
        transcript.push('\n');
    }
    if let Some(result) = item.get("result") {
        transcript.push_str("result: ");
        transcript.push_str(&compact_json(result));
        transcript.push('\n');
    }
    if let Some(error) = item.get("error").filter(|error| !error.is_null()) {
        transcript.push_str("error: ");
        transcript.push_str(&compact_json(error));
        transcript.push('\n');
    }
    transcript.push_str(if event.is_error {
        "status: error\n\n"
    } else {
        "status: success\n\n"
    });
    true
}

fn synthesize_transcript(output: &str) -> String {
    let mut transcript = String::new();

    for line in json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if let Some(text) = assistant_text_from_event(&event) {
            if !text.is_empty() {
                transcript.push_str(&text);
                transcript.push_str("\n\n");
            }
            continue;
        }

        if let Some(item) = mcp_item(&event) {
            append_mcp_transcript_item(&mut transcript, item);
            continue;
        }

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
        transcript.push_str("exit code: ");
        transcript.push_str(&exit_code_text(effective_exit_code(item)));
        transcript.push_str("\n\n");
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
    fn preserves_missing_codex_exit_code_as_unknown() {
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "i1",
                "type": "command_execution",
                "command": "bash -lc pwd",
                "aggregated_output": "/workspace\n",
                "status": "completed"
            }
        })
        .to_string();

        let output = normalize(jsonl_output, 0);
        let command_events = output.command_events().expect("structured command events");
        assert_eq!(command_events[0].exit_code, None);
        assert!(output.transcript.contains("exit code: unknown"));
        assert!(!output.transcript.contains("exit code: 0"));
    }

    #[test]
    fn maps_explicit_codex_failure_status_without_exit_code() {
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "i1",
                "type": "command_execution",
                "command": "bash -lc false",
                "status": "failed"
            }
        })
        .to_string();

        let output = normalize(jsonl_output, 0);
        let command_events = output.command_events().expect("structured command events");
        assert_eq!(command_events[0].exit_code, Some(1));
    }

    #[test]
    fn normalizes_successful_codex_mcp_tool_call() {
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "i1",
                "type": "mcp_tool_call",
                "server": "todo",
                "tool": "add",
                "arguments": {"text": "hello"},
                "success": true,
                "duration_ms": 12
            }
        })
        .to_string();

        let output = normalize(jsonl_output, 0);
        let mcp_events = output
            .mcp_tool_call_events()
            .expect("structured mcp tool call events");
        assert_eq!(mcp_events.len(), 1);
        assert_eq!(mcp_events[0].server, "todo");
        assert_eq!(mcp_events[0].tool, "add");
        assert_eq!(
            mcp_events[0].arguments,
            serde_json::json!({"text": "hello"})
        );
        assert!(!mcp_events[0].is_error);
        assert_eq!(mcp_events[0].duration_ms, Some(12));
    }

    #[test]
    fn normalizes_failed_codex_mcp_tool_call() {
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "i1",
                "type": "mcp_tool_call",
                "server": "todo",
                "tool": "complete",
                "arguments": {"id": 99},
                "status": "failed",
                "error": "missing id"
            }
        })
        .to_string();

        let output = normalize(jsonl_output, 0);
        let mcp_events = output
            .mcp_tool_call_events()
            .expect("structured mcp tool call events");
        assert_eq!(mcp_events[0].tool, "complete");
        assert!(mcp_events[0].is_error);
    }

    #[test]
    fn normalizes_mixed_codex_bash_and_mcp_events() {
        let jsonl_output = format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "i1",
                    "type": "command_execution",
                    "command": "bash -lc 'notes list'",
                    "aggregated_output": "[]",
                    "exit_code": 0,
                    "status": "completed"
                }
            }),
            serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "i2",
                    "type": "mcp_tool_call",
                    "server": "todo",
                    "tool": "add",
                    "arguments": {"text": "hello"},
                    "success": true
                }
            })
        );

        let output = normalize(jsonl_output, 0);
        assert_eq!(
            output.command_events().expect("structured command events")[0].command,
            "bash -lc 'notes list'"
        );
        assert_eq!(
            output
                .mcp_tool_call_events()
                .expect("structured mcp tool call events")[0]
                .tool,
            "add"
        );
        assert!(output.transcript.contains("$ bash -lc 'notes list'"));
        assert!(output.transcript.contains("[mcp todo.add]"));
        assert!(output.transcript.contains(r#"arguments: {"text":"hello"}"#));
    }

    #[test]
    fn codex_transcript_includes_assistant_messages_and_mcp_results() {
        let jsonl_output = format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "msg_1",
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "I will add the note."}]
                }
            }),
            serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "i2",
                    "type": "mcp_tool_call",
                    "server": "todo",
                    "tool": "add",
                    "arguments": {"text": "hello"},
                    "result": {"content": [{"type": "text", "text": "created note n1"}]},
                    "success": true
                }
            })
        );

        let output = normalize(jsonl_output, 0);

        assert!(output.transcript.contains("I will add the note."));
        assert!(output.transcript.contains("[mcp todo.add]"));
        assert!(output.transcript.contains("created note n1"));
    }

    #[test]
    fn ignores_non_mcp_codex_item_type() {
        let jsonl_output = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "i1",
                "type": "tool_use",
                "server": "todo",
                "tool": "add"
            }
        })
        .to_string();

        let output = normalize(jsonl_output, 0);
        assert!(output.mcp_tool_call_events().unwrap().is_empty());
    }

    #[test]
    fn handles_plain_text_without_panicking() {
        let output = normalize("just plain text".to_string(), 0);

        assert!(output.token_usage.is_none());
        assert!(output.transcript.is_empty());
    }

    #[test]
    fn handles_malformed_codex_json_without_panicking() {
        let output = normalize("{\"type\":\"item.completed\"\nnot json".to_string(), 0);

        assert!(output.command_events().unwrap().is_empty());
        assert!(output.mcp_tool_call_events().unwrap().is_empty());
    }

    #[test]
    fn real_codex_completed_mcp_item_with_null_error_is_success() {
        // Real captured event from codex-cli 0.144.6 running the Everything
        // reference server (2026-07-18): successful calls carry
        // "error": null and "status": "completed". A JSON null error must
        // not be read as failure.
        let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","server":"everything","tool":"get-sum","arguments":{"a":17,"b":25},"result":{"content":[{"type":"text","text":"The sum of 17 and 25 is 42."}],"structured_content":null},"error":null,"status":"completed"}}"#;

        let events = extract_mcp_events(output);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].server, "everything");
        assert_eq!(events[0].tool, "get-sum");
        assert!(
            !events[0].is_error,
            "completed item with null error must be a success"
        );
    }
}
