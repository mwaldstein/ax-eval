mod evaluators;
mod gates;
mod judge;
mod profile;

use self::evaluators::run_evaluators;
use self::gates::{evaluate_gates, GateEvaluationContext};
use self::judge::maybe_run_judge;
pub use self::profile::{EvaluationMetrics, EvaluatorResult, GateResult, ScoreTier};
use crate::interaction_profile::{
    AdapterEvidenceCapability, InteractionProfile, InteractionProfileInput, TargetInteractionSpec,
};
use crate::judge::JudgeResponse;
use crate::scenario::Scenario;
use crate::script_runner::ScriptRunner;
use crate::target_env::TargetEnvironment;
use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;

struct MetricsBuildInput<'a> {
    scenario: &'a Scenario,
    details: Vec<GateResult>,
    gates_passed: usize,
    judge_score: Option<f64>,
    judge_response: Option<JudgeResponse>,
    judge_passed: Option<bool>,
    judge_threshold: Option<f64>,
    interaction_profile: InteractionProfile,
}

fn build_metrics(input: MetricsBuildInput<'_>) -> EvaluationMetrics {
    let scenario = input.scenario;
    let judge_score = input.judge_score;
    let interaction_profile = input.interaction_profile;
    let evidence_source = interaction_profile.evidence_source;
    let efficiency = interaction_profile.metrics;
    let composite_score = scenario.evaluation.composite.as_ref().map(|weights| {
        crate::eval_helpers::compute_composite_score(
            judge_score,
            input.gates_passed,
            scenario.evaluation.gates.len(),
            &efficiency,
            Some(weights),
        )
    });

    EvaluationMetrics {
        gates_passed: input.gates_passed,
        gates_total: scenario.evaluation.gates.len(),
        details: input.details,
        judge_score,
        judge_response: input.judge_response,
        judge_passed: input.judge_passed,
        judge_threshold: input.judge_threshold,
        efficiency,
        interaction_evidence_source: evidence_source,
        composite_score,
        evaluator_results: Vec::new(),
    }
}

pub struct EvaluationInput<'a> {
    pub scenario: &'a Scenario,
    pub env_root: &'a Path,
    pub no_judge: bool,
    pub script_runner: Option<&'a ScriptRunner>,
    pub judge_model: Option<&'a str>,
    pub judge_tool: Option<&'a str>,
    pub interaction_input: &'a crate::transcript::InteractionInput,
    pub adapter_capability: AdapterEvidenceCapability,
    pub transcript_path: PathBuf,
    pub completed: bool,
    pub target_env: &'a TargetEnvironment,
}

pub fn evaluate(input: EvaluationInput<'_>) -> Result<EvaluationMetrics> {
    let scenario = input.scenario;
    println!("Evaluating results for scenario: {}", scenario.name);

    let target = TargetInteractionSpec::new(
        scenario.target.binary.clone(),
        scenario.target.command_pattern.clone(),
    );
    let interaction_profile =
        crate::interaction_profile::build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: input.interaction_input,
            adapter_capability: input.adapter_capability,
            transcript_path: input.transcript_path,
            completed: input.completed,
            target_command_policy: scenario.interaction.target_commands,
        })?;

    let ctx = GateEvaluationContext {
        env_root: input.env_root,
        target_env: input.target_env,
        script_runner: input.script_runner,
        interaction_profile: &interaction_profile,
    };

    let (details, gates_passed) = evaluate_gates(&scenario.evaluation.gates, &ctx);
    let gates_total = scenario.evaluation.gates.len();
    let judge_result = maybe_run_judge(
        scenario,
        input.env_root,
        input.no_judge,
        gates_passed,
        gates_total,
        input.judge_model,
        input.judge_tool,
    )?;
    let mut metrics = build_metrics(MetricsBuildInput {
        scenario,
        details,
        gates_passed,
        judge_score: judge_result.score,
        judge_response: judge_result.response,
        judge_passed: judge_result.passed,
        judge_threshold: judge_result.threshold,
        interaction_profile,
    });

    // Run custom evaluators after gates and judge evaluation
    metrics.evaluator_results = run_evaluators(scenario, input.script_runner);

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Evaluation, Scenario, TargetConfig, Task};
    use crate::transcript::{CommandEvent, InteractionInput};

    fn scenario() -> Scenario {
        Scenario {
            name: "evaluation-input-test".to_string(),
            description: "Test evaluation input".to_string(),
            template_folder: "fixture".to_string(),
            target: TargetConfig {
                binary: "target".to_string(),
                command_pattern: None,
                health_check: None,
                env: None,
            },
            task: Task {
                prompt: "Do the task".to_string(),
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

    #[test]
    fn evaluation_input_runs_evaluation_with_structured_interactions() {
        let scenario = scenario();
        let dir = tempfile::tempdir().expect("tempdir");
        let interaction_input = InteractionInput::StructuredToolCalls(vec![CommandEvent {
            command: "target status".to_string(),
            exit_code: Some(0),
        }]);

        let metrics = evaluate(EvaluationInput {
            scenario: &scenario,
            env_root: dir.path(),
            no_judge: true,
            script_runner: None,
            judge_model: None,
            judge_tool: None,
            interaction_input: &interaction_input,
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: dir.path().join("unused-transcript.raw.txt"),
            completed: true,
            target_env: &TargetEnvironment::default(),
        })
        .expect("evaluate");

        assert_eq!(metrics.gates_passed, 0);
        assert_eq!(metrics.gates_total, 0);
        assert_eq!(metrics.efficiency.total_commands, 1);
        assert!(metrics.efficiency.completed);
    }

    #[test]
    fn metrics_build_input_builds_metrics_record() {
        let scenario = scenario();
        let interaction_profile = InteractionProfile {
            metrics: crate::transcript::EfficiencyMetrics {
                total_commands: 1,
                unique_commands: 1,
                error_count: 0,
                retry_count: 0,
                help_invocations: 0,
                first_try_success_rate: 1.0,
                iteration_ratio: 1.0,
                completed: true,
            },
            evidence_source:
                crate::interaction_profile::InteractionEvidenceSource::StructuredToolCalls,
        };

        let metrics = build_metrics(MetricsBuildInput {
            scenario: &scenario,
            details: vec![],
            gates_passed: 0,
            judge_score: None,
            judge_response: None,
            judge_passed: None,
            judge_threshold: None,
            interaction_profile,
        });

        assert_eq!(metrics.gates_passed, 0);
        assert_eq!(metrics.gates_total, 0);
        assert_eq!(metrics.efficiency.total_commands, 1);
        assert_eq!(
            metrics.interaction_evidence_source,
            crate::interaction_profile::InteractionEvidenceSource::StructuredToolCalls
        );
    }
}
