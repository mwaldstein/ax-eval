use crate::evaluation::ScoreTier;
use crate::results::ResultRecord;

#[derive(Debug, Clone)]
pub struct ToolModelConfig {
    pub tool: String,
    pub model: String,
}

pub fn print_matrix_summary(results: &[(ToolModelConfig, anyhow::Result<ResultRecord>)]) {
    println!("\n--- Matrix Summary ---");

    let mut table: std::collections::HashMap<String, std::collections::HashMap<String, String>> =
        std::collections::HashMap::new();

    for (config, result) in results {
        let outcome = match result {
            Ok(record) => record.outcome.clone(),
            Err(e) => format!("Error: {}", e),
        };

        table
            .entry(config.tool.clone())
            .or_default()
            .insert(config.model.clone(), outcome);
    }

    println!("{:<20} |", "Tool");
    let all_models: std::collections::BTreeSet<_> = table
        .values()
        .flat_map(|models| models.keys().cloned())
        .collect();

    for model in &all_models {
        print!(" {:<20} |", model);
    }
    println!();

    println!("{}", "-".repeat(22));
    for _ in &all_models {
        println!("{}", "-".repeat(22));
    }
    println!();

    let mut tools: Vec<_> = table.keys().collect();
    tools.sort();

    for tool in tools {
        print!("{:<20} |", tool);
        for model in &all_models {
            let default = "-";
            let outcome = table
                .get(tool)
                .and_then(|m| m.get(model))
                .map_or(default, |v| v.as_str());
            print!(" {:<20} |", outcome);
        }
        println!();
    }
}

pub fn print_result_summary(record: &ResultRecord) {
    println!("\n--- Result Summary ---");
    println!("ID: {}", record.id);
    println!("Scenario: {}", record.scenario_id);
    println!("Tool: {}", record.tool);
    println!("Outcome: {}", record.outcome);
    println!(
        "Gates: {}/{}",
        record.metrics.gates_passed, record.metrics.gates_total
    );
    println!("Duration: {:.2}s", record.duration_secs);
    println!(
        "Commands: {} ({} unique, {} errors, {} help, {} retries)",
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
    if let Some(score) = record.judge_score {
        let tier = ScoreTier::from_score(score);
        let passed_str = match record.metrics.judge_passed {
            Some(true) => "PASS",
            Some(false) => "FAIL",
            None => "N/A",
        };
        println!("Judge Score: {:.2} ({}) [{}]", score, tier, passed_str);
    }
    if let Some(composite_score) = record.metrics.composite_score {
        let composite_tier = ScoreTier::from_score(composite_score);
        println!(
            "Composite Score: {:.2} ({})",
            composite_score, composite_tier
        );
    }
}
