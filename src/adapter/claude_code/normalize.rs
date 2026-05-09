use crate::adapter::normalize::{output_from_parts, transcript_from_command_events};
use crate::adapter::ToolRunOutput;
use crate::transcript::CommandEvent;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(crate) fn normalize(output: String, exit_code: i32) -> ToolRunOutput {
    let command_events = extract_command_events(&output);
    let transcript = transcript_from_command_events(&command_events);
    let transcript = if transcript.is_empty() {
        output.clone()
    } else {
        transcript
    };
    let cost = parse_cost(&output);

    output_from_parts(output, exit_code, transcript, cost, None, command_events)
}

#[derive(Debug, Default)]
struct PartialToolUse {
    id: Option<String>,
    name: Option<String>,
    input_json: String,
}

fn is_bash_tool(name: &str) -> bool {
    name == "Bash" || name == "bash"
}

fn bash_command_from_input(input: Option<&Value>) -> Option<String> {
    input?
        .get("command")
        .and_then(Value::as_str)
        .filter(|command| !command.trim().is_empty())
        .map(ToString::to_string)
}

fn result_exit_code(item: &Value) -> Option<i32> {
    if let Some(is_error) = item.get("is_error").and_then(Value::as_bool) {
        return Some(if is_error { 1 } else { 0 });
    }

    let text = item
        .get("content")
        .and_then(content_to_text)
        .unwrap_or_default();
    let exit_regex = Regex::new(r"(?i)exit\s+(?:code|status):?\s*(\d+)").expect("valid regex");
    if let Some(captures) = exit_regex.captures(&text) {
        return captures
            .get(1)
            .and_then(|matched| matched.as_str().parse::<i32>().ok());
    }

    if text.to_lowercase().contains("error") || text.to_lowercase().contains("failed") {
        Some(1)
    } else {
        Some(0)
    }
}

fn content_to_text(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    if let Some(items) = content.as_array() {
        let text = items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Some(text);
        }
    }

    None
}

fn add_tool_use(
    tool_use_id: Option<String>,
    tool_name: Option<&str>,
    input: Option<&Value>,
    tool_ids: &mut HashMap<String, usize>,
    seen_keys: &mut HashSet<String>,
    command_events: &mut Vec<CommandEvent>,
) {
    let Some(tool_name) = tool_name else {
        return;
    };
    if !is_bash_tool(tool_name) {
        return;
    }
    let Some(command) = bash_command_from_input(input) else {
        return;
    };

    let key = tool_use_id
        .clone()
        .unwrap_or_else(|| format!("{}:{}", command_events.len(), command));
    if !seen_keys.insert(key.clone()) {
        return;
    }

    let index = command_events.len();
    command_events.push(CommandEvent {
        command,
        exit_code: None,
    });

    if let Some(id) = tool_use_id {
        tool_ids.insert(id, index);
    }
}

fn update_tool_result(
    item: &Value,
    tool_ids: &HashMap<String, usize>,
    command_events: &mut [CommandEvent],
) {
    let Some(tool_use_id) = item.get("tool_use_id").and_then(Value::as_str) else {
        return;
    };
    let Some(index) = tool_ids.get(tool_use_id).copied() else {
        return;
    };

    command_events[index].exit_code = result_exit_code(item);
}

fn process_message_content(
    message: &Value,
    tool_ids: &mut HashMap<String, usize>,
    seen_keys: &mut HashSet<String>,
    command_events: &mut Vec<CommandEvent>,
) {
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return;
    };

    for item in content {
        match item.get("type").and_then(Value::as_str) {
            Some("tool_use") => add_tool_use(
                item.get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                item.get("name").and_then(Value::as_str),
                item.get("input"),
                tool_ids,
                seen_keys,
                command_events,
            ),
            Some("tool_result") => update_tool_result(item, tool_ids, command_events),
            _ => {}
        }
    }
}

