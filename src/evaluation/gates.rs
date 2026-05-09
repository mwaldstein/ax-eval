use crate::interaction_profile::InteractionProfile;
use crate::scenario::Gate;
use crate::script_runner::ScriptRunner;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

use super::GateResult;

/// Context passed to gate evaluators, containing environment and optional script runner.
pub struct GateEvaluationContext<'a> {
    pub env_root: &'a Path,
    pub script_runner: Option<&'a ScriptRunner>,
    pub interaction_profile: &'a InteractionProfile,
}

trait GateEvaluator {
    fn evaluate(&self, ctx: &GateEvaluationContext<'_>) -> GateResult;
}

impl GateEvaluator for Gate {
    fn evaluate(&self, ctx: &GateEvaluationContext<'_>) -> GateResult {
        match self {
            Gate::CommandSucceeds { command } => eval_command_succeeds(command, ctx.env_root),
            Gate::CommandOutputContains { command, substring } => {
                eval_command_output_contains(command, substring, ctx.env_root)
            }
            Gate::CommandOutputMatches { command, pattern } => {
                eval_command_output_matches(command, pattern, ctx.env_root)
            }
            Gate::CommandJsonPath {
                command,
                path,
                assertion,
            } => eval_command_json_path(command, path, assertion, ctx.env_root),
            Gate::FileExists { path } => eval_file_exists(path, ctx.env_root),
            Gate::FileContains { path, substring } => {
                eval_file_contains(path, substring, ctx.env_root)
            }
            Gate::FileMatches { path, pattern } => eval_file_matches(path, pattern, ctx.env_root),
            Gate::NoTranscriptErrors => eval_no_transcript_errors(ctx.interaction_profile),
            Gate::Script {
                command,
                description,
                timeout_secs,
            } => eval_script(command, description, *timeout_secs, ctx.script_runner),
        }
    }
}

pub fn evaluate_gates(gates: &[Gate], ctx: &GateEvaluationContext<'_>) -> (Vec<GateResult>, usize) {
    let mut details = Vec::new();
    let mut gates_passed = 0;

    for gate in gates {
        let result = gate.evaluate(ctx);

        if result.passed {
            println!("Gate {} passed: {}", result.gate_type, result.message);
            gates_passed += 1;
        } else {
            println!("Gate {} FAILED: {}", result.gate_type, result.message);
        }
        details.push(result);
    }

    (details, gates_passed)
}

fn eval_command_succeeds(command: &str, env_root: &Path) -> GateResult {
    if command.trim().is_empty() {
        return GateResult {
            gate_type: "CommandSucceeds".to_string(),
            passed: false,
            message: "Empty command".to_string(),
        };
    }

    let output = run_shell_command(command, env_root);

    match output {
        Ok(output) => {
            let succeeds = output.status.success();
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

fn eval_command_output_contains(command: &str, substring: &str, env_root: &Path) -> GateResult {
    let output = run_shell_command(command, env_root);

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let passed = output.status.success() && stdout.contains(substring);
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

fn eval_command_output_matches(command: &str, pattern: &str, env_root: &Path) -> GateResult {
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

    let output = run_shell_command(command, env_root);

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let passed = output.status.success() && regex.is_match(&stdout);
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

fn eval_command_json_path(
    command: &str,
    path: &str,
    assertion: &str,
    env_root: &Path,
) -> GateResult {
    match run_shell_command(command, env_root) {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                return GateResult {
                    gate_type: "CommandJsonPath".to_string(),
                    passed: false,
                    message: format!(
                        "Command '{}' failed with exit code {:?}: {}",
                        command,
                        output.status.code(),
                        stderr
                    ),
                };
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let json: Value = match serde_json::from_str(&stdout) {
                Ok(value) => value,
                Err(e) => {
                    return GateResult {
                        gate_type: "CommandJsonPath".to_string(),
                        passed: false,
                        message: format!("Command output is not valid JSON: {}", e),
                    };
                }
            };

            let resolved_value = match resolve_json_path(&json, path) {
                Ok(value) => value,
                Err(e) => {
                    return GateResult {
                        gate_type: "CommandJsonPath".to_string(),
                        passed: false,
                        message: format!("Invalid JSON path '{}': {}", path, e),
                    };
                }
            };

            let (passed, detail) = match evaluate_json_assertion(resolved_value, assertion) {
                Ok(result) => result,
                Err(e) => {
                    return GateResult {
                        gate_type: "CommandJsonPath".to_string(),
                        passed: false,
                        message: format!("Invalid assertion '{}': {}", assertion, e),
                    };
                }
            };

            GateResult {
                gate_type: "CommandJsonPath".to_string(),
                passed,
                message: format!(
                    "Path '{}' with assertion '{}' => {} ({})",
                    path, assertion, passed, detail
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

fn eval_file_exists(path: &str, env_root: &Path) -> GateResult {
    let full_path = env_root.join(path);
    let passed = full_path.exists();
    GateResult {
        gate_type: "FileExists".to_string(),
        passed,
        message: format!("File '{}' exists: {}", full_path.display(), passed),
    }
}

fn eval_file_contains(path: &str, substring: &str, env_root: &Path) -> GateResult {
    let full_path = env_root.join(path);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let passed = content.contains(substring);
            GateResult {
                gate_type: "FileContains".to_string(),
                passed,
                message: format!(
                    "File '{}' contains substring '{}': {}",
                    full_path.display(),
                    substring,
                    passed
                ),
            }
        }
        Err(e) => GateResult {
            gate_type: "FileContains".to_string(),
            passed: false,
            message: format!("Failed to read file '{}': {}", full_path.display(), e),
        },
    }
}

fn eval_file_matches(path: &str, pattern: &str, env_root: &Path) -> GateResult {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(e) => {
            return GateResult {
                gate_type: "FileMatches".to_string(),
                passed: false,
                message: format!("Invalid regex pattern '{}': {}", pattern, e),
            }
        }
    };

    let full_path = env_root.join(path);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let passed = regex.is_match(&content);
            GateResult {
                gate_type: "FileMatches".to_string(),
                passed,
                message: format!(
                    "File '{}' matches pattern '{}': {}",
                    full_path.display(),
                    pattern,
                    passed
                ),
            }
        }
        Err(e) => GateResult {
            gate_type: "FileMatches".to_string(),
            passed: false,
            message: format!("Failed to read file '{}': {}", full_path.display(), e),
        },
    }
}

fn run_shell_command(command: &str, env_root: &Path) -> std::io::Result<Output> {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(env_root)
        .output()
}

#[derive(Debug)]
enum JsonPathSegment {
    Key(String),
    Index(usize),
}

fn parse_json_path(path: &str) -> std::result::Result<Vec<JsonPathSegment>, String> {
    if !path.starts_with('$') {
        return Err("path must start with '$'".to_string());
    }

    if path == "$" {
        return Ok(Vec::new());
    }

    let chars: Vec<char> = path.chars().collect();
    let mut i = 1;
    let mut segments = Vec::new();

    while i < chars.len() {
        match chars[i] {
            '.' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '.' && chars[i] != '[' {
                    i += 1;
                }
                if start == i {
                    return Err("empty object key in path".to_string());
                }
                let key: String = chars[start..i].iter().collect();
                segments.push(JsonPathSegment::Key(key));
            }
            '[' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != ']' {
                    i += 1;
                }
                if i >= chars.len() || chars[i] != ']' {
                    return Err("unclosed array index bracket".to_string());
                }
                let index_text: String = chars[start..i].iter().collect();
                let index = index_text
                    .parse::<usize>()
                    .map_err(|_| format!("invalid array index '{}'", index_text))?;
                segments.push(JsonPathSegment::Index(index));
                i += 1;
            }
            _ => return Err(format!("unexpected character '{}' in path", chars[i])),
        }
    }

    Ok(segments)
}

fn resolve_json_path<'a>(
    json: &'a Value,
    path: &str,
) -> std::result::Result<Option<&'a Value>, String> {
    let segments = parse_json_path(path)?;
    let mut current = json;

    for segment in segments {
        match segment {
            JsonPathSegment::Key(key) => {
                let Some(next) = current.get(&key) else {
                    return Ok(None);
                };
                current = next;
            }
            JsonPathSegment::Index(index) => {
                let Some(array) = current.as_array() else {
                    return Ok(None);
                };
                let Some(next) = array.get(index) else {
                    return Ok(None);
                };
                current = next;
            }
        }
    }

    Ok(Some(current))
}

fn evaluate_json_assertion(
    value: Option<&Value>,
    assertion: &str,
) -> std::result::Result<(bool, String), String> {
    let trimmed = assertion.trim();

    if trimmed == "exists" {
        let passed = matches!(value, Some(v) if !v.is_null());
        return Ok((passed, "value exists and is not null".to_string()));
    }

    if let Some(expected_text) = trimmed.strip_prefix("equals ") {
        let Some(actual) = value else {
            return Ok((false, "path not found".to_string()));
        };
        let expected = serde_json::from_str::<Value>(expected_text)
            .unwrap_or_else(|_| Value::String(expected_text.to_string()));
        let passed = actual == &expected;
        return Ok((passed, format!("actual={}, expected={}", actual, expected)));
    }

    if let Some(needle) = trimmed.strip_prefix("contains ") {
        let Some(actual) = value else {
            return Ok((false, "path not found".to_string()));
        };
        let Some(text) = actual.as_str() else {
            return Ok((false, "value is not a string".to_string()));
        };
        let passed = text.contains(needle);
        return Ok((passed, format!("substring='{}'", needle)));
    }

    let len_regex = Regex::new(r"^len\s*(>=|==|>)\s*(\d+)$").expect("valid len regex");
    if let Some(captures) = len_regex.captures(trimmed) {
        let Some(actual) = value else {
            return Ok((false, "path not found".to_string()));
        };
        let operator = captures
            .get(1)
            .map(|m| m.as_str())
            .ok_or_else(|| "missing length operator".to_string())?;
        let expected_len = captures
            .get(2)
            .ok_or_else(|| "missing length value".to_string())?
            .as_str()
            .parse::<usize>()
            .map_err(|_| "length must be a non-negative integer".to_string())?;

        let actual_len = if let Some(array) = actual.as_array() {
            array.len()
        } else if let Some(object) = actual.as_object() {
            object.len()
        } else {
            return Ok((false, "value is not an array or object".to_string()));
        };

        let passed = match operator {
            ">=" => actual_len >= expected_len,
            "==" => actual_len == expected_len,
            ">" => actual_len > expected_len,
            _ => return Err(format!("unsupported length operator '{}'", operator)),
        };

        return Ok((
            passed,
            format!("actual_len={} {} {}", actual_len, operator, expected_len),
        ));
    }

    Err("assertion must be one of: exists, equals <value>, contains <substring>, len >= N, len == N, len > N".to_string())
}

fn eval_script(
    command: &str,
    description: &str,
    timeout_secs: u64,
    script_runner: Option<&ScriptRunner>,
) -> GateResult {
    let runner = match script_runner {
        Some(r) => r,
        None => {
            return GateResult {
                gate_type: "Script".to_string(),
                passed: false,
                message: "Script runner not available for script gate evaluation".to_string(),
            };
        }
    };

    let result = match runner.run(command, timeout_secs) {
        Ok(r) => r,
        Err(e) => {
            return GateResult {
                gate_type: "Script".to_string(),
                passed: false,
                message: format!("Failed to execute script '{}': {}", command, e),
            };
        }
    };

    if result.timed_out {
        return GateResult {
            gate_type: "Script".to_string(),
            passed: false,
            message: format!(
                "Script '{}' timed out after {} seconds",
                command, timeout_secs
            ),
        };
    }

    #[derive(Deserialize)]
    struct ScriptGateOutput {
        passed: bool,
        message: Option<String>,
    }

    let stdout = result.stdout.trim();
    if let Ok(parsed) = serde_json::from_str::<ScriptGateOutput>(stdout) {
        return GateResult {
            gate_type: "Script".to_string(),
            passed: parsed.passed,
            message: parsed.message.unwrap_or_else(|| description.to_string()),
        };
    }

    let passed = result.succeeded();
    GateResult {
        gate_type: "Script".to_string(),
        passed,
        message: format!(
            "Script '{}' {} (exit code: {}, description: {})",
            command,
            if passed { "passed" } else { "failed" },
            result.exit_code,
            description
        ),
    }
}

fn eval_no_transcript_errors(interaction_profile: &InteractionProfile) -> GateResult {
    let no_errors = interaction_profile.metrics.error_count == 0;
    GateResult {
        gate_type: "NoTranscriptErrors".to_string(),
        passed: no_errors,
        message: format!("Transcript has no command errors: {}", no_errors),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    fn temp_env() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn test_runner(fixture_dir: PathBuf) -> ScriptRunner {
        ScriptRunner::new(
            fixture_dir,
            PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            HashMap::new(),
        )
    }

    #[test]
    fn command_succeeds_gate_passes_for_successful_command() {
        let env = temp_env();
        let result = eval_command_succeeds("true", env.path());
        assert!(result.passed);
    }

    #[test]
    fn command_succeeds_gate_fails_for_failing_command() {
        let env = temp_env();
        let result = eval_command_succeeds("false", env.path());
        assert!(!result.passed);
    }

    #[test]
    fn command_output_contains_gate_checks_stdout_substring() {
        let env = temp_env();
        let result = eval_command_output_contains("printf 'hello world'", "hello", env.path());
        assert!(result.passed);
    }

    #[test]
    fn command_output_matches_gate_checks_stdout_regex() {
        let env = temp_env();
        let result = eval_command_output_matches("printf 'abc-123'", r"abc-\d+", env.path());
        assert!(result.passed);
    }

    #[test]
    fn command_json_path_gate_supports_exists_assertion() {
        let env = temp_env();
        let result = eval_command_json_path(
            "printf '{\"meta\":{\"ok\":true}}'",
            "$.meta.ok",
            "exists",
            env.path(),
        );
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn command_json_path_gate_supports_equals_assertion() {
        let env = temp_env();
        let result =
            eval_command_json_path("printf '{\"count\":3}'", "$.count", "equals 3", env.path());
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn command_json_path_gate_supports_contains_assertion() {
        let env = temp_env();
        let result = eval_command_json_path(
            "printf '{\"msg\":\"build succeeded\"}'",
            "$.msg",
            "contains succeeded",
            env.path(),
        );
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn command_json_path_gate_supports_len_assertion() {
        let env = temp_env();
        let result = eval_command_json_path(
            "printf '{\"items\":[1,2,3]}'",
            "$.items",
            "len >= 3",
            env.path(),
        );
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn file_exists_gate_checks_relative_path() {
        let env = temp_env();
        fs::write(env.path().join("result.txt"), "ok").expect("write file");

        let result = eval_file_exists("result.txt", env.path());
        assert!(result.passed);
    }

    #[test]
    fn file_contains_gate_checks_file_contents() {
        let env = temp_env();
        fs::write(env.path().join("notes.md"), "status: complete").expect("write file");

        let result = eval_file_contains("notes.md", "complete", env.path());
        assert!(result.passed);
    }

    #[test]
    fn file_matches_gate_checks_file_regex() {
        let env = temp_env();
        fs::write(env.path().join("logs.txt"), "run-42 done").expect("write file");

        let result = eval_file_matches("logs.txt", r"run-\d+", env.path());
        assert!(result.passed);
    }

    #[test]
    fn script_gate_with_exit_code_success() {
        let temp = tempfile::tempdir().unwrap();
        let runner = test_runner(temp.path().to_path_buf());

        let result = eval_script("true", "should pass", 30, Some(&runner));
        assert!(result.passed, "Exit code 0 should pass: {}", result.message);
    }

    #[test]
    fn script_gate_with_exit_code_failure() {
        let temp = tempfile::tempdir().unwrap();
        let runner = test_runner(temp.path().to_path_buf());

        let result = eval_script("false", "should fail", 30, Some(&runner));
        assert!(
            !result.passed,
            "Exit code 1 should fail: {}",
            result.message
        );
    }

    #[test]
    fn script_gate_with_json_output() {
        let temp = tempfile::tempdir().unwrap();
        let runner = test_runner(temp.path().to_path_buf());

        let result = eval_script(
            "echo '{\"passed\": true, \"message\": \"Custom check passed\"}'",
            "json gate",
            30,
            Some(&runner),
        );
        assert!(
            result.passed,
            "JSON passed=true should pass: {}",
            result.message
        );
        assert!(result.message.contains("Custom check passed"));
    }

    #[test]
    fn script_gate_with_json_output_failure() {
        let temp = tempfile::tempdir().unwrap();
        let runner = test_runner(temp.path().to_path_buf());

        let result = eval_script(
            "echo '{\"passed\": false, \"message\": \"Custom check failed\"}'",
            "json gate",
            30,
            Some(&runner),
        );
        assert!(
            !result.passed,
            "JSON passed=false should fail: {}",
            result.message
        );
        assert!(result.message.contains("Custom check failed"));
    }

    #[test]
    fn script_gate_without_runner_fails() {
        let result = eval_script("true", "no runner", 30, None);
        assert!(!result.passed);
        assert!(result.message.contains("Script runner not available"));
    }
}
