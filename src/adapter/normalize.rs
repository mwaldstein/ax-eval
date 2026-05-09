use super::{TokenUsage, ToolRunOutput};
use crate::transcript::{CommandEvent, InteractionMetricsSource};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(crate) fn opencode_output(output: String, exit_code: i32) -> ToolRunOutput {
    let token_usage = parse_opencode_token_usage(&output);
    let cost = parse_opencode_cost(&output);
    let command_events = extract_opencode_command_events(&output);
    let transcript = synthesize_opencode_transcript(&output);

    ToolRunOutput {
        transcript,
        raw_output: Some(output),
        exit_code,
        cost_usd: cost,
        token_usage,
        metrics_source: InteractionMetricsSource::StructuredToolCalls,
        command_events,
    }
}

pub(crate) fn codex_output(output: String, exit_code: i32) -> ToolRunOutput {
    let token_usage = parse_codex_token_usage(&output);
    let command_events = extract_codex_command_events(&output);
    let transcript = synthesize_codex_transcript(&output);

    ToolRunOutput {
        transcript,
        raw_output: Some(output),
        exit_code,
        cost_usd: None,
        token_usage,
        metrics_source: InteractionMetricsSource::StructuredToolCalls,
        command_events,
    }
}

pub(crate) fn claude_code_output(output: String, exit_code: i32) -> ToolRunOutput {
    let command_events = extract_claude_command_events(&output);
    let transcript = synthesize_transcript_from_command_events(&command_events);
    let transcript = if transcript.is_empty() {
        output.clone()
    } else {
        transcript
    };
    let cost = parse_claude_cost(&output);

    ToolRunOutput {
        transcript,
        raw_output: Some(output),
        exit_code,
        cost_usd: cost,
        token_usage: None,
        metrics_source: InteractionMetricsSource::StructuredToolCalls,
        command_events,
    }
}

fn extract_json_lines(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .collect()
}

fn opencode_step_finish_event(json: &Value) -> bool {
    json.get("type") == Some(&Value::String("step_finish".to_string()))
}

fn opencode_tokens_from_event(json: &Value) -> Option<(u64, u64)> {
    let tokens = json.get("part").and_then(|p| p.get("tokens"))?;
    let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
    let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
    let reasoning = tokens
        .get("reasoning")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some((input + reasoning, output))
}

