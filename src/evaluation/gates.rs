mod assertions;
mod command;
mod file;
mod interaction;
mod script;

use crate::interaction_profile::InteractionProfile;
use crate::scenario::Gate;
use crate::script_runner::ScriptRunner;
use crate::target_env::TargetEnvironment;
use std::path::Path;

use super::{GateResult, GateStatus};

/// Context passed to gate evaluators, containing environment and optional script runner.
pub struct GateEvaluationContext<'a> {
    pub env_root: &'a Path,
    pub target_env: &'a TargetEnvironment,
    pub script_runner: Option<&'a ScriptRunner>,
    pub interaction_profile: &'a InteractionProfile,
}

trait GateEvaluator {
    fn evaluate(&self, ctx: &GateEvaluationContext<'_>) -> GateResult;
}

impl GateEvaluator for Gate {
    fn evaluate(&self, ctx: &GateEvaluationContext<'_>) -> GateResult {
        match self {
            Gate::CommandSucceeds { command } => {
                command::eval_command_succeeds(command, ctx.env_root, ctx.target_env)
            }
            Gate::CommandOutputContains { command, substring } => {
                command::eval_command_output_contains(
                    command,
                    substring,
                    ctx.env_root,
                    ctx.target_env,
                )
            }
            Gate::CommandOutputMatches { command, pattern } => {
                command::eval_command_output_matches(command, pattern, ctx.env_root, ctx.target_env)
            }
            Gate::CommandJsonPath {
                command,
                path,
                assertion,
            } => command::eval_command_json_path(
                command,
                path,
                assertion,
                ctx.env_root,
                ctx.target_env,
            ),
            Gate::FileExists { path } => file::eval_file_exists(path, ctx.env_root),
            Gate::FileContains { path, substring } => {
                file::eval_file_contains(path, substring, ctx.env_root)
            }
            Gate::FileMatches { path, pattern } => {
                file::eval_file_matches(path, pattern, ctx.env_root)
            }
            Gate::NoTranscriptErrors => {
                interaction::eval_no_transcript_errors(ctx.interaction_profile)
            }
            Gate::Script {
                command,
                description,
                timeout_secs,
            } => script::eval_script(command, description, *timeout_secs, ctx.script_runner),
        }
    }
}

pub fn evaluate_gates(
    gates: &[Gate],
    ctx: &GateEvaluationContext<'_>,
) -> (Vec<GateResult>, GateStatus) {
    let mut details = Vec::new();

    for gate in gates {
        let result = gate.evaluate(ctx);

        if result.passed {
            println!("Gate {} passed: {}", result.gate_type, result.message);
        } else {
            println!("Gate {} FAILED: {}", result.gate_type, result.message);
        }
        details.push(result);
    }

    let status = GateStatus::from_details(gates.len(), &details);
    (details, status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script_runner::ScriptRunnerConfig;
    use crate::target_env::TargetEnvironment;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    fn temp_env() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn test_runner(fixture_dir: PathBuf) -> ScriptRunner {
        ScriptRunner::new(ScriptRunnerConfig {
            fixture_dir,
            results_dir: PathBuf::from("/tmp/results"),
            scenario_name: "test".to_string(),
            agent: "test_agent".to_string(),
            model: "test_model".to_string(),
            transcript_path: None,
            events_path: None,
            target_env: HashMap::new(),
        })
    }

    fn empty_target_env() -> TargetEnvironment {
        TargetEnvironment::default()
    }

    fn interaction_profile() -> crate::interaction_profile::InteractionProfile {
        crate::interaction_profile::InteractionProfile {
            metrics: crate::transcript::EfficiencyMetrics {
                total_commands: 0,
                unique_commands: 0,
                error_count: 0,
                retry_count: 0,
                help_invocations: 0,
                first_try_success_rate: 0.0,
                iteration_ratio: 0.0,
                completed: true,
            },
            evidence_source:
                crate::interaction_profile::InteractionEvidenceSource::StructuredToolCalls,
            warnings: vec![],
        }
    }

    #[test]
    fn evaluate_gates_derives_not_configured_for_empty_gate_list() {
        let env = temp_env();
        let target_env = empty_target_env();
        let interaction_profile = interaction_profile();
        let ctx = GateEvaluationContext {
            env_root: env.path(),
            target_env: &target_env,
            script_runner: None,
            interaction_profile: &interaction_profile,
        };

        let (details, status) = evaluate_gates(&[], &ctx);

        assert!(details.is_empty());
        assert_eq!(status, GateStatus::NotConfigured);
    }

    #[test]
    fn evaluate_gates_derives_passed_when_all_gates_pass() {
        let env = temp_env();
        fs::write(env.path().join("summary.md"), "done").expect("write fixture");
        let target_env = empty_target_env();
        let interaction_profile = interaction_profile();
        let ctx = GateEvaluationContext {
            env_root: env.path(),
            target_env: &target_env,
            script_runner: None,
            interaction_profile: &interaction_profile,
        };
        let gates = vec![Gate::FileExists {
            path: "summary.md".to_string(),
        }];

        let (_, status) = evaluate_gates(&gates, &ctx);

        assert_eq!(status, GateStatus::Passed);
    }

    #[test]
    fn evaluate_gates_derives_failed_when_any_gate_fails() {
        let env = temp_env();
        let target_env = empty_target_env();
        let interaction_profile = interaction_profile();
        let ctx = GateEvaluationContext {
            env_root: env.path(),
            target_env: &target_env,
            script_runner: None,
            interaction_profile: &interaction_profile,
        };
        let gates = vec![Gate::FileExists {
            path: "summary.md".to_string(),
        }];

        let (details, status) = evaluate_gates(&gates, &ctx);

        assert_eq!(status, GateStatus::Failed);
        assert_eq!(details[0].identifier, "file_exists(summary.md)");
    }

    #[test]
    fn command_succeeds_gate_passes_for_successful_command() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_succeeds("true", env.path(), &target_env);
        assert!(result.passed);
    }

    #[test]
    fn command_succeeds_gate_fails_for_failing_command() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_succeeds("false", env.path(), &target_env);
        assert!(!result.passed);
    }

