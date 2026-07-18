use crate::interaction_evidence::{CommandEvent, McpToolCallEvent};
use crate::interaction_profile::TargetInteractionSpec;
use crate::transcript::TranscriptAnalyzer;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    Success,
    Failure,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetAction {
    pub action: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetActionProjection {
    pub actions: Vec<TargetAction>,
    pub warnings: Vec<String>,
}

pub(crate) fn structured_target_actions(
    events: &[CommandEvent],
    target: &TargetInteractionSpec,
) -> TargetActionProjection {
    let TargetInteractionSpec::Cli { .. } = target else {
        return TargetActionProjection {
            actions: Vec::new(),
            warnings: vec!["CLI tool-call evidence was returned for an MCP target".to_string()],
        };
    };

    let actions = normalize_target_events(events, target)
        .into_iter()
        .map(|event| TargetAction {
            action: event.command,
            outcome: cli_outcome(event.exit_code),
        })
        .collect();

    TargetActionProjection {
        actions,
        warnings: Vec::new(),
    }
}

pub(crate) fn structured_mcp_target_actions(
    events: &[McpToolCallEvent],
    target: &TargetInteractionSpec,
) -> TargetActionProjection {
    let TargetInteractionSpec::Mcp { server, tools } = target else {
        return TargetActionProjection {
            actions: Vec::new(),
            warnings: vec!["MCP tool-call evidence was returned for a CLI target".to_string()],
        };
    };

    let mut warnings = Vec::new();
    let actions = events
        .iter()
        .filter(|event| event.server == *server)
        .map(|event| {
            if !tools.contains(&event.tool) {
                warnings.push(format!(
                    "MCP target server '{}' called undeclared tool '{}'",
                    server, event.tool
                ));
            }
            TargetAction {
                action: event.tool.clone(),
                outcome: if event.is_error {
                    Outcome::Failure
                } else {
                    Outcome::Success
                },
            }
        })
        .collect();

    TargetActionProjection { actions, warnings }
}

pub(crate) fn transcript_target_actions(
    transcript: &str,
    target: &TargetInteractionSpec,
) -> TargetActionProjection {
    let TargetInteractionSpec::Cli { .. } = target else {
        return TargetActionProjection {
            actions: Vec::new(),
            warnings: Vec::new(),
        };
    };

    if target
        .command_pattern()
        .is_none_or(|pattern| pattern.trim().is_empty())
    {
        let events = TranscriptAnalyzer::extract_command_lines_with_exit_codes(transcript);
        return TargetActionProjection {
            actions: normalize_target_events(&events, target)
                .into_iter()
                .map(|event| TargetAction {
                    action: event.command,
                    outcome: cli_outcome(event.exit_code),
                })
                .collect(),
            warnings: Vec::new(),
        };
    }

    let pattern = resolve_command_pattern(target.binary(), target.command_pattern());
    TargetActionProjection {
        actions: TranscriptAnalyzer::extract_commands_with_pattern(transcript, &pattern)
            .into_iter()
            .map(|event| TargetAction {
                action: event.command,
                outcome: cli_outcome(event.exit_code),
            })
            .collect(),
        warnings: Vec::new(),
    }
}

fn cli_outcome(exit_code: Option<i32>) -> Outcome {
    match exit_code {
        Some(0) => Outcome::Success,
        Some(_) => Outcome::Failure,
        None => Outcome::Unknown,
    }
}

fn normalize_target_events(
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

fn resolve_command_pattern(target_binary: &str, command_pattern: Option<&str>) -> String {
    if let Some(pattern) = command_pattern {
        if !pattern.trim().is_empty() {
            return pattern.to_string();
        }
    }

    format!(r"^\s*({})\s+(--help|\S+)\b", regex::escape(target_binary))
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
    fn structured_events_extract_target_commands() {
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

        let commands = structured_target_actions(&events, &target)
            .actions
            .iter()
            .map(|event| (event.action.clone(), event.outcome))
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![
                ("init".to_string(), Outcome::Success),
                ("add".to_string(), Outcome::Failure),
                ("help".to_string(), Outcome::Success)
            ]
        );
    }

    #[test]
    fn transcript_events_use_same_default_target_rules() {
        let target = target();
        let transcript = "$ ./notes init\nexit code: 0\n\
                          $ bash -lc './notes add \"Hello\"'\nexit code: 1\n\
                          $ /tmp/work/notes list --help\nexit code: 0\n";

        let commands = transcript_target_actions(transcript, &target)
            .actions
            .iter()
            .map(|event| (event.action.clone(), event.outcome))
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![
                ("init".to_string(), Outcome::Success),
                ("add".to_string(), Outcome::Failure),
                ("help".to_string(), Outcome::Success)
            ]
        );
    }

    #[test]
    fn transcript_events_honor_custom_command_pattern() {
        let target = TargetInteractionSpec::new("notes", Some("tool:notes action=(\\S+)".into()));
        let transcript = "tool:notes action=sync\nexit code: 0\ntool:other action=skip\n";

        let events = transcript_target_actions(transcript, &target).actions;

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "sync");
    }

    #[test]
    fn mcp_events_match_server_and_tool_allow_list() {
        let target =
            TargetInteractionSpec::mcp("todo", vec!["add".to_string(), "list".to_string()]);
        let events = vec![
            McpToolCallEvent {
                server: "todo".to_string(),
                tool: "add".to_string(),
                arguments: serde_json::json!({"text":"x"}),
                is_error: false,
                duration_ms: Some(10),
            },
            McpToolCallEvent {
                server: "other".to_string(),
                tool: "add".to_string(),
                arguments: serde_json::json!({}),
                is_error: false,
                duration_ms: None,
            },
            McpToolCallEvent {
                server: "todo".to_string(),
                tool: "delete".to_string(),
                arguments: serde_json::json!({}),
                is_error: true,
                duration_ms: None,
            },
        ];

        let projection = structured_mcp_target_actions(&events, &target);

        assert_eq!(
            projection.actions,
            vec![
                TargetAction {
                    action: "add".to_string(),
                    outcome: Outcome::Success,
                },
                TargetAction {
                    action: "delete".to_string(),
                    outcome: Outcome::Failure,
                }
            ]
        );
        assert_eq!(projection.warnings.len(), 1);
        assert!(projection.warnings[0].contains("undeclared tool 'delete'"));
    }
}
