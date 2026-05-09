use crate::cli::TemplateKind;
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

            let result = run::run_single_scenario(run::ScenarioRunRequest {
                scenario: &s,
                scenario_path: &record.path,
                tool: &config.tool,
                model: &config.model,
                dry_run: exec_config.dry_run,
                no_cache: exec_config.no_cache,
                timeout_secs: exec_config.timeout_secs,
                no_judge: exec_config.no_judge,
                judge_model: exec_config.judge_model.as_deref(),
                judge_tool: exec_config.judge_tool.as_deref(),
                results_db: ctx.results_db,
                cache: ctx.cache,
            });

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

pub fn handle_template_command(kind: TemplateKind) {
    let template = match kind {
        TemplateKind::Scenario => SCENARIO_TEMPLATE,
        TemplateKind::Config => CONFIG_TEMPLATE,
        TemplateKind::ScriptGate => SCRIPT_GATE_TEMPLATE,
        TemplateKind::Evaluator => EVALUATOR_TEMPLATE,
    };
    print!("{}", template);
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

const SCENARIO_TEMPLATE: &str = r#"name: example_cli_workflow
description: Verify an agent can use mytool to complete a realistic workflow
tags:
  - smoke
tier: 0
template_folder: example_cli_workflow

target:
  binary: mytool
  command_pattern: "mytool\\s+(\\S+)"
  health_check: "mytool --version"
  env:
    MYTOOL_STATE: ".mytool/state.json"

task:
  prompt: |
    Use `mytool` to create a project named "Example" and export a summary.
    Read AGENTS.md first, then inspect `mytool --help` only if needed.

setup:
  commands:
    - "chmod +x mytool scripts/*.sh"

scripts:
  post:
    - command: "./scripts/export_state.sh"
      timeout_secs: 30
  evaluators:
    - command: "./scripts/score_quality.sh"
      name: quality
      timeout_secs: 60

evaluation:
  gates:
    - type: file_exists
      path: summary.md
    - type: file_contains
      path: summary.md
      substring: "Example"
    - type: command_succeeds
      command: "mytool status"
    - type: command_json_path
      command: "cat .mytool/export.json"
      path: "$.items"
      assertion: "len >= 1"
    - type: script
      command: "./scripts/check_summary.sh"
      description: Summary includes required project fields
      timeout_secs: 30
  judge:
    enabled: false
    rubric: rubrics/workflow.yaml
    pass_threshold: 0.70

tool_matrix:
  - tool: opencode
    models:
      - default

run:
  timeout_secs: 300
"#;

const CONFIG_TEMPLATE: &str = r#"fixtures_path = "fixtures"
results_path = "llm-tool-test-results"

[tools.opencode]
name = "opencode"
command = "opencode"
models = ["default", "gpt-4o"]

[tools.claude-code]
name = "claude-code"
command = "claude"
models = ["default", "claude-sonnet"]

[tools.codex]
name = "codex"
command = "codex"
models = ["default", "gpt-5-codex"]

[profiles.quick]
name = "quick"
tools = ["opencode"]
models = ["default"]

# Profiles are tool x model Cartesian products. Every model must be valid for
# every selected tool. Use scenario `tool_matrix` for heterogeneous matrices.
[profiles.compare_opencode_models]
name = "compare_opencode_models"
tools = ["opencode"]
models = ["default", "gpt-4o"]
"#;

const SCRIPT_GATE_TEMPLATE: &str = r#"#!/usr/bin/env bash
set -euo pipefail

# Script gates run in the fixture directory.
# Exit code 0 passes; non-zero fails. Optional JSON stdout may include
# `passed` and `message`.

if test -f summary.md && grep -q "Example" summary.md; then
  printf '{"passed":true,"message":"summary.md contains Example"}\n'
  exit 0
fi

printf '{"passed":false,"message":"summary.md is missing Example"}\n'
exit 1
"#;

const EVALUATOR_TEMPLATE: &str = r#"#!/usr/bin/env bash
set -euo pipefail

# Evaluators run in the fixture directory after gates and judge evaluation.
# Exit code 0 means the evaluator ran successfully; it does not pass/fail the
# scenario. Print JSON with optional metrics, score, and summary fields.

note_count=0
if test -f .mytool/export.json; then
  note_count=$(jq '.items | length' .mytool/export.json)
fi

cat <<JSON
{
  "metrics": {
    "item_count": ${note_count}
  },
  "score": 0.80,
  "summary": "Export contains ${note_count} item(s)"
}
JSON
"#;
