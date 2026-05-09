use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RunMetadata {
    pub scenario_id: String,
    pub scenario_hash: String,
    pub tool: String,
    pub model: String,
    pub timestamp: String,
    pub duration_secs: f64,
    pub cost_estimate_usd: Option<f64>,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: usize,
    pub output: usize,
}

#[derive(Debug)]
pub struct RunReport {
    pub scenario_id: String,
    pub tool: String,
    pub model: String,
    pub timestamp: String,
    pub duration_secs: f64,
    pub cost_usd: Option<f64>,
    pub token_usage: Option<TokenUsage>,
    pub outcome: String,
    pub gates_passed: usize,
    pub gates_total: usize,
    pub composite_score: Option<f64>,
    pub gate_details: Vec<GateDetail>,
    pub efficiency: EfficiencyReport,
    pub setup_success: bool,
    pub setup_commands: Vec<SetupCommandResult>,
}

#[derive(Debug)]
pub(crate) struct GateDetail {
    pub gate_type: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCommandResult {
    pub command: String,
    pub success: bool,
    pub output: String,
}

#[derive(Debug)]
pub(crate) struct EfficiencyReport {
    pub total_commands: usize,
    pub unique_commands: usize,
    pub error_count: usize,
    pub first_try_success_rate: f64,
    pub iteration_ratio: f64,
}

#[derive(Debug)]
pub struct EvaluationReport {
    pub scenario_id: String,
    pub tool: String,
    pub model: String,
    pub outcome: String,
    pub judge_score_1_to_5: Option<f64>,
    pub gates_passed: usize,
    pub gates_total: usize,
    pub duration_secs: f64,
    pub cost_usd: Option<f64>,
    pub composite_score: Option<f64>,
    pub judge_feedback: Vec<String>,
    pub evaluator_results: Vec<EvaluatorResultSummary>,
}

#[derive(Debug)]
pub struct EvaluatorResultSummary {
    pub name: String,
    pub score: Option<f64>,
    pub summary: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EfficiencyMetrics {
    pub total_commands: usize,
    pub unique_commands: usize,
    pub error_count: usize,
    pub retry_count: usize,
    pub help_invocations: usize,
    pub first_try_success_rate: f64,
    pub iteration_ratio: f64,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEvent {
    pub command: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionInput {
    StructuredToolCalls(Vec<CommandEvent>),
    TranscriptRegex,
}
