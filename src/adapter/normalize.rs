use crate::adapter::{TokenUsage, ToolRunOutput};
use crate::transcript::{CommandEvent, InteractionInput};

pub(crate) fn json_lines(output: &str) -> impl Iterator<Item = &str> {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
}

pub(crate) fn output_from_parts(
    raw_output: String,
    exit_code: i32,
    transcript: String,
    cost_usd: Option<f64>,
    token_usage: Option<TokenUsage>,
    command_events: Vec<CommandEvent>,
) -> ToolRunOutput {
    ToolRunOutput {
        transcript,
        raw_output: Some(raw_output),
        exit_code,
        cost_usd,
        token_usage,
        interaction_input: InteractionInput::StructuredToolCalls(command_events),
    }
}

pub(crate) fn transcript_from_command_events(events: &[CommandEvent]) -> String {
    let mut transcript = String::new();

    for event in events {
        transcript.push_str("$ ");
        transcript.push_str(&event.command);
        transcript.push('\n');
        transcript.push_str(&format!("exit code: {}\n\n", event.exit_code.unwrap_or(0)));
    }

    transcript
}

#[cfg(test)]
mod tests {
    use crate::adapter::{claude_code, codex, opencode};

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
        let output = codex::normalize::normalize(sample_codex_jsonl(), 0);
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

        let output = codex::normalize::normalize(jsonl_output, 0);
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

        let output = codex::normalize::normalize(jsonl_output, 0);
        let command_events = output.command_events().expect("structured command events");
        assert!(output.transcript.contains("exit code: 1"));
        assert_eq!(command_events[0].exit_code, Some(1));
    }

    #[test]
    fn normalizes_opencode_json() {
        let output = opencode::normalize::normalize(sample_opencode_json(), 0);
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

        let output = opencode::normalize::normalize(json_output, 0);
        assert!(output.transcript.is_empty());
        assert!(output
            .command_events()
            .expect("structured command events")
            .is_empty());
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

        let output = claude_code::normalize::normalize(raw, 0);
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

        let output = claude_code::normalize::normalize(raw, 0);
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

        let output = claude_code::normalize::normalize(raw, 0);
        let command_events = output.command_events().expect("structured command events");
        assert_eq!(command_events[0].exit_code, Some(1));
    }

    #[test]
    fn normalizers_handle_plain_text_without_panicking() {
        let codex = codex::normalize::normalize("just plain text".to_string(), 0);
        let opencode = opencode::normalize::normalize("just plain text".to_string(), 0);

        assert!(codex.token_usage.is_none());
        assert!(codex.transcript.is_empty());
        assert!(opencode.token_usage.is_none());
        assert!(opencode.cost_usd.is_none());
        assert!(opencode.transcript.is_empty());
    }
}
