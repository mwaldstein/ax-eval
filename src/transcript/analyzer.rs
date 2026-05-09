use crate::transcript::types::{CommandEvent, EfficiencyMetrics};
use regex::Regex;

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
        crate::interaction_profile::reduce_command_events(&commands)
    }

    #[allow(dead_code)]
    pub fn analyze_with_exit_codes_for_target(
        transcript: &str,
        target_binary: &str,
        command_pattern: Option<&str>,
    ) -> EfficiencyMetrics {
        let commands =
            Self::extract_command_events_for_target(transcript, target_binary, command_pattern);
        crate::interaction_profile::reduce_command_events(&commands)
    }

    pub fn analyze_with_pattern(transcript: &str, command_pattern: &str) -> EfficiencyMetrics {
        let commands = Self::extract_commands_with_pattern(transcript, command_pattern);
        crate::interaction_profile::reduce_command_events(&commands)
    }

    pub fn resolve_command_pattern(target_binary: &str, command_pattern: Option<&str>) -> String {
        if let Some(pattern) = command_pattern {
            if !pattern.trim().is_empty() {
                return pattern.to_string();
            }
        }

        format!(r"^\s*({})\s+(--help|\S+)\b", regex::escape(target_binary))
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

    pub(crate) fn extract_command_events_for_target(
        transcript: &str,
        target_binary: &str,
        command_pattern: Option<&str>,
    ) -> Vec<CommandEvent> {
        let pattern = Self::resolve_command_pattern(target_binary, command_pattern);
        Self::extract_commands_with_pattern(transcript, &pattern)
    }
}
