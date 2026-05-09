use crate::config::Config;
use crate::evaluation::ScoreTier;
use crate::output;
use crate::results::{Cache, ResultsDB};
use crate::run;
use crate::scenario::catalog::ScenarioCatalog;
use chrono::{Duration, Utc};
use std::path::Path;

pub struct ScenarioSelection {
    pub scenario: Option<String>,
    pub all: bool,
    pub tags: Vec<String>,
    pub tier: usize,
}

pub struct ExecutionConfig {
    pub tool: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
    pub dry_run: bool,
    pub no_cache: bool,
    pub timeout_secs: u64,
    pub judge_model: Option<String>,
    pub judge_tool: Option<String>,
    pub no_judge: bool,
}

pub struct ExecutionContext<'a> {
    pub base_dir: &'a Path,
    pub results_db: &'a ResultsDB,
    pub cache: &'a Cache,
}

pub fn handle_run_command(
    selection: &ScenarioSelection,
    exec_config: &ExecutionConfig,
    ctx: &ExecutionContext,
    config: &Config,
) -> anyhow::Result<()> {
    let catalog = ScenarioCatalog::from_default_fixtures();
    let scenarios_to_run = if selection.all {
        catalog.select_all(&selection.tags, selection.tier)?
    } else if let Some(path) = &selection.scenario {
        vec![catalog.load_one(path)?]
    } else {
        println!("No scenario specified. Use --scenario <path> or --all");
        return Ok(());
    };

    for record in scenarios_to_run {
        let s = record.scenario;
        println!("Loaded scenario: {}", s.name);

        let matrix = crate::build_tool_matrix(
            &exec_config.tool,
            &exec_config.model,
            &exec_config.profile,
            config,
            &s.tool_matrix,
        )?;

        if matrix.len() > 1 {
            println!("Matrix run: {} tool×model combinations", matrix.len());
        }

        let mut results = Vec::new();

        for config in &matrix {
            println!("\n=== Running: {} / {} ===", config.tool, config.model);

            let result = run::run_single_scenario(
                &s,
                &record.path,
                &config.tool,
                &config.model,
                exec_config.dry_run,
                exec_config.no_cache,
                exec_config.timeout_secs,
                exec_config.no_judge,
                exec_config.judge_model.as_deref(),
                exec_config.judge_tool.as_deref(),
                ctx.base_dir,
                ctx.results_db,
                ctx.cache,
            );

            results.push((config.clone(), result));
        }

        if matrix.len() > 1 {
            output::print_matrix_summary(&results);
        }
    }

    Ok(())
}

pub fn handle_list_command(
    tags: &[String],
    tier: &usize,
    _results_db: &ResultsDB,
) -> anyhow::Result<()> {
    let catalog = ScenarioCatalog::from_default_fixtures();
    let summaries = catalog.summaries(tags, *tier)?;

    let tier_label = match *tier {
        0 => "smoke",
        1 => "quick",
        2 => "standard",
        3 => "comprehensive",
        _ => "unknown",
    };
    println!("Available scenarios (tier {}):", tier_label);
    for summary in &summaries {
        let tags_str = if summary.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", summary.tags.join(", "))
        };
        println!(
            "  [{}] {}{} - {}",
            tier_label, summary.name, tags_str, summary.description
        );
    }

    Ok(())
}

pub fn handle_show_command(name: &str, results_db: &ResultsDB) -> anyhow::Result<()> {
    let record = results_db.load_by_id(name)?;
    match record {
        Some(r) => {
            println!("Run ID: {}", r.id);
            println!("Scenario: {}", r.scenario_id);
            println!("Tool: {}", r.tool);
            println!("Timestamp: {}", r.timestamp);
            println!("Duration: {:.2}s", r.duration_secs);
            if let Some(cost) = r.cost_usd {
                println!("Cost: ${:.4}", cost);
            }
            println!("Outcome: {}", r.outcome);
            println!(
                "Gates: {}/{}",
                r.metrics.gates_passed, r.metrics.gates_total
            );
            if let Some(score) = r.judge_score {
                let tier = ScoreTier::from_score(score);
                println!("Judge Score: {:.2} ({})", score, tier);
            }
            if let Some(composite_score) = r.metrics.composite_score {
                let composite_tier = ScoreTier::from_score(composite_score);
                println!(
                    "Composite Score: {:.2} ({})",
                    composite_score, composite_tier
                );
            }
            println!("Transcript: {}", r.transcript_path);
        }
        None => println!("Run not found: {}", name),
    }

    Ok(())
}

pub fn handle_clean_command(
    cache: &Cache,
    older_than: &Option<String>,
    base_dir: &Path,
) -> anyhow::Result<()> {
    let cutoff_time = if let Some(duration_str) = older_than {
        let duration = parse_duration(duration_str)?;
        Some(Utc::now() - duration)
    } else {
        None
    };

    println!("Cleaning cache...");
    let (cache_removed_count, cache_kept_count) = cache.clear_older_than(cutoff_time)?;
    if let Some(duration_str) = older_than {
        println!(
            "Cleaned {} cache file(s) older than {}, kept {}",
            cache_removed_count, duration_str, cache_kept_count
        );
    } else {
        println!("Cleaned {} cache file(s)", cache_removed_count);
    }

    // Clean old transcripts
    let transcripts_dir = base_dir.join("transcripts");
    if !transcripts_dir.exists() {
        println!("No transcripts directory found");
        return Ok(());
    }

    let mut removed_count = 0;
    let mut kept_count = 0;

    for entry in std::fs::read_dir(&transcripts_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Check if we should delete based on age
        let should_delete = if let Some(cutoff) = cutoff_time {
            // Get the modification time of the directory
            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    let modified_datetime = chrono::DateTime::<Utc>::from(modified);
                    modified_datetime < cutoff
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            // If no cutoff time specified, delete all
            true
        };

        if should_delete {
            if let Err(e) = std::fs::remove_dir_all(&path) {
                eprintln!("Warning: Failed to remove {}: {}", path.display(), e);
            } else {
                removed_count += 1;
            }
        } else {
            kept_count += 1;
        }
    }

    if let Some(duration_str) = older_than {
        println!(
            "Cleaned {} transcript(s) older than {}, kept {}",
            removed_count, duration_str, kept_count
        );
    } else {
        println!("Cleaned {} transcript(s)", removed_count);
    }

    Ok(())
}

fn parse_duration(s: &str) -> anyhow::Result<Duration> {
    let re = regex::Regex::new(r"^(\d+)([dhm])$")?;
    let caps = re.captures(s).ok_or_else(|| {
        anyhow::anyhow!("Invalid duration format. Use format like '30d', '7d', '1h'")
    })?;

    let value: i64 = caps[1].parse()?;
    let unit = &caps[2];

    let duration = match unit {
        "d" => Duration::days(value),
        "h" => Duration::hours(value),
        "m" => Duration::minutes(value),
        _ => anyhow::bail!("Invalid duration unit. Use 'd' (days), 'h' (hours), or 'm' (minutes)"),
    };

    Ok(duration)
}
