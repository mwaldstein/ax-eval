use crate::transcript::{CommandEvent, EfficiencyMetrics, InteractionInput, TranscriptAnalyzer};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEvidenceSource {
    StructuredToolCalls,
    TranscriptRegexFallback,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractionProfile {
    pub metrics: EfficiencyMetrics,
    pub evidence_source: InteractionEvidenceSource,
}

pub struct InteractionProfileInput<'a> {
    pub env_root: &'a Path,
    pub target_binary: &'a str,
    pub command_pattern: Option<&'a str>,
    pub interaction_input: &'a InteractionInput,
    pub completed: bool,
    pub supports_structured_tool_calls: bool,
}

pub(crate) fn reduce_command_events(events: &[CommandEvent]) -> EfficiencyMetrics {
    let commands = events
        .iter()
        .map(|event| {
            let is_error = event.exit_code.map(|code| code != 0).unwrap_or(false);
            (event.command.clone(), is_error)
        })
        .collect::<Vec<_>>();

    let total_commands = commands.len();
    let error_count = commands.iter().filter(|(_, e)| *e).count();
    let help_invocations = commands.iter().filter(|(c, _)| c == "help").count();

    let unique_commands = commands
        .iter()
        .map(|(c, _)| c.clone())
        .collect::<std::collections::HashSet<_>>();
    let retry_count = total_commands.saturating_sub(unique_commands.len());

    let mut seen_first: HashMap<String, bool> = HashMap::new();
    let mut first_try_success_count: usize = 0;

    for (cmd, is_error) in &commands {
        if !seen_first.contains_key(cmd) {
            seen_first.insert(cmd.clone(), !is_error);
            if !is_error {
                first_try_success_count += 1;
            }
        }
    }

    let first_try_success_rate = if total_commands > 0 {
        first_try_success_count as f64 / total_commands as f64
    } else {
        0.0
    };

    let iteration_ratio = if total_commands > 0 {
        unique_commands.len() as f64 / total_commands as f64
    } else {
        0.0
    };

    EfficiencyMetrics {
        total_commands,
        unique_commands: unique_commands.len(),
        error_count,
        retry_count,
        help_invocations,
        first_try_success_rate,
        iteration_ratio,
        completed: false,
    }
}

fn target_command_events(events: &[CommandEvent], target_binary: &str) -> Vec<CommandEvent> {
    events
        .iter()
        .filter_map(|event| {
            target_subcommand(&event.command, target_binary).map(|command| CommandEvent {
                command,
                exit_code: event.exit_code,
            })
        })
        .collect()
}

fn target_subcommand(command: &str, target_binary: &str) -> Option<String> {
    let tokens = shell_like_tokens(command);
    let target = std::path::Path::new(target_binary)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(target_binary);

    for (index, token) in tokens.iter().enumerate() {
        if token.contains(char::is_whitespace) {
            if let Some(subcommand) = target_subcommand(token, target_binary) {
                return Some(subcommand);
            }
        }

        let token_binary = std::path::Path::new(token)
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

pub fn build_interaction_profile(input: InteractionProfileInput<'_>) -> Result<InteractionProfile> {
    let (mut metrics, evidence_source) = match input.interaction_input {
        InteractionInput::StructuredToolCalls(command_events) => {
            let target_events = target_command_events(command_events, input.target_binary);
            let metrics = reduce_command_events(&target_events);
            (metrics, InteractionEvidenceSource::StructuredToolCalls)
        }
        InteractionInput::TranscriptRegex => {
            if input.supports_structured_tool_calls {
                anyhow::bail!(
                    "Adapter supports structured tool calls but returned transcript regex evidence"
                );
            }

            let transcript_path = input.env_root.join("transcript.raw.txt");
            let content = std::fs::read_to_string(&transcript_path)
                .with_context(|| "Failed to read transcript file for regex interaction profile")?;
            let command_events = TranscriptAnalyzer::extract_command_events_for_target(
                &content,
                input.target_binary,
                input.command_pattern,
            );
            let metrics = reduce_command_events(&command_events);
            (metrics, InteractionEvidenceSource::TranscriptRegexFallback)
        }
    };

    metrics.completed = input.completed;

    if input.supports_structured_tool_calls
        && input.completed
        && matches!(
            input.interaction_input,
            InteractionInput::StructuredToolCalls(_)
        )
        && metrics.total_commands == 0
    {
        anyhow::bail!(
            "Adapter supports structured tool calls but returned no usable structured target-tool events"
        );
    }

    Ok(InteractionProfile {
        metrics,
        evidence_source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::CommandEvent;

    #[test]
    fn structured_evidence_builds_profile() {
        let temp = tempfile::tempdir().expect("tempdir");
        let events = vec![CommandEvent {
            command: "notes add hello".to_string(),
            exit_code: Some(0),
        }];

        let profile = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            completed: true,
            supports_structured_tool_calls: true,
        })
        .expect("profile");

        assert_eq!(
            profile.evidence_source,
            InteractionEvidenceSource::StructuredToolCalls
        );
        assert_eq!(profile.metrics.total_commands, 1);
        assert!(profile.metrics.completed);
    }

    #[test]
    fn structured_evidence_reduces_target_commands_without_transcript_regex() {
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
                command: "/tmp/work/notes add \"Hello\"".to_string(),
                exit_code: Some(0),
            },
            CommandEvent {
                command: "./notes list --help".to_string(),
                exit_code: Some(0),
            },
        ];

        let profile = build_interaction_profile(InteractionProfileInput {
            env_root: Path::new("/unused"),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            completed: true,
            supports_structured_tool_calls: true,
        })
        .expect("profile");

        assert_eq!(profile.metrics.total_commands, 4);
        assert_eq!(profile.metrics.unique_commands, 3);
        assert_eq!(profile.metrics.error_count, 1);
        assert_eq!(profile.metrics.retry_count, 1);
        assert_eq!(profile.metrics.help_invocations, 1);
        assert_eq!(profile.metrics.first_try_success_rate, 0.5);
    }

    #[test]
    fn structured_capable_adapter_cannot_use_regex_fallback() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("transcript.raw.txt"), "notes add\n").expect("write");

        let error = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::TranscriptRegex,
            completed: true,
            supports_structured_tool_calls: true,
        })
        .unwrap_err();

        assert!(error.to_string().contains("returned transcript regex"));
    }

    #[test]
    fn completed_structured_run_requires_target_tool_events() {
        let temp = tempfile::tempdir().expect("tempdir");
        let events = vec![CommandEvent {
            command: "ls -la".to_string(),
            exit_code: Some(0),
        }];

        let error = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            completed: true,
            supports_structured_tool_calls: true,
        })
        .unwrap_err();

        assert!(error.to_string().contains("no usable structured"));
    }

    #[test]
    fn regex_fallback_is_available_for_non_structured_adapters() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("transcript.raw.txt"),
            "notes add hello\nexit code: 0\n",
        )
        .expect("write");

        let profile = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::TranscriptRegex,
            completed: true,
            supports_structured_tool_calls: false,
        })
        .expect("profile");

        assert_eq!(
            profile.evidence_source,
            InteractionEvidenceSource::TranscriptRegexFallback
        );
        assert_eq!(profile.metrics.total_commands, 1);
    }
}
