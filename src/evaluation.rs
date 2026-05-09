use crate::interaction_profile::InteractionProfile;
use crate::judge::{load_rubric, JudgeResponse};
use crate::scenario::{Gate, JudgeConfig, Scenario};
use crate::script_runner::ScriptRunner;
use crate::transcript::InteractionInput;
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::process::{Command, Output};

/// Context passed to gate evaluators, containing environment and optional script runner.
pub struct EvaluationContext<'a> {
    pub env_root: &'a Path,
    pub script_runner: Option<&'a ScriptRunner>,
    pub interaction_profile: &'a InteractionProfile,
}

pub trait GateEvaluator {
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> GateResult;
}

impl GateEvaluator for Gate {
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> GateResult {
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

    // Try to parse stdout as JSON with {passed, message}
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

    // Fall back to exit code (consider both exit code and timeout)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreTier {
    Excellent,
    Good,
    Acceptable,
    Poor,
}

impl ScoreTier {
    pub fn from_score(score: f64) -> Self {
        if score >= 0.9 {
            ScoreTier::Excellent
        } else if score >= 0.7 {
            ScoreTier::Good
        } else if score >= 0.5 {
            ScoreTier::Acceptable
        } else {
            ScoreTier::Poor
        }
    }
}

impl fmt::Display for ScoreTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScoreTier::Excellent => write!(f, "Excellent"),
            ScoreTier::Good => write!(f, "Good"),
            ScoreTier::Acceptable => write!(f, "Acceptable"),
            ScoreTier::Poor => write!(f, "Poor"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluationMetrics {
    pub gates_passed: usize,
    pub gates_total: usize,
    pub details: Vec<GateResult>,
    pub judge_score: Option<f64>,
    pub judge_response: Option<JudgeResponse>,
    pub judge_passed: Option<bool>,
    pub efficiency: crate::transcript::EfficiencyMetrics,
    pub interaction_evidence_source: crate::interaction_profile::InteractionEvidenceSource,
    /// Composite score is only computed if scenario configures composite weights
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composite_score: Option<f64>,
    /// Results from custom evaluator scripts
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evaluator_results: Vec<EvaluatorResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_type: String,
    pub passed: bool,
    pub message: String,
}

/// Result from a custom evaluator script.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResult {
    /// Name of the evaluator
    pub name: String,
    /// Optional metrics as JSON value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Value>,
    /// Optional score (0.0-1.0 or unbounded depending on evaluator)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Human-readable summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Error message if evaluator failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn evaluate_gates(gates: &[Gate], ctx: &EvaluationContext<'_>) -> (Vec<GateResult>, usize) {
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

fn run_judge_evaluation(
    judge_config: &JudgeConfig,
    judge_model: Option<&str>,
    judge_tool: Option<&str>,
    scenario: &Scenario,
    env_root: &Path,
) -> Result<(Option<f64>, Option<JudgeResponse>)> {
    println!("Running LLM-as-judge evaluation...");
    let tool = resolve_judge_tool(judge_config, judge_tool);
    let rubric_path = crate::utils::resolve_fixtures_path(&judge_config.rubric);
    let rubric = load_rubric(&rubric_path)
        .with_context(|| format!("Failed to load rubric from {}", rubric_path.display()))?;

    let transcript_path = env_root.join("transcript.raw.txt");
    let prompt = crate::judge::build_judge_prompt(
        &scenario.target.binary,
        &scenario.task.prompt,
        &transcript_path.display().to_string(),
        &rubric,
    );

    let runner = crate::session::SessionRunner::new();
    let command = build_judge_command(tool, judge_model, &prompt)?;
    let args = command.args.iter().map(String::as_str).collect::<Vec<_>>();

    let (output, exit_code) = runner
        .run_command(&command.binary, &args, env_root, 300)
        .context("Judge execution failed")?;

    if exit_code != 0 {
        anyhow::bail!("Judge exited with code {}: {}", exit_code, output);
    }

    let response: JudgeResponse = serde_json::from_str(&output)
        .with_context(|| format!("Failed to parse judge response: {}", output))?;

    println!(
        "Judge score: {:.2} (confidence: {:.2})",
        response.weighted_score, response.confidence
    );
    if !response.issues.is_empty() {
        println!("Issues: {}", response.issues.join(", "));
    }
    if !response.highlights.is_empty() {
        println!("Highlights: {}", response.highlights.join(", "));
    }

    Ok((Some(response.weighted_score), Some(response)))
}

fn resolve_judge_tool<'a>(judge_config: &'a JudgeConfig, judge_tool: Option<&'a str>) -> &'a str {
    judge_tool
        .or(judge_config.tool.as_deref())
        .unwrap_or("opencode")
}

#[derive(Debug, PartialEq, Eq)]
struct JudgeCommand {
    binary: String,
    args: Vec<String>,
}

fn build_judge_command(
    tool: &str,
    judge_model: Option<&str>,
    prompt: &str,
) -> Result<JudgeCommand> {
    let mut command = match tool {
        "opencode" => JudgeCommand {
            binary: "opencode".to_string(),
            args: vec!["run".to_string()],
        },
        "codex" => JudgeCommand {
            binary: "codex".to_string(),
            args: vec![
                "exec".to_string(),
                "--full-auto".to_string(),
                "--skip-git-repo-check".to_string(),
            ],
        },
        "claude" | "claude-code" => JudgeCommand {
            binary: "claude".to_string(),
            args: vec!["run".to_string()],
        },
        _ => anyhow::bail!("Unsupported judge tool: {}", tool),
    };

    if let Some(model) = judge_model {
        if tool != "codex" || model != "default" {
            command.args.push("--model".to_string());
            command.args.push(model.to_string());
        }
    }

    command.args.push(prompt.to_string());
    Ok(command)
}

fn maybe_run_judge(
    scenario: &Scenario,
    env_root: &Path,
    no_judge: bool,
    gates_passed: usize,
    gates_total: usize,
    judge_model: Option<&str>,
    judge_tool: Option<&str>,
) -> Result<(Option<f64>, Option<JudgeResponse>, Option<bool>)> {
    if let Some(judge_config) = &scenario.evaluation.judge {
        if judge_config.enabled && !no_judge {
            if gates_passed < gates_total {
                println!(
                    "Skipping judge: {}/{} gates failed",
                    gates_total - gates_passed,
                    gates_total
                );
                return Ok((None, None, None));
            }
            let (score, response) =
                run_judge_evaluation(judge_config, judge_model, judge_tool, scenario, env_root)?;
            let passed = score.map(|s| s >= judge_config.pass_threshold);
            if let Some(s) = score {
                if s >= judge_config.pass_threshold {
                    println!(
                        "Judge passed: score {:.2} >= threshold {:.2}",
                        s, judge_config.pass_threshold
                    );
                } else {
                    println!(
                        "Judge failed: score {:.2} < threshold {:.2}",
                        s, judge_config.pass_threshold
                    );
                }
            }
            return Ok((score, response, passed));
        }
    }
    Ok((None, None, None))
}

/// Run custom evaluator scripts from scenario configuration.
fn run_evaluators(
    scenario: &Scenario,
    script_runner: Option<&ScriptRunner>,
) -> Vec<EvaluatorResult> {
    let mut results = Vec::new();

    if let Some(scripts) = &scenario.scripts {
        for entry in &scripts.evaluators {
            println!("Running evaluator '{}'...", entry.name);

            let result = if let Some(runner) = script_runner {
                match runner.run(&entry.command, entry.timeout_secs) {
                    Ok(script_result) => {
                        if script_result.timed_out {
                            EvaluatorResult {
                                name: entry.name.clone(),
                                metrics: None,
                                score: None,
                                summary: None,
                                error: Some(format!(
                                    "Timed out after {} seconds",
                                    entry.timeout_secs
                                )),
                            }
                        } else if script_result.exit_code != 0 {
                            EvaluatorResult {
                                name: entry.name.clone(),
                                metrics: None,
                                score: None,
                                summary: None,
                                error: Some(format!(
                                    "Exit code {}: {}",
                                    script_result.exit_code, script_result.stderr
                                )),
                            }
                        } else {
                            // Try to parse stdout as JSON
                            match serde_json::from_str::<Value>(&script_result.stdout) {
                                Ok(json) => {
                                    let metrics = json.get("metrics").cloned();
                                    let score = json.get("score").and_then(|v| v.as_f64());
                                    let summary = json
                                        .get("summary")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());

                                    EvaluatorResult {
                                        name: entry.name.clone(),
                                        metrics,
                                        score,
                                        summary,
                                        error: None,
                                    }
                                }
                                Err(e) => {
                                    // Not valid JSON, use stdout as summary
                                    EvaluatorResult {
                                        name: entry.name.clone(),
                                        metrics: None,
                                        score: None,
                                        summary: Some(script_result.stdout.trim().to_string()),
                                        error: Some(format!("Invalid JSON output: {}", e)),
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => EvaluatorResult {
                        name: entry.name.clone(),
                        metrics: None,
                        score: None,
                        summary: None,
                        error: Some(format!("Execution failed: {}", e)),
                    },
                }
            } else {
                EvaluatorResult {
                    name: entry.name.clone(),
                    metrics: None,
                    score: None,
                    summary: None,
                    error: Some("Script runner not available".to_string()),
                }
            };

            if let Some(ref err) = result.error {
                eprintln!("Evaluator '{}' failed: {:?}", entry.name, err);
            } else if let Some(ref summary) = result.summary {
                println!("Evaluator '{}' result: {}", entry.name, summary);
            }

            results.push(result);
        }
    }

    results
}

fn build_interaction_profile(
    env_root: &Path,
    target_binary: &str,
    command_pattern: Option<&str>,
    interaction_input: &InteractionInput,
    completed: bool,
    supports_structured_tool_calls: bool,
) -> Result<crate::interaction_profile::InteractionProfile> {
    crate::interaction_profile::build_interaction_profile(
        crate::interaction_profile::InteractionProfileInput {
            env_root,
            target_binary,
            command_pattern,
            interaction_input,
            completed,
            supports_structured_tool_calls,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn build_metrics(
    scenario: &Scenario,
    details: Vec<GateResult>,
    gates_passed: usize,
    judge_score: Option<f64>,
    judge_response: Option<JudgeResponse>,
    judge_passed: Option<bool>,
    interaction_profile: InteractionProfile,
) -> EvaluationMetrics {
    let evidence_source = interaction_profile.evidence_source;
    let efficiency = interaction_profile.metrics;
    let composite_score = scenario.evaluation.composite.as_ref().map(|weights| {
        crate::eval_helpers::compute_composite_score(
            judge_score,
            gates_passed,
            scenario.evaluation.gates.len(),
            &efficiency,
            Some(weights),
        )
    });

    EvaluationMetrics {
        gates_passed,
        gates_total: scenario.evaluation.gates.len(),
        details,
        judge_score,
        judge_response,
        judge_passed,
        efficiency,
        interaction_evidence_source: evidence_source,
        composite_score,
        evaluator_results: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    scenario: &Scenario,
    env_root: &Path,
    no_judge: bool,
    script_runner: Option<&ScriptRunner>,
    judge_model: Option<&str>,
    judge_tool: Option<&str>,
    interaction_input: &InteractionInput,
    completed: bool,
    supports_structured_tool_calls: bool,
) -> Result<EvaluationMetrics> {
    println!("Evaluating results for scenario: {}", scenario.name);

    let interaction_profile = build_interaction_profile(
        env_root,
        &scenario.target.binary,
        scenario.target.command_pattern.as_deref(),
        interaction_input,
        completed,
        supports_structured_tool_calls,
    )?;

    let ctx = EvaluationContext {
        env_root,
        script_runner,
        interaction_profile: &interaction_profile,
    };

    let (details, gates_passed) = evaluate_gates(&scenario.evaluation.gates, &ctx);
    let gates_total = scenario.evaluation.gates.len();
    let (judge_score, judge_response, judge_passed) = maybe_run_judge(
        scenario,
        env_root,
        no_judge,
        gates_passed,
        gates_total,
        judge_model,
        judge_tool,
    )?;
    let mut metrics = build_metrics(
        scenario,
        details,
        gates_passed,
        judge_score,
        judge_response,
        judge_passed,
        interaction_profile,
    );

    // Run custom evaluators after gates and judge evaluation
    metrics.evaluator_results = run_evaluators(scenario, script_runner);

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_env() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn judge_tool_resolution_prefers_cli_then_config_then_default() {
        let mut judge_config = JudgeConfig {
            enabled: true,
            tool: Some("codex".to_string()),
            rubric: "rubrics/test.yaml".to_string(),
            pass_threshold: 0.7,
        };

        assert_eq!(
            resolve_judge_tool(&judge_config, Some("claude-code")),
            "claude-code"
        );
        assert_eq!(resolve_judge_tool(&judge_config, None), "codex");

        judge_config.tool = None;
        assert_eq!(resolve_judge_tool(&judge_config, None), "opencode");
    }

    #[test]
    fn judge_command_uses_tool_specific_invocation() {
        assert_eq!(
            build_judge_command("opencode", Some("gpt-4o-mini"), "judge prompt").unwrap(),
            JudgeCommand {
                binary: "opencode".to_string(),
                args: vec![
                    "run".to_string(),
                    "--model".to_string(),
                    "gpt-4o-mini".to_string(),
                    "judge prompt".to_string(),
                ],
            }
        );

        assert_eq!(
            build_judge_command("codex", Some("gpt-5.1"), "judge prompt").unwrap(),
            JudgeCommand {
                binary: "codex".to_string(),
                args: vec![
                    "exec".to_string(),
                    "--full-auto".to_string(),
                    "--skip-git-repo-check".to_string(),
                    "--model".to_string(),
                    "gpt-5.1".to_string(),
                    "judge prompt".to_string(),
                ],
            }
        );

        assert_eq!(
            build_judge_command("claude-code", Some("claude-haiku"), "judge prompt").unwrap(),
            JudgeCommand {
                binary: "claude".to_string(),
                args: vec![
                    "run".to_string(),
                    "--model".to_string(),
                    "claude-haiku".to_string(),
                    "judge prompt".to_string(),
                ],
            }
        );
    }

    #[test]
    fn judge_command_skips_default_codex_model() {
        assert_eq!(
            build_judge_command("codex", Some("default"), "judge prompt").unwrap(),
            JudgeCommand {
                binary: "codex".to_string(),
                args: vec![
                    "exec".to_string(),
                    "--full-auto".to_string(),
                    "--skip-git-repo-check".to_string(),
                    "judge prompt".to_string(),
                ],
            }
        );
    }

    #[test]
    fn judge_command_rejects_unknown_tool() {
        let error = build_judge_command("unknown", None, "judge prompt").unwrap_err();
        assert!(error.to_string().contains("Unsupported judge tool"));
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
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

        let result = eval_script("true", "should pass", 30, Some(&runner));
        assert!(result.passed, "Exit code 0 should pass: {}", result.message);
    }

    #[test]
    fn script_gate_with_exit_code_failure() {
        let temp = tempfile::tempdir().unwrap();
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

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
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

        // Script outputs JSON with passed=true
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
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

        // Script outputs JSON with passed=false
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

    #[test]
    fn evaluator_script_success_with_json_output() {
        let temp = tempfile::tempdir().unwrap();
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

        // Create evaluator scenario
        let mut scenario = create_test_scenario();
        scenario.scripts = Some(crate::scenario::types::ScriptsConfig {
            post: vec![],
            evaluators: vec![crate::scenario::types::EvaluatorEntry {
                command: "echo '{\"score\": 0.85, \"summary\": \"Good performance\", \"metrics\": {\"tokens\": 150}}'".to_string(),
                name: "performance_check".to_string(),
                timeout_secs: 60,
            }],
        });

        let results = run_evaluators(&scenario, Some(&runner));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "performance_check");
        assert!(results[0].error.is_none());
        assert_eq!(results[0].score, Some(0.85));
        assert_eq!(results[0].summary, Some("Good performance".to_string()));
        assert!(results[0].metrics.is_some());
    }

    #[test]
    fn evaluator_script_failure_exit_code() {
        let temp = tempfile::tempdir().unwrap();
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

        let mut scenario = create_test_scenario();
        scenario.scripts = Some(crate::scenario::types::ScriptsConfig {
            post: vec![],
            evaluators: vec![crate::scenario::types::EvaluatorEntry {
                command: "exit 1".to_string(),
                name: "failing_eval".to_string(),
                timeout_secs: 60,
            }],
        });

        let results = run_evaluators(&scenario, Some(&runner));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "failing_eval");
        assert!(results[0].error.is_some());
        assert!(results[0].error.as_ref().unwrap().contains("Exit code 1"));
    }

    #[test]
    fn evaluator_script_timeout() {
        let temp = tempfile::tempdir().unwrap();
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

        let mut scenario = create_test_scenario();
        scenario.scripts = Some(crate::scenario::types::ScriptsConfig {
            post: vec![],
            evaluators: vec![crate::scenario::types::EvaluatorEntry {
                command: "sleep 10".to_string(),
                name: "slow_eval".to_string(),
                timeout_secs: 1, // 1 second timeout
            }],
        });

        let results = run_evaluators(&scenario, Some(&runner));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "slow_eval");
        assert!(results[0].error.is_some());
        assert!(results[0].error.as_ref().unwrap().contains("Timed out"));
    }

    #[test]
    fn evaluator_no_scripts_config() {
        let temp = tempfile::tempdir().unwrap();
        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            std::path::PathBuf::from("/tmp/results"),
            "test".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            std::collections::HashMap::new(),
        );

        let scenario = create_test_scenario(); // No scripts config
        let results = run_evaluators(&scenario, Some(&runner));
        assert!(results.is_empty());
    }

    #[test]
    fn evaluator_no_runner_returns_error() {
        let mut scenario = create_test_scenario();
        scenario.scripts = Some(crate::scenario::types::ScriptsConfig {
            post: vec![],
            evaluators: vec![crate::scenario::types::EvaluatorEntry {
                command: "echo test".to_string(),
                name: "no_runner_test".to_string(),
                timeout_secs: 60,
            }],
        });

        let results = run_evaluators(&scenario, None);
        assert_eq!(results.len(), 1);
        assert!(results[0].error.is_some());
        assert!(results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("Script runner not available"));
    }

    fn create_test_scenario() -> Scenario {
        use crate::scenario::{Evaluation, TargetConfig, Task};

        Scenario {
            name: "test".to_string(),
            description: "Test scenario".to_string(),
            template_folder: "test_fixture".to_string(),
            target: TargetConfig {
                binary: "test".to_string(),
                command_pattern: None,
                health_check: None,
                env: None,
            },
            task: Task {
                prompt: "Test prompt".to_string(),
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
}
