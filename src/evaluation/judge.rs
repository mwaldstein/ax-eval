use crate::judge::{load_rubric, JudgeResponse};
use crate::scenario::{JudgeConfig, Scenario};
use anyhow::{Context, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct JudgeEvaluationResult {
    pub score: Option<f64>,
    pub response: Option<JudgeResponse>,
    pub passed: Option<bool>,
}

impl JudgeEvaluationResult {
    pub fn skipped() -> Self {
        Self {
            score: None,
            response: None,
            passed: None,
        }
    }
}

#[derive(Debug, Clone)]
struct JudgeExecutionResult {
    score: Option<f64>,
    response: Option<JudgeResponse>,
}

impl JudgeExecutionResult {
    fn from_response(response: JudgeResponse) -> Self {
        Self {
            score: Some(response.weighted_score),
            response: Some(response),
        }
    }
}

pub fn maybe_run_judge(
    scenario: &Scenario,
    env_root: &Path,
    no_judge: bool,
    gates_passed: usize,
    gates_total: usize,
    judge_model: Option<&str>,
    judge_tool: Option<&str>,
) -> Result<JudgeEvaluationResult> {
    if let Some(judge_config) = &scenario.evaluation.judge {
        if judge_config.enabled && !no_judge {
            if gates_passed < gates_total {
                println!(
                    "Skipping judge: {}/{} gates failed",
                    gates_total - gates_passed,
                    gates_total
                );
                return Ok(JudgeEvaluationResult::skipped());
            }
            let execution =
                run_judge_evaluation(judge_config, judge_model, judge_tool, scenario, env_root)?;
            let passed = execution.score.map(|s| s >= judge_config.pass_threshold);
            if let Some(s) = execution.score {
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
            return Ok(JudgeEvaluationResult {
                score: execution.score,
                response: execution.response,
                passed,
            });
        }
    }
    Ok(JudgeEvaluationResult::skipped())
}

fn run_judge_evaluation(
    judge_config: &JudgeConfig,
    judge_model: Option<&str>,
    judge_tool: Option<&str>,
    scenario: &Scenario,
    env_root: &Path,
) -> Result<JudgeExecutionResult> {
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

    Ok(JudgeExecutionResult::from_response(response))
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
    fn judge_evaluation_result_represents_skipped_judge() {
        let result = JudgeEvaluationResult::skipped();

        assert_eq!(result.score, None);
        assert!(result.response.is_none());
        assert_eq!(result.passed, None);
    }

    #[test]
    fn judge_execution_result_carries_response_score() {
        let response = JudgeResponse {
            weighted_score: 0.82,
            confidence: 0.9,
            rationale: "solid".to_string(),
            issues: vec![],
            highlights: vec!["clear".to_string()],
            scores: std::collections::HashMap::new(),
        };

        let result = JudgeExecutionResult::from_response(response);

        assert_eq!(result.score, Some(0.82));
        assert_eq!(
            result
                .response
                .as_ref()
                .map(|response| response.rationale.as_str()),
            Some("solid")
        );
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
}
