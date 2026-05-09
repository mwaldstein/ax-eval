mod gates;

use self::gates::{evaluate_gates, GateEvaluationContext};
use crate::interaction_profile::InteractionProfile;
use crate::judge::{load_rubric, JudgeResponse};
use crate::scenario::{JudgeConfig, Scenario};
use crate::script_runner::ScriptRunner;
use crate::transcript::InteractionInput;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::Path;

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

    let ctx = GateEvaluationContext {
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
