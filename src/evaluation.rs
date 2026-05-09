mod evaluators;
mod gates;
mod judge;

use self::evaluators::run_evaluators;
use self::gates::{evaluate_gates, GateEvaluationContext};
use self::judge::maybe_run_judge;
use crate::interaction_profile::InteractionProfile;
use crate::judge::JudgeResponse;
use crate::scenario::Scenario;
use crate::script_runner::ScriptRunner;
use crate::transcript::InteractionInput;
use anyhow::Result;
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
