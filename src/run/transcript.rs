use crate::evaluation::EvaluationMetrics;
use crate::fixture::TestEnv;
use crate::results::CacheKey;
use crate::scenario::Scenario;
use crate::transcript::{RunMetadata, TranscriptWriter};

#[allow(clippy::too_many_arguments)]
pub fn write_transcript_files(
    writer: &TranscriptWriter,
    s: &Scenario,
    tool: &str,
    model: &str,
    cache_key: &CacheKey,
    _output: &str,
    _exit_code: i32,
    cost: Option<f64>,
    token_usage: Option<crate::adapter::TokenUsage>,
    duration: std::time::Duration,
    metrics: &EvaluationMetrics,
    outcome: &str,
    setup_success: bool,
    setup_commands: Vec<(String, bool, String)>,
    _env: &TestEnv,
) -> anyhow::Result<()> {
    // Note: transcript.raw.txt and execution event are already written in run_evaluation_flow

    let run_metadata = RunMetadata {
        scenario_id: s.name.clone(),
        scenario_hash: cache_key.scenario_hash.clone(),
        tool: tool.to_string(),
        model: model.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        duration_secs: duration.as_secs_f64(),
        cost_estimate_usd: cost,
        token_usage: token_usage.clone().map(|t| crate::transcript::TokenUsage {
            input: t.input,
            output: t.output,
        }),
    };
    writer.write_run_metadata(&run_metadata)?;

    let report = crate::transcript::RunReport {
        scenario_id: s.name.clone(),
        tool: tool.to_string(),
        model: model.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        duration_secs: duration.as_secs_f64(),
        cost_usd: cost,
        token_usage: token_usage.map(|t| crate::transcript::TokenUsage {
            input: t.input,
            output: t.output,
        }),
        outcome: outcome.to_string(),
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
        setup_success,
        setup_commands: setup_commands
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
    writer.write_report(&report)?;

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
        scenario_id: s.name.clone(),
        tool: tool.to_string(),
        model: model.to_string(),
        outcome: outcome.to_string(),
        judge_score_1_to_5,
        gates_passed: metrics.gates_passed,
        gates_total: metrics.gates_total,
        duration_secs: duration.as_secs_f64(),
        cost_usd: cost,
        composite_score: metrics.composite_score,
        judge_feedback,
        evaluator_results,
    };
    writer.write_evaluation(&evaluation)?;

    Ok(())
}
