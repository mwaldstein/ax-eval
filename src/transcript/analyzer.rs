use crate::interaction_evidence::CommandEvent;
use regex::Regex;

pub struct TranscriptAnalyzer;

const DEFAULT_COMMAND_PATTERN: &str = r"^\s*([a-z][a-z0-9_.-]*)\s+(--help|[a-z][a-z0-9_-]*)\b";

impl TranscriptAnalyzer {
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

    pub(crate) fn extract_command_lines_with_exit_codes(transcript: &str) -> Vec<CommandEvent> {
        let command_line_regex = Regex::new(r"^\s*(?:\$\s*)?([A-Za-z0-9_./~:-][^\r\n]*)").unwrap();
        let exit_code_regex = Regex::new(r"(?i)exit\s+(?:code|status):?\s*(\d+)").unwrap();

        let lines: Vec<&str> = transcript.lines().collect();
        let mut commands = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            let Some(caps) = command_line_regex.captures(line) else {
                continue;
            };
            let Some(command_match) = caps.get(1) else {
                continue;
            };
            let command = command_match.as_str().trim();
            let command_lower = command.to_lowercase();
            if command_lower.starts_with("exit ")
                || command_lower.starts_with("error")
                || command_lower.starts_with("failed")
            {
                continue;
            }

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
                command: command.to_string(),
                exit_code: Some(exit_code),
            });
        }

        commands
    }
}
