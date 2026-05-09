use crate::evaluation::EvaluationMetrics;
use crate::results::CacheKey;
use crate::run::execution::EvaluationFlowResult;
use crate::scenario::Scenario;
use crate::transcript::{RunMetadata, TranscriptWriter};

pub struct TranscriptFilesInput<'a> {
    pub writer: &'a TranscriptWriter,
    pub scenario: &'a Scenario,
    pub tool: &'a str,
    pub model: &'a str,
    pub cache_key: &'a CacheKey,
    pub evaluation: &'a EvaluationFlowResult,
    pub outcome: &'a str,
    pub setup_success: bool,
    pub setup_commands: Vec<(String, bool, String)>,
}

pub fn write_transcript_files(input: TranscriptFilesInput<'_>) -> anyhow::Result<()> {
    // Note: transcript.raw.txt and execution event are already written in run_evaluation_flow
    let metrics: &EvaluationMetrics = &input.evaluation.metrics;

    let run_metadata =
        RunMetadata {
            scenario_id: input.scenario.name.clone(),
            scenario_hash: input.cache_key.scenario_hash.clone(),
            tool: input.tool.to_string(),
            model: input.model.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_secs: input.evaluation.duration.as_secs_f64(),
            cost_estimate_usd: input.evaluation.cost,
            token_usage: input.evaluation.token_usage.clone().map(|t| {
                crate::transcript::TokenUsage {
                    input: t.input,
                    output: t.output,
                }
            }),
        };
    input.writer.write_run_metadata(&run_metadata)?;

    let report =
        crate::transcript::RunReport {
            scenario_id: input.scenario.name.clone(),
            tool: input.tool.to_string(),
            model: input.model.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_secs: input.evaluation.duration.as_secs_f64(),
            cost_usd: input.evaluation.cost,
            token_usage: input.evaluation.token_usage.clone().map(|t| {
                crate::transcript::TokenUsage {
                    input: t.input,
                    output: t.output,
                }
            }),
            outcome: input.outcome.to_string(),
            gates_passed: metrics.gates_passed,
            gates_total: metrics.gates_total,
            composite_score: metrics.composite_score,
            gate_details: metrics
                .details
                .iter()
                .map(|d| crate::transcript::types::GateDetail {
                    gate_type: d.gate_type.clone(),
                    passed: d.passed,
                    message: d.message.clone(),
                })
                .collect(),
            efficiency: crate::transcript::types::EfficiencyReport {
                total_commands: metrics.efficiency.total_commands,
                unique_commands: metrics.efficiency.unique_commands,
                error_count: metrics.efficiency.error_count,
                first_try_success_rate: metrics.efficiency.first_try_success_rate,
                iteration_ratio: metrics.efficiency.iteration_ratio,
            },
            setup_success: input.setup_success,
            setup_commands: input
                .setup_commands
                .into_iter()
                .map(
                    |(cmd, success, output)| crate::transcript::types::SetupCommandResult {
                        command: cmd,
                        success,
                        output,
                    },
                )
                .collect(),
        };
    input.writer.write_report(&report)?;

    let judge_score_1_to_5 = metrics.judge_score.map(|score| (score * 5.0).round());
    let judge_feedback = if let Some(ref response) = metrics.judge_response {
        let mut feedback = Vec::new();
        if !response.rationale.is_empty() {
            feedback.push(format!("**Rationale:**\n{}", response.rationale));
        }
        if !response.issues.is_empty() {
            feedback.push(format!("**Issues:**\n{}", response.issues.join("\n")));
        }
        if !response.highlights.is_empty() {
            feedback.push(format!(
                "**Highlights:**\n{}",
                response.highlights.join("\n")
            ));
        }
        if !response.scores.is_empty() {
            let scores_text: Vec<String> = response
                .scores
                .iter()
                .map(|(k, v)| format!("- {}: {:.2}", k, v))
                .collect();
            feedback.push(format!("**Criteria Scores:**\n{}", scores_text.join("\n")));
        }
        feedback
    } else {
        Vec::new()
    };

    let evaluator_results = metrics
        .evaluator_results
        .iter()
        .map(|e| crate::transcript::types::EvaluatorResultSummary {
            name: e.name.clone(),
            score: e.score,
            summary: e.summary.clone(),
            error: e.error.clone(),
        })
        .collect();

    let evaluation = crate::transcript::EvaluationReport {
        scenario_id: input.scenario.name.clone(),
        tool: input.tool.to_string(),
        model: input.model.to_string(),
        outcome: input.outcome.to_string(),
        judge_score_1_to_5,
        gates_passed: metrics.gates_passed,
        gates_total: metrics.gates_total,
        duration_secs: input.evaluation.duration.as_secs_f64(),
        cost_usd: input.evaluation.cost,
        composite_score: metrics.composite_score,
        judge_feedback,
        evaluator_results,
    };
    input.writer.write_evaluation(&evaluation)?;

    Ok(())
}
