//! Type definitions for test results.
//!
//! This module defines all the data structures used to represent
//! test results, metrics, and cache keys.

pub mod cache_key;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use cache_key::CacheKey;

/// A complete test run result record.
///
/// Contains all metadata and metrics for a single scenario execution,
/// including timing, cost, gate results, and quality metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultRecord {
    /// Unique run identifier
    pub id: String,
    /// Scenario identifier (filename without extension)
    pub scenario_id: String,
    /// Hash of the scenario YAML content
    pub scenario_hash: String,
    /// Tool name (e.g., "opencode", "claude-code")
    pub tool: String,
    /// Model name used for this run
    pub model: String,
    /// Timestamp when the run completed
    pub timestamp: DateTime<Utc>,
    /// Total duration in seconds
    pub duration_secs: f64,
    /// Estimated cost in USD (if tool reports it)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Token usage (input/output counts, if tool reports it)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsageRecord>,
    /// Whether all gates passed
    pub gates_passed: bool,
    /// Detailed evaluation metrics
    pub metrics: EvaluationMetricsRecord,
    /// Optional LLM-as-judge score (0.0-1.0)
    pub judge_score: Option<f64>,
    /// Final outcome ("PASS", "FAIL", "ERROR")
    pub outcome: String,
    /// Path to the saved transcript file
    pub transcript_path: String,
    /// Optional cache key for this result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
}

/// Evaluation metrics for a test run.
///
/// Aggregates gate results, efficiency metrics,
/// and a composite quality score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetricsRecord {
    /// Number of gates that passed
    pub gates_passed: usize,
    /// Total number of gates evaluated
    pub gates_total: usize,
    /// Detailed results for each gate
    pub details: Vec<GateResultRecord>,
    /// Whether the judge threshold was met (None if judge was not run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge_passed: Option<bool>,
    /// Efficiency metrics
    pub efficiency: EfficiencyMetricsRecord,
    /// Composite quality score (0.0-1.0), only present if scenario configures composite weights
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composite_score: Option<f64>,
    /// Results from custom evaluators
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evaluator_results: Vec<EvaluatorResultRecord>,
}

/// Record of a custom evaluator result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResultRecord {
    /// Name of the evaluator
    pub name: String,
    /// Optional metrics as JSON value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
    /// Optional score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Human-readable summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Error message if evaluator failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Efficiency metrics measuring tool interaction patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyMetricsRecord {
    /// Total number of commands executed
    pub total_commands: usize,
    /// Number of unique commands executed
    pub unique_commands: usize,
    /// Number of commands that resulted in errors
    pub error_count: usize,
    /// Number of command retries
    pub retry_count: usize,
    /// Number of help invocations
    pub help_invocations: usize,
    /// Rate of commands succeeding on first attempt (0.0-1.0)
    pub first_try_success_rate: f64,
    /// Ratio of total commands to unique commands
    pub iteration_ratio: f64,
    /// Whether the agent completed the task (exited normally with exit code 0)
    pub completed: bool,
}

/// Result of evaluating a single gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResultRecord {
    /// Type of gate evaluated
    pub gate_type: String,
    /// Whether the gate passed
    pub passed: bool,
    /// Human-readable message about the result
    pub message: String,
}

/// Token usage record for a test run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenUsageRecord {
    /// Number of input tokens
    pub input: usize,
    /// Number of output tokens
    pub output: usize,
}
