use super::EvaluatorResult;
use crate::scenario::Scenario;
use crate::script_runner::ScriptRunner;
use serde_json::Value;

/// Run custom evaluator scripts from scenario configuration.
pub fn run_evaluators(
    scenario: &Scenario,
    script_runner: Option<&ScriptRunner>,
) -> Vec<EvaluatorResult> {
    let mut results = Vec::new();

    if let Some(scripts) = &scenario.scripts {
        for entry in &scripts.evaluators {
            println!("Running evaluator '{}'...", entry.name);

            let result = if let Some(runner) = script_runner {
                match runner.run_report(&entry.command, entry.timeout_secs) {
                    Ok(report) => {
                        if let Some(error) = report.failure_summary() {
                            EvaluatorResult {
                                name: entry.name.clone(),
                                metrics: None,
                                score: None,
                                summary: None,
                                error: Some(error),
                            }
                        } else {
                            parse_evaluator_stdout(&entry.name, &report.result.stdout)
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

fn parse_evaluator_stdout(name: &str, stdout: &str) -> EvaluatorResult {
    match serde_json::from_str::<Value>(stdout) {
        Ok(json) => {
            let metrics = json.get("metrics").cloned();
            let score = json.get("score").and_then(|v| v.as_f64());
            let summary = json
                .get("summary")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            EvaluatorResult {
                name: name.to_string(),
                metrics,
                score,
                summary,
                error: None,
            }
        }
        Err(e) => EvaluatorResult {
            name: name.to_string(),
            metrics: None,
            score: None,
            summary: Some(stdout.trim().to_string()),
            error: Some(format!("Invalid JSON output: {}", e)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

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
    fn evaluator_script_success_with_json_output() {
        let temp = tempfile::tempdir().unwrap();
        let runner = test_runner(temp.path().to_path_buf());

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
        let runner = test_runner(temp.path().to_path_buf());

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
        let runner = test_runner(temp.path().to_path_buf());

        let mut scenario = create_test_scenario();
        scenario.scripts = Some(crate::scenario::types::ScriptsConfig {
            post: vec![],
            evaluators: vec![crate::scenario::types::EvaluatorEntry {
                command: "sleep 10".to_string(),
                name: "slow_eval".to_string(),
                timeout_secs: 1,
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
        let runner = test_runner(temp.path().to_path_buf());

        let scenario = create_test_scenario();
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
