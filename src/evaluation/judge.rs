use crate::judge::{load_rubric, Criterion, JudgeResponse, OutputFormat, Rubric};
use crate::scenario::{JudgeConfig, Scenario};
use anyhow::{Context, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct JudgeEvaluationResult {
    pub score: Option<f64>,
    pub response: Option<JudgeResponse>,
    pub passed: Option<bool>,
    pub threshold: Option<f64>,
}

impl JudgeEvaluationResult {
    pub fn skipped() -> Self {
        Self {
            score: None,
            response: None,
            passed: None,
            threshold: None,
        }
    }

    pub fn skipped_with_threshold(threshold: f64) -> Self {
        Self {
            threshold: Some(threshold),
            ..Self::skipped()
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
        if judge_config.enabled {
            if no_judge {
                return Ok(JudgeEvaluationResult::skipped_with_threshold(
                    judge_config.pass_threshold,
                ));
            }
            if gates_passed < gates_total {
                println!(
                    "Skipping judge: {}/{} gates failed",
                    gates_total - gates_passed,
                    gates_total
                );
                return Ok(JudgeEvaluationResult::skipped_with_threshold(
                    judge_config.pass_threshold,
                ));
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
                threshold: Some(judge_config.pass_threshold),
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
    let rubric = resolve_judge_rubric(judge_config)?;

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

    let response = parse_judge_response(&output)?;

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

fn resolve_judge_rubric(judge_config: &JudgeConfig) -> Result<Rubric> {
    if let Some(rubric) = &judge_config.rubric {
        let rubric_path = crate::utils::resolve_fixtures_path(rubric);
        return load_rubric(&rubric_path)
            .with_context(|| format!("Failed to load rubric from {}", rubric_path.display()));
    }

    if !judge_config.criteria.is_empty() {
        let rubric = Rubric {
            criteria: judge_config.criteria.clone(),
            output: default_output_format(),
        };
        validate_rubric_weights(&rubric)?;
        return Ok(rubric);
    }

    Ok(default_judge_rubric())
}

fn default_judge_rubric() -> Rubric {
    Rubric {
        criteria: vec![
            Criterion {
                id: "task_completion".to_string(),
                weight: 0.50,
                description: "The agent achieved the user's requested goal and produced the intended outcome, regardless of the exact command sequence used".to_string(),
            },
            Criterion {
                id: "tool_usage_correctness".to_string(),
                weight: 0.30,
                description: "The agent used the CLI tool correctly with valid commands, appropriate arguments, and suitable verification".to_string(),
            },
            Criterion {
                id: "efficiency".to_string(),
                weight: 0.20,
                description:
                    "The agent completed the task without unnecessary commands, repeated dead ends, or avoidable confusion"
                        .to_string(),
            },
        ],
        output: default_output_format(),
    }
}

fn default_output_format() -> OutputFormat {
    OutputFormat {
        format: "json".to_string(),
        require_fields: vec![
            "scores".to_string(),
            "weighted_score".to_string(),
            "confidence".to_string(),
            "issues".to_string(),
            "highlights".to_string(),
            "rationale".to_string(),
        ],
    }
}

fn validate_rubric_weights(rubric: &Rubric) -> Result<()> {
    let total_weight: f64 = rubric.criteria.iter().map(|c| c.weight).sum();
    if (total_weight - 1.0).abs() > 0.01 {
        anyhow::bail!(
            "Rubric criterion weights must sum to 1.0, got {}",
            total_weight
        );
    }
    Ok(())
}

fn parse_judge_response(output: &str) -> Result<JudgeResponse> {
    if let Ok(response) = serde_json::from_str::<JudgeResponse>(output) {
        return Ok(response);
    }

    if let Some(enveloped) = extract_judge_result_envelope(output) {
        if let Ok(response) = serde_json::from_str::<JudgeResponse>(enveloped.trim()) {
            return Ok(response);
        }
    }

    for (index, _) in output.match_indices('{') {
        let mut stream =
            serde_json::Deserializer::from_str(&output[index..]).into_iter::<JudgeResponse>();
        if let Some(Ok(response)) = stream.next() {
            return Ok(response);
        }
    }

    anyhow::bail!("Failed to parse judge response: {}", output)
}

fn extract_judge_result_envelope(output: &str) -> Option<&str> {
    let start_tag = "<judge_result>";
    let end_tag = "</judge_result>";
    let start = output.find(start_tag)? + start_tag.len();
    let end = output[start..].find(end_tag)? + start;
    Some(&output[start..end])
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
            rubric: Some("rubrics/test.yaml".to_string()),
            criteria: vec![],
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
    fn judge_rubric_defaults_to_goal_oriented_criteria() {
        let judge_config = JudgeConfig {
            enabled: true,
            tool: None,
            rubric: None,
            criteria: vec![],
            pass_threshold: 0.7,
        };

        let rubric = resolve_judge_rubric(&judge_config).expect("default rubric");

        assert_eq!(rubric.criteria.len(), 3);
        assert_eq!(rubric.criteria[0].id, "task_completion");
        assert!(rubric.criteria[0]
            .description
            .contains("achieved the user's requested goal"));
        assert!(!rubric.criteria[0].description.contains("required steps"));
    }

    #[test]
    fn judge_rubric_uses_inline_criteria_when_no_rubric_path_is_set() {
        let judge_config = JudgeConfig {
            enabled: true,
            tool: None,
            rubric: None,
            criteria: vec![
                Criterion {
                    id: "goal".to_string(),
                    weight: 0.60,
                    description: "Goal achievement".to_string(),
                },
                Criterion {
                    id: "quality".to_string(),
                    weight: 0.40,
                    description: "Quality".to_string(),
                },
            ],
            pass_threshold: 0.7,
        };

        let rubric = resolve_judge_rubric(&judge_config).expect("inline rubric");

        assert_eq!(rubric.criteria.len(), 2);
        assert_eq!(rubric.criteria[0].id, "goal");
        assert_eq!(rubric.output.format, "json");
        assert!(rubric
            .output
            .require_fields
            .contains(&"rationale".to_string()));
    }

    #[test]
    fn judge_response_parser_accepts_fenced_json_with_terminal_text() {
        let output = r#"opencode output

```json
{
  "scores": {
    "completion": 1.0,
    "quality": 0.93
  },
  "weighted_score": 0.965,
  "confidence": 0.9,
  "issues": [],
  "highlights": ["Completed the requested workflow"],
  "rationale": "The agent completed the task and verified the result."
}
```

done
"#;

        let response = parse_judge_response(output).expect("parse fenced judge response");

        assert_eq!(response.weighted_score, 0.965);
        assert_eq!(response.confidence, 0.9);
        assert_eq!(
            response.highlights,
            vec!["Completed the requested workflow".to_string()]
        );
    }

    #[test]
    fn judge_response_parser_prefers_judge_result_envelope() {
        let output = r#"terminal text before
<judge_result>
{
  "scores": {
    "completion": 1.0
  },
  "weighted_score": 1.0,
  "confidence": 0.95,
  "issues": [],
  "highlights": ["Used the target tool effectively"],
  "rationale": "The workflow was completed cleanly."
}
</judge_result>
terminal text after
"#;

        let response = parse_judge_response(output).expect("parse enveloped judge response");

        assert_eq!(response.weighted_score, 1.0);
        assert_eq!(
            response.highlights,
            vec!["Used the target tool effectively".to_string()]
        );
    }

    #[test]
    fn judge_response_parser_does_not_parse_prose_inside_judge_result() {
        let output = r#"<judge_result>
This is not JSON.
</judge_result>
{
  "scores": {
    "fallback": 0.7
  },
  "weighted_score": 0.7,
  "confidence": 0.8,
  "issues": ["Envelope was malformed"],
  "highlights": [],
  "rationale": "Fallback still found valid JSON."
}
"#;

        let response = parse_judge_response(output).expect("parse fallback judge response");

        assert_eq!(response.weighted_score, 0.7);
        assert_eq!(response.issues, vec!["Envelope was malformed".to_string()]);
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
