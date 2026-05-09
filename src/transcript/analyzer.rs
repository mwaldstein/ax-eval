use crate::transcript::types::{CommandEvent, EfficiencyMetrics};
use regex::Regex;
use std::collections::HashMap;

pub struct TranscriptAnalyzer;

const DEFAULT_COMMAND_PATTERN: &str = r"^\s*([a-z][a-z0-9_.-]*)\s+(--help|[a-z][a-z0-9_-]*)\b";

impl TranscriptAnalyzer {
    #[allow(dead_code)]
    pub fn analyze(transcript: &str) -> EfficiencyMetrics {
        Self::analyze_with_pattern(transcript, DEFAULT_COMMAND_PATTERN)
    }

    #[allow(dead_code)]
    pub fn analyze_for_target(
        transcript: &str,
        target_binary: &str,
        command_pattern: Option<&str>,
    ) -> EfficiencyMetrics {
        let pattern = Self::resolve_command_pattern(target_binary, command_pattern);
        Self::analyze_with_pattern(transcript, &pattern)
    }

    #[allow(dead_code)]
    pub fn analyze_with_exit_codes(transcript: &str) -> EfficiencyMetrics {
        let commands = Self::extract_commands_with_pattern(transcript, DEFAULT_COMMAND_PATTERN);
        Self::analyze_with_events(transcript, Some(commands))
    }

    pub fn analyze_with_exit_codes_for_target(
        transcript: &str,
        target_binary: &str,
        command_pattern: Option<&str>,
    ) -> EfficiencyMetrics {
        let pattern = Self::resolve_command_pattern(target_binary, command_pattern);
        let commands = Self::extract_commands_with_pattern(transcript, &pattern);
        Self::analyze_with_events(transcript, Some(commands))
    }

    pub fn analyze_command_events_for_target(
        events: &[CommandEvent],
        target_binary: &str,
    ) -> EfficiencyMetrics {
        let target_events = Self::target_command_events(events, target_binary);
        Self::analyze_command_events(&target_events)
    }

    pub fn analyze_command_events(events: &[CommandEvent]) -> EfficiencyMetrics {
        Self::analyze_with_events("", Some(events.to_vec()))
    }

    pub fn analyze_with_pattern(transcript: &str, command_pattern: &str) -> EfficiencyMetrics {
        let commands = Self::extract_commands_with_pattern(transcript, command_pattern);
        Self::analyze_with_events(transcript, Some(commands))
    }

    pub fn resolve_command_pattern(target_binary: &str, command_pattern: Option<&str>) -> String {
        if let Some(pattern) = command_pattern {
            if !pattern.trim().is_empty() {
                return pattern.to_string();
            }
        }

        format!(r"^\s*({})\s+(--help|\S+)\b", regex::escape(target_binary))
    }

    pub fn analyze_with_events(
        _transcript: &str,
        events: Option<Vec<CommandEvent>>,
    ) -> EfficiencyMetrics {
        let mut commands: Vec<(String, bool)> = Vec::new();

        if let Some(command_events) = events {
            for event in command_events {
                let is_error = event.exit_code.map(|code| code != 0).unwrap_or(false);
                commands.push((event.command, is_error));
            }
        }

        let total_commands = commands.len();
        let error_count = commands.iter().filter(|(_, e)| *e).count();
        let help_invocations = commands.iter().filter(|(c, _)| c == "help").count();

        let unique_commands: std::collections::HashSet<_> =
            commands.iter().map(|(c, _)| c.clone()).collect();
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
                Self::target_subcommand(&event.command, target_binary).map(|command| CommandEvent {
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
                if let Some(subcommand) = Self::target_subcommand(token, target_binary) {
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

    fn is_error_line(line: &str) -> bool {
        let line_lower = line.to_lowercase();
        line_lower.contains("error")
            || line_lower.contains("failed")
            || line_lower.contains("exit code")
            || line_lower.contains("non-zero")
    }

    #[allow(dead_code)]
    pub(crate) fn extract_commands_with_exit_codes(transcript: &str) -> Vec<CommandEvent> {
        Self::extract_commands_with_pattern(transcript, DEFAULT_COMMAND_PATTERN)
    }

    /// Extract command events from transcript lines using the provided regex pattern.
    ///
    /// The pattern must capture the binary in group 1 and subcommand in group 2.
    ///
    /// Hypothetical command examples this supports:
    /// - `taskmgr create --title "Ship v1"`
    /// - `notes-cli list --format json`
    /// - `acme-tool deploy --env staging`
    pub(crate) fn extract_commands_with_pattern(
        transcript: &str,
        command_pattern: &str,
    ) -> Vec<CommandEvent> {
        let Ok(command_regex) = Regex::new(command_pattern) else {
            return Vec::new();
        };
        let exit_code_regex = Regex::new(r"(?i)exit\s+(?:code|status):?\s*(\d+)").unwrap();

        let lines: Vec<&str> = transcript.lines().collect();
        let mut commands = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            if let Some(caps) = command_regex.captures(line) {
                let binary = caps.get(1).map(|m| m.as_str().to_string());
                if let Some(binary_name) = &binary {
                    if binary_name.eq_ignore_ascii_case("exit")
                        || binary_name.eq_ignore_ascii_case("error")
                        || binary_name.eq_ignore_ascii_case("failed")
                    {
                        continue;
                    }
                }

                let subcommand = if let Some(subcommand_match) = caps.get(2) {
                    subcommand_match.as_str().to_string()
                } else if let Some(primary_capture) = caps.get(1) {
                    primary_capture.as_str().to_string()
                } else {
                    let mut parts = line.split_whitespace();
                    let _binary_part = parts.next();
                    parts.next().unwrap_or("command").to_string()
                };
                let is_help = subcommand == "--help" || line.contains("--help");

                if is_help {
                    commands.push(CommandEvent {
                        command: "help".to_string(),
                        exit_code: Some(0),
                    });
                } else {
                    let next_lines: Vec<&str> = lines[i + 1..].iter().take(20).cloned().collect();
                    let joined = next_lines.join("\n");

                    let exit_code = if let Some(exit_caps) = exit_code_regex.captures(&joined) {
                        exit_caps[1].parse().unwrap_or(-1)
                    } else if Self::is_error_line(&joined) {
                        1
                    } else {
                        0
                    };

                    commands.push(CommandEvent {
                        command: subcommand,
                        exit_code: Some(exit_code),
                    });
                }
            }
        }

        commands
    }
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
