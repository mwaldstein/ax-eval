use crate::command_execution::CommandResult;
use crate::target_env::TargetEnvironment;
use regex::Regex;
use serde_json::Value;
use std::path::Path;

use super::assertions::{evaluate_json_path_assertion, JsonAssertionError};
use super::GateResult;

const COMMAND_GATE_TIMEOUT_SECS: u64 = 30;

pub(super) fn eval_command_succeeds(
    command: &str,
    env_root: &Path,
    target_env: &TargetEnvironment,
) -> GateResult {
    if command.trim().is_empty() {
        return GateResult {
            gate_type: "CommandSucceeds".to_string(),
            passed: false,
            message: "Empty command".to_string(),
        };
    }

    let output = run_shell_command(command, env_root, target_env);

    match output {
        Ok(output) => {
            let succeeds = output.succeeded();
            GateResult {
                gate_type: "CommandSucceeds".to_string(),
                passed: succeeds,
                message: format!("Command '{}' succeeded: {}", command, succeeds),
            }
        }
        Err(e) => GateResult {
            gate_type: "CommandSucceeds".to_string(),
            passed: false,
            message: format!("Failed to execute command '{}': {}", command, e),
        },
    }
}

pub(super) fn eval_command_output_contains(
    command: &str,
    substring: &str,
    env_root: &Path,
    target_env: &TargetEnvironment,
) -> GateResult {
    let output = run_shell_command(command, env_root, target_env);

    match output {
        Ok(output) => {
            let passed = output.succeeded() && output.stdout.contains(substring);
            GateResult {
                gate_type: "CommandOutputContains".to_string(),
                passed,
                message: format!(
                    "Command '{}' contains substring '{}': {}",
                    command, substring, passed
                ),
            }
        }
        Err(e) => GateResult {
            gate_type: "CommandOutputContains".to_string(),
            passed: false,
            message: format!("Failed to execute command '{}': {}", command, e),
        },
    }
}

pub(super) fn eval_command_output_matches(
    command: &str,
    pattern: &str,
    env_root: &Path,
    target_env: &TargetEnvironment,
) -> GateResult {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(e) => {
            return GateResult {
                gate_type: "CommandOutputMatches".to_string(),
                passed: false,
                message: format!("Invalid regex pattern '{}': {}", pattern, e),
            }
        }
    };

    let output = run_shell_command(command, env_root, target_env);

    match output {
        Ok(output) => {
            let passed = output.succeeded() && regex.is_match(&output.stdout);
            GateResult {
                gate_type: "CommandOutputMatches".to_string(),
                passed,
                message: format!(
                    "Command '{}' matches pattern '{}': {}",
                    command, pattern, passed
                ),
            }
        }
        Err(e) => GateResult {
            gate_type: "CommandOutputMatches".to_string(),
            passed: false,
            message: format!("Failed to execute command '{}': {}", command, e),
        },
    }
}

pub(super) fn eval_command_json_path(
    command: &str,
    path: &str,
    assertion: &str,
    env_root: &Path,
    target_env: &TargetEnvironment,
) -> GateResult {
    match run_shell_command(command, env_root, target_env) {
        Ok(output) => {
            if !output.succeeded() {
                let stderr = output.stderr.trim().to_string();
                return GateResult {
                    gate_type: "CommandJsonPath".to_string(),
                    passed: false,
                    message: format!(
                        "Command '{}' failed with exit code {}: {}",
                        command, output.exit_code, stderr
                    ),
                };
            }

            let json: Value = match serde_json::from_str(&output.stdout) {
                Ok(value) => value,
                Err(e) => {
                    return GateResult {
                        gate_type: "CommandJsonPath".to_string(),
                        passed: false,
                        message: format!("Command output is not valid JSON: {}", e),
                    };
                }
            };

            let outcome = match evaluate_json_path_assertion(&json, path, assertion) {
                Ok(outcome) => outcome,
                Err(JsonAssertionError::Path(e)) => {
                    return GateResult {
                        gate_type: "CommandJsonPath".to_string(),
                        passed: false,
                        message: format!("Invalid JSON path '{}': {}", path, e),
                    };
                }
                Err(JsonAssertionError::Assertion(e)) => {
                    return GateResult {
                        gate_type: "CommandJsonPath".to_string(),
                        passed: false,
                        message: format!("Invalid assertion '{}': {}", assertion, e),
                    };
                }
            };

            GateResult {
                gate_type: "CommandJsonPath".to_string(),
                passed: outcome.passed,
                message: format!(
                    "Path '{}' with assertion '{}' => {} ({})",
                    path, assertion, outcome.passed, outcome.detail
                ),
            }
        }
        Err(e) => GateResult {
            gate_type: "CommandJsonPath".to_string(),
            passed: false,
            message: format!("Failed to execute command '{}': {}", command, e),
        },
    }
}

fn run_shell_command(
    command: &str,
    env_root: &Path,
    target_env: &TargetEnvironment,
) -> anyhow::Result<CommandResult> {
    crate::command_execution::run_shell_piped(
        command,
        env_root,
        COMMAND_GATE_TIMEOUT_SECS,
        target_env.as_map(),
    )
}
