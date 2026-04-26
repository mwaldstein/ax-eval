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
}
