mod evaluators;
mod gates;
mod judge;

use self::evaluators::run_evaluators;
use self::gates::{evaluate_gates, GateEvaluationContext};
use self::judge::maybe_run_judge;
use crate::interaction_profile::{
    AdapterEvidenceCapability, InteractionProfile, InteractionProfileInput, TargetInteractionSpec,
};
use crate::judge::JudgeResponse;
use crate::scenario::Scenario;
use crate::script_runner::ScriptRunner;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

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

struct MetricsBuildInput<'a> {
    scenario: &'a Scenario,
    details: Vec<GateResult>,
    gates_passed: usize,
    judge_score: Option<f64>,
    judge_response: Option<JudgeResponse>,
    judge_passed: Option<bool>,
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
        })?;

    let ctx = GateEvaluationContext {
        env_root: input.env_root,
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
