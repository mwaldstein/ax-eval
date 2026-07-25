//! Type definitions for LLM-as-judge evaluation.
//!
//! This module defines the data structures used for rubric-based evaluation,
//! including rubrics, criteria, and judge responses.

use serde::{Deserialize, Serialize};

/// A rubric defining evaluation criteria for LLM tool assessment.
///
/// Rubrics are loaded from YAML files and define weighted criteria
/// for scoring LLM tool performance on a scale from 0.0 to 1.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rubric {
    /// List of evaluation criteria with weights
    pub criteria: Vec<Criterion>,
    /// Output format requirements for judge responses
    pub output: OutputFormat,
}

/// An individual evaluation criterion within a rubric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Criterion {
    /// Unique identifier for this criterion
    pub id: String,
    /// Weight of this criterion (must sum to 1.0 across all criteria)
    pub weight: f64,
    /// Human-readable description of what this criterion measures
    pub description: String,
}

/// Output format requirements for judge responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputFormat {
    /// Response format type (typically "json")
    pub format: String,
    /// Required fields that must be present in the response
    pub require_fields: Vec<String>,
}

/// Response from an LLM-as-judge evaluation.
///
/// Contains scores for each criterion, overall weighted score,
/// qualitative feedback about the evaluation, and a rationale
/// explaining the overall assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeResponse {
    /// Map of criterion IDs to scores (0.0-1.0)
    pub scores: std::collections::HashMap<String, f64>,
    /// Overall weighted score across all criteria (0.0-1.0)
    pub weighted_score: f64,
    /// Confidence level in the evaluation (0.0-1.0)
    pub confidence: f64,
    /// List of issues or problems identified
    pub issues: Vec<String>,
    /// List of positive highlights or good practices observed
    pub highlights: Vec<String>,
    /// Rationale explaining the overall assessment
    #[serde(default)]
    pub rationale: String,
    /// Whole-scenario prescriptiveness of the guidance the agent was given,
    /// assessed by the judge from the task prompt and agent guidance files.
    /// This rates the scenario's inputs, not the agent's performance, so it sits
    /// outside the weighted criteria. `None` when the judge did not assess it.
    #[serde(default)]
    pub prescriptiveness: Option<Prescriptiveness>,
    /// Difficulty-adjusted view of `weighted_score` that discounts credit as
    /// prescriptiveness rises. Computed by ax-eval (not the judge) via
    /// [`JudgeResponse::compute_adjusted_score`]; it never affects pass/fail or
    /// the composite score. `None` when prescriptiveness was not assessed.
    #[serde(default)]
    pub adjusted_score: Option<f64>,
}

impl JudgeResponse {
    /// Compute the difficulty-adjusted score from the assessed prescriptiveness
    /// level and a discount factor.
    ///
    /// `discount` is the maximum fraction of judge credit removed at the most
    /// prescriptive level (3): `weighted_score * (1 - (level / 3) * discount)`.
    /// A goal-only scenario (level 0) is unchanged. Returns `None` when
    /// prescriptiveness was not assessed.
    pub fn compute_adjusted_score(&self, discount: f64) -> Option<f64> {
        self.prescriptiveness.as_ref().map(|prescriptiveness| {
            let level = f64::from(prescriptiveness.level.min(Prescriptiveness::MAX_LEVEL));
            let factor = 1.0 - (level / f64::from(Prescriptiveness::MAX_LEVEL)) * discount;
            (self.weighted_score * factor).clamp(0.0, 1.0)
        })
    }
}

/// How prescriptive the scenario's guidance was, rated over the whole scenario
/// (task prompt plus any agent guidance files) on a 0–3 scale.
///
/// - `0` (goal-only): states an outcome; names no tools or steps.
/// - `1` (light hints): mentions relevant tools but not how or when to use them.
/// - `2` (partial recipe): spells out steps for part of the task.
/// - `3` (step-by-step): dictates the exact sequence of tool calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prescriptiveness {
    /// Prescriptiveness level on a 0–3 scale (see type docs).
    pub level: u8,
    /// One-sentence explanation for the assigned level.
    #[serde(default)]
    pub rationale: String,
}

impl Prescriptiveness {
    /// The most prescriptive level on the scale (step-by-step).
    pub const MAX_LEVEL: u8 = 3;
}
