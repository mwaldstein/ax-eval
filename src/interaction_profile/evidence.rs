use crate::interaction_profile::{
    AdapterEvidenceCapability, InteractionEvidenceSource, TargetInteractionSpec,
};
use crate::transcript::{CommandEvent, InteractionInput, TranscriptAnalyzer};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct ExtractedInteractionEvidence {
    pub target_events: Vec<CommandEvent>,
    pub source: InteractionEvidenceSource,
}

pub(crate) struct InteractionEvidenceInput<'a> {
    pub target: &'a TargetInteractionSpec,
    pub interaction_input: &'a InteractionInput,
    pub adapter_capability: AdapterEvidenceCapability,
    pub transcript_path: PathBuf,
}

enum RawInteractionEvidence {
    StructuredToolCalls(Vec<CommandEvent>),
    TranscriptRegexFallback { transcript_path: PathBuf },
}

impl RawInteractionEvidence {
    fn from_interaction_input(
        interaction_input: &InteractionInput,
        adapter_capability: AdapterEvidenceCapability,
        transcript_path: impl Into<PathBuf>,
    ) -> Result<Self> {
        match (interaction_input, adapter_capability) {
            (
                InteractionInput::StructuredToolCalls(command_events),
                AdapterEvidenceCapability::StructuredToolCalls,
            ) => Ok(Self::StructuredToolCalls(command_events.clone())),
            (
                InteractionInput::StructuredToolCalls(_),
                AdapterEvidenceCapability::TranscriptRegexFallback,
            ) => {
                anyhow::bail!(
                    "Adapter does not declare structured tool-call support but returned structured tool calls"
                )
            }
            (
                InteractionInput::TranscriptRegex,
                AdapterEvidenceCapability::TranscriptRegexFallback,
            ) => Ok(Self::TranscriptRegexFallback {
                transcript_path: transcript_path.into(),
            }),
            (InteractionInput::TranscriptRegex, AdapterEvidenceCapability::StructuredToolCalls) => {
                anyhow::bail!(
                    "Adapter supports structured tool calls but returned transcript regex evidence"
                )
            }
        }
    }
}

pub(crate) fn extract_target_interaction_evidence(
    input: InteractionEvidenceInput<'_>,
) -> Result<ExtractedInteractionEvidence> {
    let evidence = RawInteractionEvidence::from_interaction_input(
        input.interaction_input,
        input.adapter_capability,
        input.transcript_path,
    )?;

    match evidence {
        RawInteractionEvidence::StructuredToolCalls(command_events) => {
            Ok(ExtractedInteractionEvidence {
                target_events: target_command_events(&command_events, input.target),
                source: InteractionEvidenceSource::StructuredToolCalls,
            })
        }
        RawInteractionEvidence::TranscriptRegexFallback { transcript_path } => {
            let content = std::fs::read_to_string(&transcript_path)
                .with_context(|| "Failed to read transcript file for regex interaction profile")?;
            Ok(ExtractedInteractionEvidence {
                target_events: extract_transcript_target_events(&content, input.target),
                source: InteractionEvidenceSource::TranscriptRegexFallback,
            })
        }
    }
}

fn extract_transcript_target_events(
    transcript: &str,
    target: &TargetInteractionSpec,
) -> Vec<CommandEvent> {
    if target
        .command_pattern()
        .is_none_or(|pattern| pattern.trim().is_empty())
    {
        let events = TranscriptAnalyzer::extract_command_lines_with_exit_codes(transcript);
        return target_command_events(&events, target);
    }

    let pattern = resolve_command_pattern(target.binary(), target.command_pattern());
    TranscriptAnalyzer::extract_commands_with_pattern(transcript, &pattern)
}

fn resolve_command_pattern(target_binary: &str, command_pattern: Option<&str>) -> String {
    if let Some(pattern) = command_pattern {
        if !pattern.trim().is_empty() {
            return pattern.to_string();
        }
    }

    format!(r"^\s*({})\s+(--help|\S+)\b", regex::escape(target_binary))
}

fn target_command_events(
    events: &[CommandEvent],
    target: &TargetInteractionSpec,
) -> Vec<CommandEvent> {
    events
        .iter()
        .filter_map(|event| {
            target_subcommand(&event.command, target.binary()).map(|command| CommandEvent {
                command,
                exit_code: event.exit_code,
            })
        })
        .collect()
}

fn target_subcommand(command: &str, target_binary: &str) -> Option<String> {
    let tokens = shell_like_tokens(command);
    let target = Path::new(target_binary)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(target_binary);

    for (index, token) in tokens.iter().enumerate() {
        if token.contains(char::is_whitespace) {
            if let Some(subcommand) = target_subcommand(token, target_binary) {
                return Some(subcommand);
            }
        }

        if token == "$" {
            continue;
        }

        let token_binary = Path::new(token)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(token);

        if token_binary == target {
            let subcommand = tokens
                .get(index + 1)
                .map(String::as_str)
                .unwrap_or("command");
            if subcommand == "--help" || tokens[index + 1..].iter().any(|arg| arg == "--help") {
                return Some("help".to_string());
            }
            return Some(subcommand.to_string());
        }
    }

    None
}

fn shell_like_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if let Some(quote_char) = quote {
            if ch == quote_char {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }

        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }

        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }

        current.push(ch);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> TargetInteractionSpec {
        TargetInteractionSpec::new("notes", None)
    }

    #[test]
    fn structured_evidence_extracts_target_commands_without_transcript_regex() {
        let events = vec![
            CommandEvent {
                command: "ls".to_string(),
                exit_code: Some(0),
            },
            CommandEvent {
                command: "./notes init".to_string(),
                exit_code: Some(0),
            },
            CommandEvent {
                command: "bash -lc './notes add \"Hello\"'".to_string(),
                exit_code: Some(1),
            },
            CommandEvent {
                command: "/tmp/work/notes list --help".to_string(),
                exit_code: Some(0),
            },
        ];
        let target = target();

        let extracted = extract_target_interaction_evidence(InteractionEvidenceInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: PathBuf::from("unused-transcript.raw.txt"),
        })
        .expect("evidence");

        let commands = extracted
            .target_events
            .iter()
            .map(|event| (event.command.as_str(), event.exit_code))
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![("init", Some(0)), ("add", Some(1)), ("help", Some(0))]
        );
    }

    #[test]
    fn regex_fallback_extracts_default_target_commands() {
        let target = target();
        let transcript = "$ ./notes init\nexit code: 0\n\
                          $ bash -lc './notes add \"Hello\"'\nexit code: 1\n\
                          $ /tmp/work/notes list --help\nexit code: 0\n";

        let events = extract_transcript_target_events(transcript, &target);

        let commands = events
            .iter()
            .map(|event| (event.command.as_str(), event.exit_code))
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![("init", Some(0)), ("add", Some(1)), ("help", Some(0))]
        );
    }

    #[test]
    fn regex_fallback_honors_custom_command_pattern() {
        let target = TargetInteractionSpec::new("notes", Some("tool:notes action=(\\S+)".into()));
        let transcript = "tool:notes action=sync\nexit code: 0\ntool:other action=skip\n";

        let events = extract_transcript_target_events(transcript, &target);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].command, "sync");
    }
}
