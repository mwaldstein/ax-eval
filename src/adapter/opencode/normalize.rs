use crate::adapter::normalize::{json_lines, output_from_parts};
use crate::adapter::{TokenUsage, ToolRunOutput};
use crate::transcript::CommandEvent;
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
