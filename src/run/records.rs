use crate::adapter::TokenUsage as AdapterTokenUsage;
use crate::evaluation::EvaluationMetrics;
use crate::output;
use crate::results::{
    Cache, CacheKey, EvaluationMetricsRecord, ResultRecord, ResultsDB, TokenUsageRecord,
};
use crate::scenario::Scenario;
use std::path::Path;

pub struct ResultRecordInput<'a> {
    pub scenario: &'a Scenario,
    pub tool: &'a str,
    pub model: &'a str,
    pub cache_key: &'a CacheKey,
    pub metrics: EvaluationMetrics,
    pub outcome: String,
    pub duration_secs: f64,
    pub cost: Option<f64>,
    pub token_usage: Option<AdapterTokenUsage>,
    pub transcript_path: String,
}

impl ResultRecordInput<'_> {
    pub fn build(self) -> ResultRecord {
        use crate::results::{EfficiencyMetricsRecord, EvaluatorResultRecord, GateResultRecord};

        let metrics = self.metrics;

        ResultRecord {
            id: crate::results::generate_run_id(),
            scenario_id: self.scenario.name.clone(),
            scenario_hash: self.cache_key.scenario_hash.clone(),
            tool: self.tool.to_string(),
            model: self.model.to_string(),
            timestamp: chrono::Utc::now(),
            duration_secs: self.duration_secs,
            cost_usd: self.cost,
            token_usage: self.token_usage.map(|tu| TokenUsageRecord {
                input: tu.input,
                output: tu.output,
            }),
            gates_passed: metrics.gates_passed >= metrics.gates_total
                && metrics.judge_passed.unwrap_or(true),
            metrics: EvaluationMetricsRecord {
                gates_passed: metrics.gates_passed,
                gates_total: metrics.gates_total,
                details: metrics
                    .details
                    .into_iter()
                    .map(|d| GateResultRecord {
                        gate_type: d.gate_type,
                        passed: d.passed,
                        message: d.message,
                    })
                    .collect(),
                judge_passed: metrics.judge_passed,
                efficiency: EfficiencyMetricsRecord {
                    total_commands: metrics.efficiency.total_commands,
                    unique_commands: metrics.efficiency.unique_commands,
                    error_count: metrics.efficiency.error_count,
                    retry_count: metrics.efficiency.retry_count,
                    help_invocations: metrics.efficiency.help_invocations,
                    first_try_success_rate: metrics.efficiency.first_try_success_rate,
                    iteration_ratio: metrics.efficiency.iteration_ratio,
                    completed: metrics.efficiency.completed,
                },
                interaction_evidence_source: Some(metrics.interaction_evidence_source),
                composite_score: metrics.composite_score,
                evaluator_results: metrics
                    .evaluator_results
                    .into_iter()
                    .map(|e| EvaluatorResultRecord {
                        name: e.name,
                        metrics: e.metrics,
                        score: e.score,
                        summary: e.summary,
                        error: e.error,
                    })
                    .collect(),
            },
            judge_score: metrics.judge_score,
            outcome: self.outcome,
            transcript_path: self.transcript_path,
            cache_key: Some(self.cache_key.as_string()),
        }
    }
}

pub fn handle_dry_run(
    s: &Scenario,
    tool: &str,
    model: &str,
    cache_key: &CacheKey,
) -> anyhow::Result<ResultRecord> {
    use crate::results::{EfficiencyMetricsRecord, EvaluationMetricsRecord};

    println!("Dry run - skipping execution");

    let record = ResultRecord {
        id: crate::results::generate_run_id(),
        scenario_id: s.name.clone(),
        scenario_hash: cache_key.scenario_hash.clone(),
        tool: tool.to_string(),
        model: model.to_string(),
        timestamp: chrono::Utc::now(),
        duration_secs: 0.0,
        cost_usd: None,
        token_usage: None,
        gates_passed: true,
        metrics: EvaluationMetricsRecord {
            gates_passed: 0,
            gates_total: 0,
            details: vec![],
            judge_passed: None,
            efficiency: EfficiencyMetricsRecord {
                total_commands: 0,
                unique_commands: 0,
                error_count: 0,
                retry_count: 0,
                help_invocations: 0,
                first_try_success_rate: 0.0,
                iteration_ratio: 0.0,
                completed: false,
            },
            interaction_evidence_source: None,
            composite_score: None,
            evaluator_results: vec![],
        },
        judge_score: None,
        outcome: "Dry run".to_string(),
        transcript_path: String::new(),
        cache_key: Some(cache_key.as_string()),
    };

    output::print_result_summary(&record);
    Ok(record)
}

pub fn finalize_execution(
    results_db: &ResultsDB,
    cache: &Cache,
    cache_key: &CacheKey,
    record: &ResultRecord,
    results_dir: &Path,
    setup_success: bool,
) -> anyhow::Result<ResultRecord> {
    results_db.append(record)?;
    cache.put(cache_key, record)?;

    let metrics_json = serde_json::to_string_pretty(&record.metrics)?;
    std::fs::write(results_dir.join("metrics.json"), metrics_json)?;

    println!("\nRun completed: {}", record.id);
    println!("Artifacts written to: {}", results_dir.display());

    if !setup_success {
        println!("\nWarning: Setup commands failed. Results may be invalid.");
    }

    output::print_result_summary(record);
    Ok(record.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::EvaluationMetrics;
    use crate::interaction_profile::InteractionEvidenceSource;
    use crate::scenario::{Evaluation, Scenario, TargetConfig, Task};
    use crate::transcript::EfficiencyMetrics;

    fn scenario() -> Scenario {
        Scenario {
            name: "record-input-test".to_string(),
            description: "Test scenario".to_string(),
            template_folder: "fixture".to_string(),
            target: TargetConfig {
                binary: "tool".to_string(),
                command_pattern: None,
                health_check: None,
                env: None,
            },
            task: Task {
                prompt: "Test prompt".to_string(),
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

    fn metrics() -> EvaluationMetrics {
        EvaluationMetrics {
            gates_passed: 1,
            gates_total: 1,
            details: vec![],
            judge_score: Some(0.9),
            judge_response: None,
            judge_passed: Some(true),
            efficiency: EfficiencyMetrics {
                total_commands: 1,
                unique_commands: 1,
                error_count: 0,
                retry_count: 0,
                help_invocations: 0,
                first_try_success_rate: 1.0,
                iteration_ratio: 1.0,
                completed: true,
            },
            interaction_evidence_source: InteractionEvidenceSource::StructuredToolCalls,
            composite_score: Some(0.95),
            evaluator_results: vec![],
        }
    }

    #[test]
    fn record_input_builds_result_record() {
        let scenario = scenario();
        let cache_key = CacheKey::compute("scenario", "prompt", "mock", "model");

        let record = ResultRecordInput {
            scenario: &scenario,
            tool: "mock",
            model: "model",
            cache_key: &cache_key,
            metrics: metrics(),
            outcome: "Pass".to_string(),
            duration_secs: 1.25,
            cost: None,
            token_usage: None,
            transcript_path: "artifacts".to_string(),
        }
        .build();

        assert_eq!(record.scenario_id, "record-input-test");
        assert_eq!(record.tool, "mock");
        assert_eq!(record.model, "model");
        assert_eq!(record.duration_secs, 1.25);
        assert_eq!(record.outcome, "Pass");
        assert_eq!(record.transcript_path, "artifacts");
        assert_eq!(record.cache_key, Some(cache_key.as_string()));
        assert!(record.gates_passed);
    }
}
