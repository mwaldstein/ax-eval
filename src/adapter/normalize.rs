use crate::adapter::{TokenUsage, ToolRunOutput};
use crate::interaction_evidence::{CommandEvent, InteractionInput};

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
    use super::*;

    #[test]
    fn json_lines_skips_non_json_lines() {
        let lines = json_lines("plain\n {\"ok\":true}\n[]\n{\"ok\":false}").collect::<Vec<_>>();

        assert_eq!(lines, vec![" {\"ok\":true}", "{\"ok\":false}"]);
    }

    #[test]
    fn output_from_parts_uses_structured_command_events() {
        let output = output_from_parts(
            "raw".to_string(),
            0,
            "transcript".to_string(),
            Some(0.25),
            Some(TokenUsage {
                input: 10,
                output: 5,
            }),
            vec![CommandEvent {
                command: "notes list".to_string(),
                exit_code: Some(0),
            }],
        );

        assert_eq!(output.raw_output.as_deref(), Some("raw"));
        assert_eq!(output.transcript, "transcript");
        assert_eq!(output.cost_usd, Some(0.25));
        assert_eq!(output.token_usage.as_ref().unwrap().input, 10);
        assert_eq!(
            output.command_events().expect("command events")[0].command,
            "notes list"
        );
    }

    #[test]
    fn transcript_from_command_events_writes_shell_style_events() {
        let transcript = transcript_from_command_events(&[CommandEvent {
            command: "notes add hello".to_string(),
            exit_code: Some(7),
        }]);

        assert_eq!(transcript, "$ notes add hello\nexit code: 7\n\n");
    }
}