fn parse_opencode_token_usage(output: &str) -> Option<TokenUsage> {
    let mut total_input = 0u64;
    let mut total_output = 0u64;

    for line in extract_json_lines(output) {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if opencode_step_finish_event(&json) {
                if let Some((input, output)) = opencode_tokens_from_event(&json) {
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

fn parse_opencode_cost(output: &str) -> Option<f64> {
    let mut total = 0.0;

    for line in extract_json_lines(output) {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if opencode_step_finish_event(&json) {
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

fn opencode_command_event(event: &Value) -> Option<CommandEvent> {
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

fn extract_opencode_command_events(output: &str) -> Vec<CommandEvent> {
    extract_json_lines(output)
        .into_iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|event| opencode_command_event(&event))
        .collect()
}

fn synthesize_opencode_transcript(output: &str) -> String {
    let mut transcript = String::new();

    for line in extract_json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(command_event) = opencode_command_event(&event) else {
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

fn codex_tokens_from_turn_completed(json: &Value) -> Option<(u64, u64)> {
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

fn parse_codex_token_usage(output: &str) -> Option<TokenUsage> {
    let mut total_input = 0u64;
    let mut total_output = 0u64;

    for line in extract_json_lines(output) {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if json.get("type") == Some(&Value::String("turn.completed".to_string())) {
                if let Some((input, output)) = codex_tokens_from_turn_completed(&json) {
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

fn codex_item_type(item: &Value) -> Option<&str> {
    item.get("type")
        .and_then(Value::as_str)
        .or_else(|| item.get("item_type").and_then(Value::as_str))
}

fn codex_effective_exit_code(item: &Value) -> i32 {
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

fn codex_command_item(event: &Value) -> Option<&Value> {
    if event.get("type") != Some(&Value::String("item.completed".to_string())) {
        return None;
    }
    let item = event.get("item")?;
    if codex_item_type(item)? != "command_execution" {
        return None;
    }
    Some(item)
}

fn extract_codex_command_events(output: &str) -> Vec<CommandEvent> {
    let mut events = Vec::new();

    for line in extract_json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(item) = codex_command_item(&event) else {
            continue;
        };
        let command = item.get("command").and_then(Value::as_str).unwrap_or("");
        if command.is_empty() {
            continue;
        }
        events.push(CommandEvent {
            command: command.to_string(),
            exit_code: Some(codex_effective_exit_code(item)),
        });
    }

    events
}

fn synthesize_codex_transcript(output: &str) -> String {
    let mut transcript = String::new();

    for line in extract_json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(item) = codex_command_item(&event) else {
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
        transcript.push_str(&format!(
            "exit code: {}\n\n",
            codex_effective_exit_code(item)
        ));
    }

    transcript
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

fn claude_result_exit_code(item: &Value) -> Option<i32> {
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

fn add_claude_tool_use(
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

fn update_claude_tool_result(
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

    command_events[index].exit_code = claude_result_exit_code(item);
}

fn process_claude_message_content(
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
            Some("tool_use") => add_claude_tool_use(
                item.get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                item.get("name").and_then(Value::as_str),
                item.get("input"),
                tool_ids,
                seen_keys,
                command_events,
            ),
            Some("tool_result") => update_claude_tool_result(item, tool_ids, command_events),
            _ => {}
        }
    }
}

fn process_claude_stream_event(
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
            add_claude_tool_use(
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

fn extract_claude_command_events(output: &str) -> Vec<CommandEvent> {
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
            process_claude_message_content(
                message,
                &mut tool_ids,
                &mut seen_keys,
                &mut command_events,
            );
        }

        if value.get("type").and_then(Value::as_str) == Some("stream_event") {
            if let Some(event) = value.get("event") {
                process_claude_stream_event(
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

fn synthesize_transcript_from_command_events(events: &[CommandEvent]) -> String {
    let mut transcript = String::new();

    for event in events {
        transcript.push_str("$ ");
        transcript.push_str(&event.command);
        transcript.push('\n');
        transcript.push_str(&format!("exit code: {}\n\n", event.exit_code.unwrap_or(0)));
    }

    transcript
}

fn parse_claude_cost(output: &str) -> Option<f64> {
    output.lines().rev().find_map(|line| {
        let value = serde_json::from_str::<Value>(line).ok()?;
        value.get("total_cost_usd").and_then(Value::as_f64)
    })
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
    fn normalizes_codex_jsonl() {
        let output = codex_output(sample_codex_jsonl(), 0);

        assert_eq!(output.token_usage.unwrap().input, 170);
        assert_eq!(output.command_events.len(), 2);
        assert_eq!(output.command_events[0].command, "bash -lc ls");
        assert_eq!(output.command_events[1].exit_code, Some(1));
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

        let output = codex_output(jsonl_output, 0);
        assert!(output.transcript.contains("$ bash -lc pwd"));
        assert_eq!(output.command_events[0].exit_code, Some(0));
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

        let output = codex_output(jsonl_output, 0);
        assert!(output.transcript.contains("exit code: 1"));
        assert_eq!(output.command_events[0].exit_code, Some(1));
    }

    #[test]
    fn normalizes_opencode_json() {
        let output = opencode_output(sample_opencode_json(), 0);

        let usage = output.token_usage.unwrap();
        assert_eq!(usage.input, 28);
        assert_eq!(usage.output, 60);
        assert!((output.cost_usd.unwrap() - 0.167).abs() < 0.001);
        assert_eq!(output.command_events.len(), 3);
        assert_eq!(output.command_events[2].command, "./notes badcmd");
        assert_eq!(output.command_events[2].exit_code, Some(1));
        assert!(output.transcript.contains("Error: unknown command"));
    }

    #[test]
    fn opencode_skips_non_bash_tools() {
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

        let output = opencode_output(json_output, 0);
        assert!(output.transcript.is_empty());
        assert!(output.command_events.is_empty());
    }

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

        let output = claude_code_output(raw, 0);

        assert_eq!(output.command_events.len(), 1);
        assert_eq!(output.command_events[0].command, "notes add \"Hello\"");
        assert_eq!(output.command_events[0].exit_code, Some(0));
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

        let output = claude_code_output(raw, 0);
        assert_eq!(output.command_events[0].command, "notes list");
        assert_eq!(output.command_events[0].exit_code, Some(0));
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

        let output = claude_code_output(raw, 0);
        assert_eq!(output.command_events[0].exit_code, Some(1));
    }

    #[test]
    fn normalizers_handle_plain_text_without_panicking() {
        let codex = codex_output("just plain text".to_string(), 0);
        let opencode = opencode_output("just plain text".to_string(), 0);

        assert!(codex.token_usage.is_none());
        assert!(codex.transcript.is_empty());
        assert!(opencode.token_usage.is_none());
        assert!(opencode.cost_usd.is_none());
        assert!(opencode.transcript.is_empty());
    }
}
