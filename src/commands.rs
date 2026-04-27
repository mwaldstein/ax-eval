use crate::config::Config;
use crate::evaluation::ScoreTier;
use crate::output;
use crate::results::{Cache, ResultsDB};
use crate::run;
use crate::scenario::load;
use crate::utils::resolve_fixtures_path;
use chrono::{Duration, Utc};
use std::path::{Path, PathBuf};

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
    pub no_judge: bool,
}

pub struct ExecutionContext<'a> {
    pub base_dir: &'a Path,
    pub results_db: &'a ResultsDB,
    pub cache: &'a Cache,
}

#[allow(clippy::too_many_arguments)]
fn resolve_scenario_path(path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || p.exists() {
        p.to_path_buf()
    } else {
        let fixtures_dir = resolve_fixtures_path("");
        let fixture_path = fixtures_dir.join(path);
        if fixture_path.exists() {
            fixture_path
        } else {
            fixtures_dir.join(format!("{}.yaml", path))
        }
    }
}

fn find_scenarios(dir: &Path, scenarios: &mut Vec<(String, PathBuf)>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yaml" {
                        if let Ok(s) = load(&path) {
                            scenarios.push((s.name, path));
                        }
                    }
                }
            } else if path.is_dir() {
                find_scenarios(&path, scenarios);
            }
        }
    }
}

pub fn handle_run_command(
    selection: &ScenarioSelection,
    exec_config: &ExecutionConfig,
    ctx: &ExecutionContext,
    config: &Config,
) -> anyhow::Result<()> {
    let scenarios_to_run = if selection.all {
        let mut scenarios = Vec::new();
        let fixtures_dir = resolve_fixtures_path("");
        if fixtures_dir.exists() {
            find_scenarios(&fixtures_dir, &mut scenarios);
        }

        let mut filtered_scenarios = Vec::new();
        for (name, path) in scenarios {
            let s = load(&path)?;

            let tags_match = if selection.tags.is_empty() {
                true
            } else {
                selection.tags.iter().all(|tag| s.tags.contains(tag))
            };

            let tier_match = s.tier <= selection.tier;

            if tags_match && tier_match {
                filtered_scenarios.push((name, path));
            }
        }
        filtered_scenarios
    } else if let Some(path) = &selection.scenario {
        let resolved_path = resolve_scenario_path(path);
        let s = load(&resolved_path)?;
        vec![(s.name, resolved_path)]
    } else {
        println!("No scenario specified. Use --scenario <path> or --all");
        return Ok(());
    };

    for (name, path) in scenarios_to_run {
        let s = load(&path)?;
        println!("Loaded scenario: {}", name);

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
                &path,
                &config.tool,
                &config.model,
                exec_config.dry_run,
                exec_config.no_cache,
                exec_config.timeout_secs,
                exec_config.no_judge,
                exec_config.judge_model.as_deref(),
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
    let mut scenarios = Vec::new();

    fn find_scenarios(
        dir: &std::path::Path,
        scenarios: &mut Vec<(PathBuf, String, usize, String, Vec<String>)>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yaml" {
                            if let Ok(s) = load(&path) {
                                scenarios.push((path, s.name, s.tier, s.description, s.tags));
                            }
                        }
                    }
                } else if path.is_dir() {
                    find_scenarios(&path, scenarios);
                }
            }
        }
    }

    let fixtures_dir = resolve_fixtures_path("");
    if fixtures_dir.exists() {
        find_scenarios(&fixtures_dir, &mut scenarios);
    }

    let filtered_scenarios: Vec<_> = scenarios
        .iter()
        .filter(|(_, _, scenario_tier, _, scenario_tags)| {
            let tier_match = scenario_tier <= tier;
            let tags_match = if tags.is_empty() {
                true
            } else {
                tags.iter().all(|tag| scenario_tags.contains(tag))
            };
            tier_match && tags_match
        })
        .collect();

    let tier_label = match *tier {
        0 => "smoke",
        1 => "quick",
        2 => "standard",
        3 => "comprehensive",
        _ => "unknown",
    };
    println!("Available scenarios (tier {}):", tier_label);
    for (_path, name, _scenario_tier, description, scenario_tags) in &filtered_scenarios {
        let tags_str = if scenario_tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", scenario_tags.join(", "))
        };
        println!("  [{}] {}{} - {}", tier_label, name, tags_str, description);
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
