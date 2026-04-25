use super::{AdapterError, TokenUsage, ToolAdapter, ToolStatus};
use crate::scenario::Scenario;
use regex::Regex;
use std::path::Path;
use std::process::Command;

/// A mock adapter that extracts shell commands from the task prompt and executes
/// them in the working directory. This enables example scenarios to serve as
/// automated end-to-end tests without requiring a real LLM.
///
/// Commands are extracted by looking for lines that contain the target binary
/// name, optionally prefixed with `./`, and optionally preceded by list markers
/// (`- `, `* `, `1. `). Commands inside backticks are also extracted.
pub struct SmartMockAdapter;

impl SmartMockAdapter {
    pub fn generate_transcript(&self, scenario: &Scenario, cwd: &Path) -> String {
        let binary = &scenario.target.binary;
        let commands = self.extract_commands(&scenario.task.prompt, binary);

        if commands.is_empty() {
            return format!(
                "SmartMock: No commands found for binary '{}' in prompt.\n",
                binary
            );
        }

        let mut transcript = String::new();
        for cmd in commands {
            transcript.push_str(&format!("$ {}\n", cmd));

            let output = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .current_dir(cwd)
                .output();

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    if !stdout.is_empty() {
                        transcript.push_str(&stdout);
                        if !stdout.ends_with('\n') {
                            transcript.push('\n');
                        }
                    }
                    if !stderr.is_empty() {
                        transcript.push_str(&stderr);
                        if !stderr.ends_with('\n') {
                            transcript.push('\n');
                        }
                    }
                    transcript.push_str(&format!("(exit {})\n", out.status.code().unwrap_or(0)));
                }
                Err(e) => {
                    transcript.push_str(&format!("Error: {}\n", e));
                    transcript.push_str("(exit 1)\n");
                }
            }
        }

        transcript.push_str("SmartMock execution completed.\n");
        transcript
    }

    fn extract_commands(&self, prompt: &str, binary: &str) -> Vec<String> {
        let mut commands = Vec::new();
        let escaped = regex::escape(binary);

        // Pattern 1: lines starting with optional whitespace, optional list marker,
        // then ./binary or binary followed by the rest of the command.
        let line_pattern = format!(
            r"(?m)^\s*(?:[-*]\s+|\d+\.\s+)?((?:\./)?{}\b.*)$",
            escaped
        );
        let line_re = Regex::new(&line_pattern).expect("valid line regex");

        for cap in line_re.captures_iter(prompt) {
            let full = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            if full.is_empty() {
                continue;
            }
            // Strip any trailing explanation like "  # comment"
            let cmd = full.split("  #").next().unwrap_or(full);
            commands.push(cmd.to_string());
        }

        // Pattern 2: commands inside backticks containing the binary
        let backtick_pattern = format!(r"`(?:\./)?{}[^`]*`", escaped);
        let backtick_re = Regex::new(&backtick_pattern).expect("valid backtick regex");
        for cap in backtick_re.find_iter(prompt) {
            let mut cmd = cap.as_str();
            cmd = &cmd[1..cmd.len() - 1]; // strip backticks
            commands.push(cmd.to_string());
        }

        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        commands.retain(|c| seen.insert(c.clone()));

        commands
    }
}

impl ToolAdapter for SmartMockAdapter {
    fn is_available(&self) -> Result<ToolStatus, AdapterError> {
        Ok(ToolStatus {
            available: true,
            authenticated: true,
        })
    }

    fn check_availability(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn run(
        &self,
        scenario: &Scenario,
        cwd: &Path,
        _model: Option<&str>,
        _timeout_secs: u64,
    ) -> anyhow::Result<(String, i32, Option<f64>, Option<TokenUsage>)> {
        let transcript = self.generate_transcript(scenario, cwd);
        Ok((transcript, 0, None, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Evaluation, TargetConfig, Task};

    fn make_scenario(prompt: &str, binary: &str) -> Scenario {
        Scenario {
            name: "test".to_string(),
            description: "test".to_string(),
            template_folder: "test".to_string(),
            target: TargetConfig {
                binary: binary.to_string(),
                command_pattern: None,
                health_check: None,
                env: None,
            },
            task: Task {
                prompt: prompt.to_string(),
            },
            evaluation: Evaluation {
                gates: vec![],
                judge: None,
                composite: None,
            },
            tier: 0,
            tool_matrix: None,
            setup: None,
            tags: vec![],
            run: None,
            scripts: None,
        }
    }

    #[test]
    fn extracts_indented_command() {
        let adapter = SmartMockAdapter;
        let scenario = make_scenario(
            "Run the tool:\n    ./mytool run",
            "mytool",
        );
        let cmds = adapter.extract_commands(&scenario.task.prompt, "mytool");
        assert_eq!(cmds, vec!["./mytool run"]);
    }

    #[test]
    fn extracts_bulleted_commands() {
        let adapter = SmartMockAdapter;
        let scenario = make_scenario(
            "Do these:\n- ./mytool init\n- ./mytool create test\n* mytool list",
            "mytool",
        );
        let cmds = adapter.extract_commands(&scenario.task.prompt, "mytool");
        assert_eq!(cmds, vec!["./mytool init", "./mytool create test", "mytool list"]);
    }

    #[test]
    fn extracts_numbered_commands() {
        let adapter = SmartMockAdapter;
        let scenario = make_scenario(
            "Steps:\n1. ./mytool init\n2. mytool create foo",
            "mytool",
        );
        let cmds = adapter.extract_commands(&scenario.task.prompt, "mytool");
        assert_eq!(cmds, vec!["./mytool init", "mytool create foo"]);
    }

    #[test]
    fn extracts_backtick_commands() {
        let adapter = SmartMockAdapter;
        let scenario = make_scenario(
            "Run `./mytool run` to start",
            "mytool",
        );
        let cmds = adapter.extract_commands(&scenario.task.prompt, "mytool");
        assert_eq!(cmds, vec!["./mytool run"]);
    }

    #[test]
    fn deduplicates_commands() {
        let adapter = SmartMockAdapter;
        let scenario = make_scenario(
            "Run:\n./mytool run\n./mytool run",
            "mytool",
        );
        let cmds = adapter.extract_commands(&scenario.task.prompt, "mytool");
        assert_eq!(cmds, vec!["./mytool run"]);
    }

    #[test]
    fn ignores_lines_without_binary() {
        let adapter = SmartMockAdapter;
        let scenario = make_scenario(
            "Some text\nnot a command\n./other run",
            "mytool",
        );
        let cmds = adapter.extract_commands(&scenario.task.prompt, "mytool");
        assert!(cmds.is_empty());
    }

    #[test]
    fn generates_transcript_with_exit_code() {
        let adapter = SmartMockAdapter;
        let temp = tempfile::tempdir().unwrap();
        let scenario = make_scenario(
            "Run:\n    echo hello",
            "echo",
        );
        let transcript = adapter.generate_transcript(&scenario, temp.path());
        assert!(transcript.contains("$ echo hello"));
        assert!(transcript.contains("hello"));
        assert!(transcript.contains("(exit 0)"));
    }
}
