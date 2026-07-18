use crate::adapter::normalize::{json_lines, output_from_structured_parts};
use crate::adapter::{TokenUsage, ToolRunOutput};
use crate::interaction_evidence::{CommandEvent, McpToolCallEvent};
use serde_json::Value;

pub(crate) fn normalize(output: String, exit_code: i32) -> ToolRunOutput {
    let token_usage = parse_token_usage(&output);
    let cost = parse_cost(&output);
    let command_events = extract_command_events(&output);
    let mcp_events = extract_mcp_events(&output);
    let transcript = synthesize_transcript(&output);

    output_from_structured_parts(
        output,
        exit_code,
        transcript,
        cost,
        token_usage,
        command_events,
        mcp_events,
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

fn mcp_is_error(part: &Value) -> bool {
    let state = part.get("state");
    let metadata = state.and_then(|s| s.get("metadata"));
    let status = state
        .and_then(|s| s.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("");

    matches!(status, "error" | "failed")
        || state.and_then(|s| s.get("error")).is_some()
        || metadata
            .and_then(|m| m.get("is_error").or_else(|| m.get("isError")))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || metadata.and_then(|m| m.get("error")).is_some()
}

fn mcp_arguments(input: &Value) -> Value {
    input
        .get("arguments")
        .or_else(|| input.get("args"))
        .or_else(|| input.get("input"))
        .cloned()
        .unwrap_or(Value::Null)
}

fn mcp_event(event: &Value) -> Option<McpToolCallEvent> {
    let part = event.get("part")?;
    if part.get("type").and_then(Value::as_str)? != "tool" {
        return None;
    }

    // upstream shape: opencode emits MCP invocations as tool_use events whose
    // part.tool is "mcp" and whose state.input carries server, tool, and
    // arguments/input JSON; state/metadata carries status or error fields.
    if part.get("tool").and_then(Value::as_str)? != "mcp" {
        return None;
    }

    let input = part.get("state").and_then(|s| s.get("input"))?;
    let server = input
        .get("server")
        .or_else(|| input.get("server_name"))
        .and_then(Value::as_str)?;
    let tool = input
        .get("tool")
        .or_else(|| input.get("tool_name"))
        .or_else(|| input.get("name"))
        .and_then(Value::as_str)?;

    if server.trim().is_empty() || tool.trim().is_empty() {
        return None;
    }

    Some(McpToolCallEvent {
        server: server.to_string(),
        tool: tool.to_string(),
        arguments: mcp_arguments(input),
        is_error: mcp_is_error(part),
        duration_ms: None,
    })
}

fn extract_command_events(output: &str) -> Vec<CommandEvent> {
    json_lines(output)
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|event| command_event(&event))
        .collect()
}

fn extract_mcp_events(output: &str) -> Vec<McpToolCallEvent> {
    json_lines(output)
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|event| mcp_event(&event))
        .collect()
}

fn text_event_payload(event: &Value) -> Option<&str> {
    if event.get("type").and_then(Value::as_str) != Some("text") {
        return None;
    }
    let part = event.get("part")?;
    if part.get("type").and_then(Value::as_str) != Some("text") {
        return None;
    }
    part.get("text").and_then(Value::as_str)
}

fn synthesize_transcript(output: &str) -> String {
    let mut transcript = String::new();

    for line in json_lines(output) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if let Some(text) = text_event_payload(&event) {
            if !text.is_empty() {
                transcript.push_str(text);
                transcript.push_str("\n\n");
            }
            continue;
        }

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

    const SESSION: &str = "ses_test0000000000000000000000000";

    /// Build NDJSON from events, one per line — the format opencode emits on stdout.
    fn ndjson(events: &[serde_json::Value]) -> String {
        events
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn step_start(n: u64) -> serde_json::Value {
        serde_json::json!({
            "type": "step_start",
            "timestamp": 1000 * n,
            "sessionID": SESSION,
            "part": {
                "id": format!("prt_step_start_{n}"),
                "messageID": format!("msg_{n}"),
                "sessionID": SESSION,
                "snapshot": "abc123",
                "type": "step-start"
            }
        })
    }

    fn text_event(n: u64, text: &str, phase: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "text",
            "timestamp": 1000 * n + 100,
            "sessionID": SESSION,
            "part": {
                "id": format!("prt_text_{n}"),
                "messageID": format!("msg_{n}"),
                "sessionID": SESSION,
                "type": "text",
                "text": text,
                "time": {"start": 1000 * n + 50, "end": 1000 * n + 90},
                "metadata": {"openai": {"itemId": format!("msg_item_{n}"), "phase": phase}}
            }
        })
    }

    fn bash_event(
        n: u64,
        command: &str,
        output: &str,
        exit: i64,
        description: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "type": "tool_use",
            "timestamp": 1000 * n + 200,
            "sessionID": SESSION,
            "part": {
                "type": "tool",
                "tool": "bash",
                "callID": format!("call_{n}"),
                "state": {
                    "status": "completed",
                    "input": {"command": command, "timeout": 120000, "workdir": "/workspace", "description": description},
                    "output": output,
                    "metadata": {"output": output, "exit": exit, "description": description, "truncated": false},
                    "title": description,
                    "time": {"start": 1000 * n + 150, "end": 1000 * n + 180}
                },
                "metadata": {"openai": {"itemId": format!("fc_item_{n}")}},
                "id": format!("prt_tool_{n}"),
                "sessionID": SESSION,
                "messageID": format!("msg_{n}")
            }
        })
    }

    fn mcp_event(
        n: u64,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
        status: &str,
        is_error: bool,
    ) -> serde_json::Value {
        serde_json::json!({
            "type": "tool_use",
            "timestamp": 1000 * n + 250,
            "sessionID": SESSION,
            "part": {
                "type": "tool",
                "tool": "mcp",
                "callID": format!("mcp_call_{n}"),
                "state": {
                    "status": status,
                    "input": {"server": server, "tool": tool, "arguments": arguments},
                    "output": if is_error { "tool failed" } else { "ok" },
                    "metadata": {"is_error": is_error}
                },
                "id": format!("prt_mcp_{n}"),
                "sessionID": SESSION,
                "messageID": format!("msg_{n}")
            }
        })
    }

    fn step_finish(
        n: u64,
        input: u64,
        output: u64,
        reasoning: u64,
        cost: f64,
        reason: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "type": "step_finish",
            "timestamp": 1000 * n + 300,
            "sessionID": SESSION,
            "part": {
                "id": format!("prt_finish_{n}"),
                "reason": reason,
                "snapshot": "abc123",
                "messageID": format!("msg_{n}"),
                "sessionID": SESSION,
                "type": "step-finish",
                "tokens": {
                    "total": input + output + reasoning,
                    "input": input,
                    "output": output,
                    "reasoning": reasoning,
                    "cache": {"write": 0, "read": 0}
                },
                "cost": cost
            }
        })
    }

    /// Realistic multi-step opencode run: 3 steps with commentary text and bash commands.
    fn sample_opencode_json() -> String {
        ndjson(&[
            // Step 1: init
            step_start(1),
            text_event(1, "I'll initialize the database first.", "commentary"),
            bash_event(1, "./notes init", "Initialized", 0, "init db"),
            step_finish(1, 10, 20, 5, 0.042, "tool-calls"),
            // Step 2: add note
            step_start(2),
            text_event(2, "Now I'll add a note.", "commentary"),
            bash_event(2, "./notes add \"Hello\"", "Created note 1", 0, "add note"),
            step_finish(2, 5, 15, 0, 0.058, "tool-calls"),
            // Step 3: bad command
            step_start(3),
            bash_event(
                3,
                "./notes badcmd",
                "Error: unknown command",
                1,
                "bad command",
            ),
            step_finish(3, 8, 25, 0, 0.067, "stop"),
        ])
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
        assert!(
            output
                .transcript
                .contains("I'll initialize the database first."),
            "transcript should include model text events: {}",
            output.transcript
        );
        assert!(
            output.transcript.contains("$ ./notes init"),
            "transcript should include command events: {}",
            output.transcript
        );
    }

    #[test]
    fn skips_non_bash_tools() {
        let json_output = serde_json::json!({
            "type": "tool_use",
            "timestamp": 1000,
            "sessionID": SESSION,
            "part": {
                "type": "tool",
                "tool": "read",
                "callID": "call_read_1",
                "state": {
                    "status": "completed",
                    "input": {"filePath": "/foo"},
                    "output": "file contents",
                    "metadata": {"truncated": false}
                },
                "id": "prt_read_1",
                "sessionID": SESSION,
                "messageID": "msg_1"
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
    fn normalizes_successful_opencode_mcp_tool_call() {
        let output = normalize(
            ndjson(&[mcp_event(
                1,
                "todo",
                "add",
                serde_json::json!({"text": "hello"}),
                "completed",
                false,
            )]),
            0,
        );

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
    }

    #[test]
    fn normalizes_failed_opencode_mcp_tool_call() {
        let output = normalize(
            ndjson(&[mcp_event(
                1,
                "todo",
                "complete",
                serde_json::json!({"id": 99}),
                "failed",
                true,
            )]),
            0,
        );

        let mcp_events = output
            .mcp_tool_call_events()
            .expect("structured mcp tool call events");
        assert_eq!(mcp_events[0].tool, "complete");
        assert!(mcp_events[0].is_error);
    }

    #[test]
    fn normalizes_mixed_opencode_bash_and_mcp_events() {
        let output = normalize(
            ndjson(&[
                bash_event(1, "./notes list", "[]", 0, "list notes"),
                mcp_event(
                    2,
                    "todo",
                    "add",
                    serde_json::json!({"text": "hello"}),
                    "completed",
                    false,
                ),
            ]),
            0,
        );

        assert_eq!(
            output.command_events().expect("structured command events")[0].command,
            "./notes list"
        );
        assert_eq!(
            output
                .mcp_tool_call_events()
                .expect("structured mcp tool call events")[0]
                .tool,
            "add"
        );
    }

    #[test]
    fn does_not_treat_non_mcp_opencode_tool_as_mcp_event() {
        let output = normalize(
            ndjson(&[serde_json::json!({
                "type": "tool_use",
                "part": {
                    "type": "tool",
                    "tool": "read",
                    "state": {"input": {"server": "todo", "tool": "add"}}
                }
            })]),
            0,
        );

        assert!(output.mcp_tool_call_events().unwrap().is_empty());
    }

    #[test]
    fn handles_plain_text_without_panicking() {
        let output = normalize("just plain text".to_string(), 0);

        assert!(output.token_usage.is_none());
        assert!(output.cost_usd.is_none());
        assert!(output.transcript.is_empty());
    }

    #[test]
    fn handles_malformed_opencode_json_without_panicking() {
        let output = normalize("{\"type\":\"tool_use\"\nnot json".to_string(), 0);

        assert!(output.command_events().unwrap().is_empty());
        assert!(output.mcp_tool_call_events().unwrap().is_empty());
    }

    /// Regression test: a judge run emits only text (no bash commands). The
    /// `<judge_result>` JSON is escaped inside the text event's "text" field.
    /// The normalizer must extract the unescaped text into the transcript so
    /// `parse_judge_response` can find the envelope.
    #[test]
    fn extracts_judge_result_from_text_only_run() {
        let judge_json = serde_json::json!({
            "scores": {"task_completion": 0.9},
            "weighted_score": 0.9,
            "confidence": 0.85,
            "issues": [],
            "highlights": ["Clean work"],
            "rationale": "Good."
        });

        let ndjson = ndjson(&[
            step_start(1),
            text_event(
                1,
                &format!("<judge_result>{judge_json}</judge_result>"),
                "final_answer",
            ),
            step_finish(1, 100, 200, 0, 0.05, "stop"),
        ]);

        let output = normalize(ndjson, 0);

        assert!(
            output.transcript.contains("<judge_result>"),
            "transcript should contain unescaped judge_result tag: {}",
            output.transcript
        );
        assert!(
            output.transcript.contains("\"weighted_score\":0.9"),
            "transcript should contain unescaped judge JSON: {}",
            output.transcript
        );
        assert!(output.command_events().unwrap().is_empty());
    }
}
