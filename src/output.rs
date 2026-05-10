use crate::evaluation::ScoreTier;
use crate::results::{EvaluatorResultRecord, ResultRecord};

#[derive(Debug, Clone)]
pub struct ToolModelConfig {
    pub tool: String,
    pub model: String,
}

fn format_score(score: Option<f64>) -> String {
    score
        .map(|score| format!("{:.2} {}", score, ScoreTier::from_score(score)))
        .unwrap_or_else(|| "-".to_string())
}

fn format_judge(record: &ResultRecord) -> String {
    match (record.judge_score, record.metrics.judge_threshold) {
        (Some(score), Some(threshold)) => {
            format!("{:.2} ({:.2}) {:+.2}", score, threshold, score - threshold)
        }
        (Some(score), None) => format!("{:.2} (target n/a)", score),
        (None, Some(threshold)) => format!("not run ({:.2})", threshold),
        (None, None) => "not run".to_string(),
    }
}

fn format_guardrails(record: &ResultRecord) -> String {
    format!(
        "{}/{} gates",
        record.metrics.gates_passed, record.metrics.gates_total
    )
}

fn format_evaluators(evaluators: &[EvaluatorResultRecord]) -> String {
    let summaries: Vec<String> = evaluators
        .iter()
        .map(|result| {
            if let Some(error) = &result.error {
                format!("{} error: {}", result.name, error)
            } else if let Some(score) = result.score {
                format!("{} {:.2}", result.name, score)
            } else if let Some(summary) = &result.summary {
                format!("{} {}", result.name, summary)
            } else {
                result.name.clone()
            }
        })
        .collect();

    if summaries.is_empty() {
        "-".to_string()
    } else {
        summaries.join("; ")
    }
}

fn format_run_status(record: &ResultRecord) -> String {
    if record.outcome.eq_ignore_ascii_case("dry run") {
        return "dry run; not executed".to_string();
    }
    if !record.metrics.efficiency.completed {
        return "agent did not complete".to_string();
    }
    if record.metrics.gates_passed < record.metrics.gates_total {
        return format!(
            "guardrail attention: {}/{} gates",
            record.metrics.gates_passed, record.metrics.gates_total
        );
    }
    if let Some(false) = record.metrics.judge_passed {
        return "judge threshold attention".to_string();
    }
    if record.metrics.judge_passed.is_none() {
        return "completed; judge not run".to_string();
    }
    "completed; judge threshold met".to_string()
}

pub fn print_matrix_summary(results: &[(ToolModelConfig, anyhow::Result<ResultRecord>)]) {
    println!("\n--- Evaluation Profile Summary ---");
    println!(
        "{:<14} | {:<16} | {:<18} | {:<18} | {:>8} | {:>6} | {:>7} | {:<12} | Status",
        "Tool", "Model", "Judge", "Composite", "FirstTry", "Errors", "Retries", "Guardrails"
    );
    println!("{}", "-".repeat(128));

    for (config, result) in results {
        match result {
            Ok(record) => {
                println!(
                    "{:<14} | {:<16} | {:<18} | {:<18} | {:>7.0}% | {:>6} | {:>7} | {:<12} | {}",
                    config.tool,
                    config.model,
                    format_judge(record),
                    format_score(record.metrics.composite_score),
                    record.metrics.efficiency.first_try_success_rate * 100.0,
                    record.metrics.efficiency.error_count,
                    record.metrics.efficiency.retry_count,
                    format_guardrails(record),
                    format_run_status(record)
                );
            }
            Err(e) => {
                println!(
                    "{:<14} | {:<16} | {:<18} | {:<18} | {:>8} | {:>6} | {:>7} | {:<12} | run error: {}",
                    config.tool, config.model, "not run", "-", "-", "-", "-", "-", e
                );
            }
        };
    }
}

pub fn print_result_summary(record: &ResultRecord) {
    println!("\n--- Evaluation Profile ---");
    println!("ID: {}", record.id);
    println!("Scenario: {}", record.scenario_id);
    println!("Tool: {}", record.tool);
    println!("Model: {}", record.model);
    println!("Judge Score: {}", format_judge(record));
    if let Some(false) = record.metrics.judge_passed {
        println!("Judge threshold: not met");
    }
    if let Some(true) = record.metrics.judge_passed {
        println!("Judge threshold: met");
    }
    if let Some(composite_score) = record.metrics.composite_score {
        println!(
            "Composite Score: {:.2} ({})",
            composite_score,
            ScoreTier::from_score(composite_score)
        );
    } else {
        println!("Composite Score: not configured");
    }
    println!(
        "Evaluators: {}",
        format_evaluators(&record.metrics.evaluator_results)
    );
    println!(
        "Interaction: {} commands ({} unique, {} errors, {} help, {} retries)",
        record.metrics.efficiency.total_commands,
        record.metrics.efficiency.unique_commands,
        record.metrics.efficiency.error_count,
        record.metrics.efficiency.help_invocations,
        record.metrics.efficiency.retry_count
    );
    println!(
        "First-try success: {:.0}%, iteration ratio: {:.2}",
        record.metrics.efficiency.first_try_success_rate * 100.0,
        record.metrics.efficiency.iteration_ratio
    );
    if let Some(source) = record.metrics.interaction_evidence_source {
        println!("Interaction evidence: {:?}", source);
    }
    println!("Duration: {:.2}s", record.duration_secs);
    if let Some(cost) = record.cost_usd {
        println!("Cost: ${:.4}", cost);
    }
    println!("Guardrails: {}", format_guardrails(record));
    println!("Status: {}", format_run_status(record));
}