    #[test]
    fn command_output_contains_gate_checks_stdout_substring() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_output_contains(
            "printf 'hello world'",
            "hello",
            env.path(),
            &target_env,
        );
        assert!(result.passed);
    }

    #[test]
    fn command_output_matches_gate_checks_stdout_regex() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_output_matches(
            "printf 'abc-123'",
            r"abc-\d+",
            env.path(),
            &target_env,
        );
        assert!(result.passed);
    }

    #[test]
    fn command_json_path_gate_supports_exists_assertion() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_json_path(
            "printf '{\"meta\":{\"ok\":true}}'",
            "$.meta.ok",
            "exists",
            env.path(),
            &target_env,
        );
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn command_json_path_gate_supports_equals_assertion() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_json_path(
            "printf '{\"count\":3}'",
            "$.count",
            "equals 3",
            env.path(),
            &target_env,
        );
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn command_json_path_gate_supports_contains_assertion() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_json_path(
            "printf '{\"msg\":\"build succeeded\"}'",
            "$.msg",
            "contains succeeded",
            env.path(),
            &target_env,
        );
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn command_json_path_gate_supports_len_assertion() {
        let env = temp_env();
        let target_env = empty_target_env();
        let result = command::eval_command_json_path(
            "printf '{\"items\":[1,2,3]}'",
            "$.items",
            "len >= 3",
            env.path(),
            &target_env,
        );
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn command_gates_receive_target_env_vars() {
        let env = temp_env();
        let mut vars = HashMap::new();
        vars.insert("TARGET_ENV_TEST".to_string(), "works".to_string());
        let target_env =
            TargetEnvironment::expanded_from_config(Some(&vars), env.path(), env.path())
                .expect("target env");

        let result = command::eval_command_succeeds(
            "test \"$TARGET_ENV_TEST\" = \"works\"",
            env.path(),
            &target_env,
        );

        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn file_exists_gate_checks_relative_path() {
        let env = temp_env();
        fs::write(env.path().join("result.txt"), "ok").expect("write file");

        let result = file::eval_file_exists("result.txt", env.path());
        assert!(result.passed);
    }

    #[test]
    fn file_contains_gate_checks_file_contents() {
        let env = temp_env();
        fs::write(env.path().join("notes.md"), "status: complete").expect("write file");

        let result = file::eval_file_contains("notes.md", "complete", env.path());
        assert!(result.passed);
    }

    #[test]
    fn file_matches_gate_checks_file_regex() {
        let env = temp_env();
        fs::write(env.path().join("logs.txt"), "run-42 done").expect("write file");

        let result = file::eval_file_matches("logs.txt", r"run-\d+", env.path());
        assert!(result.passed);
    }

    #[test]
    fn script_gate_with_exit_code_success() {
        let temp = tempfile::tempdir().unwrap();
        let runner = test_runner(temp.path().to_path_buf());

        let result = script::eval_script("true", "should pass", 30, Some(&runner));
        assert!(result.passed, "Exit code 0 should pass: {}", result.message);
    }

    #[test]
    fn script_gate_with_exit_code_failure() {
        let temp = tempfile::tempdir().unwrap();
        let runner = test_runner(temp.path().to_path_buf());

        let result = script::eval_script("false", "should fail", 30, Some(&runner));
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

        let result = script::eval_script(
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

        let result = script::eval_script(
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
        let result = script::eval_script("true", "no runner", 30, None);
        assert!(!result.passed);
        assert!(result.message.contains("Script runner not available"));
    }
}