fn process_stream_event(
    event: &Value,
    partial_tools: &mut HashMap<u64, PartialToolUse>,
    tool_ids: &mut HashMap<String, usize>,
    seen_keys: &mut HashSet<String>,
    command_events: &mut Vec<CommandEvent>,
) {
    let Some(event_type) = event.get("type").and_then(Value::as_str) else {
        return;
    };
    let index = event
        .get("index")
        .or_else(|| event.get("content_block_index"))
        .and_then(Value::as_u64)
        .unwrap_or(0);

    match event_type {
        "content_block_start" => {
            let Some(block) = event.get("content_block") else {
                return;
            };
            if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                return;
            }

            let mut partial = PartialToolUse {
                id: block
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                name: block
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                input_json: String::new(),
            };

            if let Some(input) = block.get("input") {
                partial.input_json = input.to_string();
            }

            partial_tools.insert(index, partial);
        }
        "content_block_delta" => {
            let Some(delta) = event.get("delta") else {
                return;
            };
            if delta.get("type").and_then(Value::as_str) != Some("input_json_delta") {
                return;
            }
            let Some(partial_json) = delta.get("partial_json").and_then(Value::as_str) else {
                return;
            };
            partial_tools
                .entry(index)
                .or_default()
                .input_json
                .push_str(partial_json);
        }
        "content_block_stop" => {
            let Some(partial) = partial_tools.remove(&index) else {
                return;
            };
            let input = serde_json::from_str::<Value>(&partial.input_json).ok();
            add_tool_use(
                partial.id,
                partial.name.as_deref(),
                input.as_ref(),
                tool_ids,
                seen_keys,
                command_events,
            );
        }
        _ => {}
    }
}

fn extract_command_events(output: &str) -> Vec<CommandEvent> {
    let mut command_events = Vec::new();
    let mut tool_ids = HashMap::new();
    let mut seen_keys = HashSet::new();
    let mut partial_tools = HashMap::new();

    for line in output.lines() {
        if !line.trim_start().starts_with('{') {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if let Some(message) = value.get("message") {
            process_message_content(message, &mut tool_ids, &mut seen_keys, &mut command_events);
        }

        if value.get("type").and_then(Value::as_str) == Some("stream_event") {
            if let Some(event) = value.get("event") {
                process_stream_event(
                    event,
                    &mut partial_tools,
                    &mut tool_ids,
                    &mut seen_keys,
                    &mut command_events,
                );
            }
        }
    }

    for event in &mut command_events {
        event.exit_code.get_or_insert(0);
    }

    command_events
}

fn parse_cost(output: &str) -> Option<f64> {
    output.lines().rev().find_map(|line| {
        let value = serde_json::from_str::<Value>(line).ok()?;
        value.get("total_cost_usd").and_then(Value::as_f64)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_claude_complete_messages() {
        let raw = format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_1",
                        "name": "Bash",
                        "input": {"command": "notes add \"Hello\"", "description": "add note"}
                    }]
                }
            }),
            serde_json::json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_1",
                        "content": "Created note",
                        "is_error": false
                    }]
                }
            })
        );

        let output = normalize(raw, 0);
        let command_events = output.command_events().expect("structured command events");

        assert_eq!(command_events.len(), 1);
        assert_eq!(command_events[0].command, "notes add \"Hello\"");
        assert_eq!(command_events[0].exit_code, Some(0));
        assert!(output.transcript.contains("$ notes add \"Hello\""));
    }

    #[test]
    fn normalizes_claude_partial_stream_events() {
        let raw = format!(
            "{}\n{}\n{}\n{}\n",
            serde_json::json!({
                "type": "stream_event",
                "event": {
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "tool_use", "id": "toolu_1", "name": "Bash"}
                }
            }),
            serde_json::json!({
                "type": "stream_event",
                "event": {
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "input_json_delta", "partial_json": "{\"command\":\"notes"}
                }
            }),
            serde_json::json!({
                "type": "stream_event",
                "event": {
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "input_json_delta", "partial_json": " list\"}"}
                }
            }),
            serde_json::json!({
                "type": "stream_event",
                "event": {"type": "content_block_stop", "index": 0}
            })
        );

        let output = normalize(raw, 0);
        let command_events = output.command_events().expect("structured command events");
        assert_eq!(command_events[0].command, "notes list");
        assert_eq!(command_events[0].exit_code, Some(0));
    }

    #[test]
    fn normalizes_claude_failed_tool_result() {
        let raw = format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_1",
                        "name": "Bash",
                        "input": {"command": "notes badcmd"}
                    }]
                }
            }),
            serde_json::json!({
                "type": "user",
                "message": {
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_1",
                        "content": "Command failed",
                        "is_error": true
                    }]
                }
            })
        );

        let output = normalize(raw, 0);
        let command_events = output.command_events().expect("structured command events");
        assert_eq!(command_events[0].exit_code, Some(1));
    }
}
