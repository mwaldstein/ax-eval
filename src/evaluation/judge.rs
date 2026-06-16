use crate::adapter::{ToolAdapter, ToolRunOutput};
use crate::judge::{load_rubric, Criterion, JudgeResponse, OutputFormat, Rubric};
use crate::scenario::{Evaluation, JudgeConfig, Scenario, TargetConfig, Task};
use crate::target_env::TargetEnvironment;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::debug;

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

#[allow(clippy::too_many_arguments)]
pub fn maybe_run_judge(
    scenario: &Scenario,
    env_root: &Path,
    scenario_path: &Path,
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
                debug!(
                    "skipping judge: {}/{} gates failed",
                    gates_total - gates_passed,
                    gates_total,
                );
                return Ok(JudgeEvaluationResult::skipped_with_threshold(
                    judge_config.pass_threshold,
                ));
            }
            let execution = run_judge_evaluation(
                judge_config,
                judge_model,
                judge_tool,
                scenario,
                env_root,
                scenario_path,
            )?;
            let passed = execution.score.map(|s| s >= judge_config.pass_threshold);
            if let Some(s) = execution.score {
                if s >= judge_config.pass_threshold {
                    debug!(
                        "judge passed: score {:.2} >= threshold {:.2}",
                        s, judge_config.pass_threshold
                    );
                } else {
                    debug!(
                        "judge failed: score {:.2} < threshold {:.2}",
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
    scenario_path: &Path,
) -> Result<JudgeExecutionResult> {
    let tool = resolve_judge_tool(judge_config, judge_tool);
    debug!(
        "running judge evaluation with tool={}, model={:?}",
        tool, judge_model
    );
    let rubric = resolve_judge_rubric(judge_config, env_root, scenario_path)?;
    let transcript_path = env_root.join("transcript.raw.txt");
    let prompt = crate::judge::build_judge_prompt(
        &scenario.target.binary,
        &scenario.task.prompt,
        &transcript_path.display().to_string(),
        &rubric,
    );

    let mut adapter_registry = crate::adapter::registry::AdapterRegistry::new();
    let adapter = adapter_registry.resolve_checked(tool)?;
    let judge_scenario = judge_scenario(scenario, prompt);
    run_judge_evaluation_with_adapter(adapter.adapter(), judge_model, &judge_scenario, env_root)
}

fn run_judge_evaluation_with_adapter(
    adapter: &dyn ToolAdapter,
    judge_model: Option<&str>,
    judge_scenario: &Scenario,
    env_root: &Path,
) -> Result<JudgeExecutionResult> {
    let output = adapter.run(
        judge_scenario,
        env_root,
        judge_model,
        300,
        &TargetEnvironment::default(),
    )?;

    if output.exit_code != 0 {
        anyhow::bail!(
            "Judge exited with code {}: {}",
            output.exit_code,
            judge_output_text(&output)
        );
    }

    let response = parse_judge_response(&judge_output_text(&output))?;

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

fn judge_scenario(source: &Scenario, prompt: String) -> Scenario {
    Scenario {
        name: format!("{}_judge", source.name),
        description: format!("Judge evaluation for {}", source.name),
        template_folder: ".".to_string(),
        target: TargetConfig {
            binary: source.target.binary.clone(),
            command_pattern: None,
            health_check: None,
            env: None,
        },
        task: Task { prompt },
        evaluation: Evaluation {
            gates: vec![],
            judge: None,
            composite: None,
        },
        tier: source.tier,
        tool_matrix: None,
        setup: None,
        tags: vec!["judge".to_string()],
        run: None,
        scripts: None,
        interaction: Default::default(),
    }
}

fn judge_output_text(output: &ToolRunOutput) -> String {
    output
        .raw_output
        .as_ref()
        .filter(|raw| !raw.trim().is_empty())
        .unwrap_or(&output.transcript)
        .to_string()
}

fn resolve_judge_rubric(
    judge_config: &JudgeConfig,
    env_root: &Path,
    scenario_path: &Path,
) -> Result<Rubric> {
    if let Some(rubric) = &judge_config.rubric {
        let rubric_path = resolve_rubric_path(rubric, env_root, scenario_path);
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

/// Resolve a rubric path in a CWD-agnostic way:
///
/// 1. If absolute, use as-is.
/// 2. If relative and exists next to the scenario file (scenario-relative), use that.
/// 3. If relative and exists under `env_root` (the workspace), use that.
/// 4. Otherwise fall back to `resolve_fixtures_path` (checks CWD, then fixtures_path).
fn resolve_rubric_path(rubric: &str, env_root: &Path, scenario_path: &Path) -> PathBuf {
    let rubric_path = Path::new(rubric);
    if rubric_path.is_absolute() {
        return rubric_path.to_path_buf();
    }
    if let Some(scenario_dir) = scenario_path.parent() {
        let scenario_relative = scenario_dir.join(rubric_path);
        if scenario_relative.exists() {
            return scenario_relative;
        }
    }
    let workspace_path = env_root.join(rubric_path);
    if workspace_path.exists() {
        return workspace_path;
    }
    crate::utils::resolve_fixtures_path(rubric)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{AdapterError, ToolStatus};
    use crate::interaction_evidence::InteractionInput;

    struct JudgeOutputAdapter {
        output: ToolRunOutput,
    }

    impl ToolAdapter for JudgeOutputAdapter {
        fn is_available(&self) -> Result<ToolStatus, AdapterError> {
            Ok(ToolStatus {
                available: true,
                authenticated: true,
            })
        }

        fn run(
            &self,
            scenario: &Scenario,
            _cwd: &Path,
            model: Option<&str>,
            timeout_secs: u64,
            _target_env: &TargetEnvironment,
        ) -> anyhow::Result<ToolRunOutput> {
            assert!(scenario.name.ends_with("_judge"));
            assert!(scenario.task.prompt.contains("Return only valid JSON"));
            assert_eq!(model, Some("judge-model"));
            assert_eq!(timeout_secs, 300);
            Ok(self.output.clone())
        }
    }

    fn scenario() -> Scenario {
        Scenario {
            name: "judge-source".to_string(),
            description: "Scenario judged by adapter".to_string(),
            template_folder: "fixture".to_string(),
            target: TargetConfig {
                binary: "notes".to_string(),
                command_pattern: None,
                health_check: None,
                env: None,
            },
            task: Task {
                prompt: "Create a useful note".to_string(),
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
            interaction: Default::default(),
        }
    }

    fn judge_json(score: f64) -> String {
        serde_json::json!({
            "scores": {"task_completion": score},
            "weighted_score": score,
            "confidence": 0.9,
            "issues": [],
            "highlights": ["Adapter returned judge output"],
            "rationale": "The adapter output was parsed."
        })
        .to_string()
    }

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

        let rubric = resolve_judge_rubric(
            &judge_config,
            std::path::Path::new("/tmp"),
            std::path::Path::new("/tmp"),
        )
        .expect("default rubric");

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

        let rubric = resolve_judge_rubric(
            &judge_config,
            std::path::Path::new("/tmp"),
            std::path::Path::new("/tmp"),
        )
        .expect("inline rubric");

        assert_eq!(rubric.criteria.len(), 2);
        assert_eq!(rubric.criteria[0].id, "goal");
        assert_eq!(rubric.output.format, "json");
        assert!(rubric
            .output
            .require_fields
            .contains(&"rationale".to_string()));
    }

    #[test]
    fn judge_rubric_resolves_scenario_relative_then_workspace_then_fallback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace dir");

        // Create a rubric next to the scenario file
        let scenario_dir = dir.path().join("ax-eval-fixtures");
        std::fs::create_dir_all(&scenario_dir).expect("scenario dir");
        std::fs::create_dir_all(scenario_dir.join("rubrics")).expect("rubrics dir");
        std::fs::write(
            scenario_dir.join("rubrics/scenario_rubric.yaml"),
            r#"
criteria:
  - id: scenario
    weight: 1.0
    description: "Scenario rubric"
output:
  format: json
  require_fields: [scores, rationale]
"#,
        )
        .expect("write scenario rubric");

        // Create a rubric in the workspace
        std::fs::create_dir_all(workspace.join("rubrics")).expect("workspace rubrics dir");
        std::fs::write(
            workspace.join("rubrics/workspace_rubric.yaml"),
            r#"
criteria:
  - id: workspace
    weight: 1.0
    description: "Workspace rubric"
output:
  format: json
  require_fields: [scores, rationale]
"#,
        )
        .expect("write workspace rubric");

        let scenario_path = scenario_dir.join("scenario.yaml");

        // 1. Scenario-relative takes precedence
        let judge_config = JudgeConfig {
            enabled: true,
            tool: None,
            rubric: Some("rubrics/scenario_rubric.yaml".to_string()),
            criteria: vec![],
            pass_threshold: 0.7,
        };
        let rubric = resolve_judge_rubric(&judge_config, &workspace, &scenario_path)
            .expect("scenario-relative rubric");
        assert_eq!(rubric.criteria[0].id, "scenario");

        // 2. Workspace-relative when scenario-relative does not exist
        let judge_config = JudgeConfig {
            enabled: true,
            tool: None,
            rubric: Some("rubrics/workspace_rubric.yaml".to_string()),
            criteria: vec![],
            pass_threshold: 0.7,
        };
        let rubric = resolve_judge_rubric(&judge_config, &workspace, &scenario_path)
            .expect("workspace rubric");
        assert_eq!(rubric.criteria[0].id, "workspace");

        // 3. Fallback when neither exists
        let empty_workspace = dir.path().join("empty");
        std::fs::create_dir_all(&empty_workspace).expect("empty dir");
        let judge_config = JudgeConfig {
            enabled: true,
            tool: None,
            rubric: Some("nonexistent_rubric.yaml".to_string()),
            criteria: vec![],
            pass_threshold: 0.7,
        };
        let result = resolve_judge_rubric(&judge_config, &empty_workspace, &scenario_path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to load rubric"),
            "expected fallback error, got: {err}"
        );
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
    fn judge_scenario_carries_source_task_and_target_context() {
        let source = scenario();
        let scenario = judge_scenario(&source, "judge prompt".to_string());

        assert_eq!(scenario.name, "judge-source_judge");
        assert_eq!(scenario.target.binary, "notes");
        assert_eq!(scenario.task.prompt, "judge prompt");
        assert_eq!(scenario.evaluation.gates.len(), 0);
        assert!(scenario.evaluation.judge.is_none());
        assert_eq!(scenario.tags, vec!["judge".to_string()]);
    }

    #[test]
    fn judge_evaluation_runs_through_adapter_and_parses_raw_output() {
        let source = scenario();
        let judge_scenario = judge_scenario(&source, "Return only valid JSON".to_string());
        let adapter = JudgeOutputAdapter {
            output: ToolRunOutput {
                transcript: "transcript without judge json".to_string(),
                raw_output: Some(judge_json(0.84)),
                exit_code: 0,
                cost_usd: None,
                token_usage: None,
                interaction_input: InteractionInput::TranscriptRegex,
            },
        };

        let result = run_judge_evaluation_with_adapter(
            &adapter,
            Some("judge-model"),
            &judge_scenario,
            Path::new("."),
        )
        .expect("judge evaluation");

        assert_eq!(result.score, Some(0.84));
    }

    #[test]
    fn judge_evaluation_falls_back_to_transcript_when_raw_output_is_absent() {
        let source = scenario();
        let judge_scenario = judge_scenario(&source, "Return only valid JSON".to_string());
        let adapter = JudgeOutputAdapter {
            output: ToolRunOutput {
                transcript: judge_json(0.73),
                raw_output: None,
                exit_code: 0,
                cost_usd: None,
                token_usage: None,
                interaction_input: InteractionInput::TranscriptRegex,
            },
        };

        let result = run_judge_evaluation_with_adapter(
            &adapter,
            Some("judge-model"),
            &judge_scenario,
            Path::new("."),
        )
        .expect("judge evaluation");

        assert_eq!(result.score, Some(0.73));
    }
}
