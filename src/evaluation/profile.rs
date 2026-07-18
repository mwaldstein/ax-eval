use crate::judge::JudgeResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    NotConfigured,
    Passed,
    Failed,
}

impl GateStatus {
    pub fn from_details(gate_count: usize, details: &[GateResult]) -> Self {
        if gate_count == 0 {
            Self::NotConfigured
        } else if details.iter().any(|result| !result.passed) {
            Self::Failed
        } else {
            Self::Passed
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluationMetrics {
    pub gate_status: GateStatus,
    pub details: Vec<GateResult>,
    pub judge_score: Option<f64>,
    pub judge_response: Option<JudgeResponse>,
    pub judge_passed: Option<bool>,
    pub judge_threshold: Option<f64>,
    pub efficiency: crate::transcript::EfficiencyMetrics,
    pub interaction_evidence_source: crate::interaction_profile::InteractionEvidenceSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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
    #[serde(default)]
    pub identifier: String,
    pub passed: bool,
    pub message: String,
}

pub fn failed_gate_identifiers(details: &[GateResult]) -> Vec<String> {
    details
        .iter()
        .filter(|detail| !detail.passed)
        .map(|detail| {
            if detail.identifier.is_empty() {
                detail.gate_type.clone()
            } else {
                detail.identifier.clone()
            }
        })
        .collect()
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
