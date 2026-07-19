use crate::cli::{GuidanceCommand, TemplateKind};
use crate::config::Config;
use crate::guidance;
use crate::output;
use crate::results::{Cache, ResultsDB};
use crate::run;
use crate::scenario::catalog::ScenarioCatalog;
use crate::scenario::validation;
use chrono::{Duration, Utc};
use std::io::IsTerminal;
use std::path::Path;
use tracing::debug;

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
    pub use_cache: bool,
    pub timeout_secs: u64,
    pub judge_model: Option<String>,
    pub judge_tool: Option<String>,
    pub no_judge: bool,
}

pub struct ExecutionContext<'a> {
    pub results_db: &'a ResultsDB,
    pub cache: &'a Cache,
    pub base_dir: &'a Path,
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

    let mut run_errors = 0;

    for record in scenarios_to_run {
        let s = record.scenario;
        debug!("loaded scenario: {} from {}", s.name, record.path.display());
        println!("Loaded scenario: {}", s.name);

        let matrix = crate::build_tool_matrix(
            &exec_config.tool,
            &exec_config.model,
            &exec_config.profile,
            config,
            &s.tool_matrix,
        )?;

        if matrix.len() > 1 {
            debug!("matrix run: {} tool×model combinations", matrix.len());
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
                use_cache: exec_config.use_cache,
                timeout_secs: exec_config.timeout_secs,
                no_judge: exec_config.no_judge,
                judge_model: exec_config.judge_model.as_deref(),
                judge_tool: exec_config.judge_tool.as_deref(),
                results_db: ctx.results_db,
                cache: ctx.cache,
                results_dir_override: None,
            });

            results.push((config.clone(), result));
        }

        if matrix.len() > 1 {
            output::print_matrix_summary(&results);
        } else if let Some((config, Err(error))) = results.first() {
            eprintln!(
                "Run failed for {} / {}: {:#}",
                config.tool, config.model, error
            );
        }

        run_errors += results.iter().filter(|(_, result)| result.is_err()).count();
    }

    if run_errors > 0 {
        anyhow::bail!("{run_errors} scenario run(s) failed");
    }

    Ok(())
}

pub struct DiscoverConfig {
    pub target: String,
    pub tool: String,
    pub model: Option<String>,
    pub discover_tool: Option<String>,
    pub discover_model: Option<String>,
    pub judge_model: Option<String>,
    pub judge_tool: Option<String>,
    pub timeout_secs: u64,
}

pub fn handle_discover_command(
    config: &DiscoverConfig,
    ctx: &ExecutionContext,
) -> anyhow::Result<()> {
    let run_model = config.model.as_deref().unwrap_or("default");
    let discover_tool = config.discover_tool.as_deref().unwrap_or(&config.tool);
    let discover_model = config.discover_model.as_deref().unwrap_or(run_model);

    let root_dir = crate::discover::run_discovery(crate::discover::DiscoverRequest {
        target: &config.target,
        run_tool: &config.tool,
        run_model,
        discover_tool,
        discover_model,
        judge_model: config.judge_model.as_deref(),
        judge_tool: config.judge_tool.as_deref(),
        timeout_secs: config.timeout_secs,
        results_base_dir: ctx.base_dir,
        results_db: ctx.results_db,
        cache: ctx.cache,
    })?;

    println!(
        "Discovery summary: {}",
        root_dir.join("discovery-summary.md").display()
    );
    println!(
        "Discovery manifest: {}",
        root_dir.join("discovery.json").display()
    );
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
            println!("Timestamp: {}", r.timestamp);
            output::print_result_summary(&r);
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

pub fn handle_guidance_command(command: &GuidanceCommand) -> anyhow::Result<()> {
    match command {
        GuidanceCommand::List => {
            print!("{}", guidance::render_topic_list());
        }
        GuidanceCommand::Show { topics } => {
            print!("{}", guidance::render_topics(topics)?);
        }
        GuidanceCommand::Topics(topics) => {
            print!("{}", guidance::render_topics(topics)?);
        }
    }

    Ok(())
}

pub fn handle_template_command(kind: TemplateKind) {
    let template = match kind {
        TemplateKind::Scenario => SCENARIO_TEMPLATE,
        TemplateKind::Config => CONFIG_TEMPLATE,
        TemplateKind::ScriptGate => SCRIPT_GATE_TEMPLATE,
        TemplateKind::Evaluator => EVALUATOR_TEMPLATE,
        TemplateKind::Rubric => RUBRIC_TEMPLATE,
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

# Additional parent-process variables exposed to the evaluated agent. Adapter
# launch/auth variables are included automatically. Do not list stdio MCP
# target-private variables here.
# agent_env: [CUSTOM_CA_BUNDLE]

target:
  # If mytool is built outside the fixture, make it discoverable before running:
  # PATH="$PWD/target/debug:$PATH" AX_EVAL_ENABLED=1 ax-eval run --scenario ...
  # Or set a complete PATH in target.env for scenario-specific lookup.
  kind: cli
  binary: mytool
  command_pattern: "mytool\\s+(\\S+)"
  health_check: "mytool --version"
  env:
    MYTOOL_ROOT_DIR: "${AX_EVAL_FIXTURE_DIR}"

    MYTOOL_EXPORT: "${AX_EVAL_RESULTS_DIR}/mytool-export.json"

# Alternative MCP target blocks (see docs/mcp-targets.md). `command_pattern` is
# CLI-only; MCP targets declare a server `name`, a `transport`, and the `tools`
# the agent is expected to use.
#
# stdio MCP server:
# target:
#   kind: mcp
#   name: todo
#   transport:
#     type: stdio
#     command: "${AX_EVAL_FIXTURE_DIR}/todo-mcp-server"
#     args: ["--root", "${AX_EVAL_FIXTURE_DIR}"]
#   tools: [add, list, complete, search]
#   env:
#     TODO_DB: "${AX_EVAL_FIXTURE_DIR}/todo.db"
#   health_check: "./scripts/probe_todo.sh --health"
#
# remote (Streamable HTTP) MCP server:
# target:
#   kind: mcp
#   name: search
#   transport:
#     type: http
#     url: "https://mcp.example.com/mcp"
#   auth:
#     type: bearer_env
#     env: MY_MCP_TOKEN
#   tools: [query, index]
#
# remote MCP server with API-key headers:
# target:
#   kind: mcp
#   name: search
#   transport:
#     type: http
#     url: "https://mcp.example.com/mcp"
#   auth:
#     type: headers
#     headers:
#       X-API-Key: "${env:MY_MCP_API_KEY}"
#   tools: [query, index]
#
# MCP authoring tips (see docs/scenarios.md, "Authoring MCP scenarios"):
#   - Prefer self-documenting tool names, descriptions, schemas, help output,
#     and errors. Fixture guidance is optional context, not a substitute for the
#     target's own usage surface.
#   - Declare only the tools the task needs; the list bounds what is measured.
#   - Prefer read-only or fixture-backed servers; ax-eval cannot undo writes a
#     remote server makes outside the fixture.
#   - Verify outcomes with a script gate + probe (server state is not files).

interaction:
  target_commands: required

task:
  prompt: |
    Use `mytool` to create a project named "Example" and export a summary.
    Discover the workflow from `mytool --help` and the tool's own output.

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
    # Optional: path to a rubric YAML. Omit to use the default criteria
    # (goal completion 0.50, tool usage correctness 0.30, efficiency 0.20).
    # Relative paths resolve: scenario YAML dir, then workspace (template_folder),
    # then fixtures_path.
    rubric: rubrics/example.yaml
    pass_threshold: 0.70

tool_matrix:
  - tool: opencode
    models:
      - default

run:
  timeout_secs: 300
"#;

const CONFIG_TEMPLATE: &str = r#"fixtures_path = "ax-eval-fixtures"
results_path = "ax-eval-results"

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

const RUBRIC_TEMPLATE: &str = r#"# Rubric for LLM-as-judge evaluation. Referenced from a scenario via:
#   evaluation:
#     judge:
#       enabled: true
#       rubric: rubrics/example.yaml
#       pass_threshold: 0.70
# A rubric is optional; omit it to use the default criteria below.
# Criterion weights must sum to 1.0 (within 0.01 tolerance).
criteria:
  - id: task_completion
    weight: 0.50
    description: "The agent achieved the user's requested goal and produced the intended outcome"
  - id: tool_usage_correctness
    weight: 0.30
    description: "The agent used the CLI tool correctly with valid commands and arguments"
  - id: efficiency
    weight: 0.20
    description: "The agent completed the task without unnecessary commands or avoidable confusion"

# Output contract the judge response must satisfy.
output:
  format: json
  require_fields: [scores, weighted_score, confidence, issues, highlights, rationale]
"#;

fn looks_like_scenario(path: &std::path::Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let value: yaml_serde::Value = match yaml_serde::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let Some(mapping) = value.as_mapping() else {
        return false;
    };
    let keys = ["name", "target", "task", "evaluation", "template_folder"];
    let scenario_key_count = mapping
        .keys()
        .filter(|k| k.as_str().is_some_and(|s| keys.contains(&s)))
        .count();
    scenario_key_count >= 2
}

pub fn handle_validate_command(scenario: &Option<String>, all: bool) -> anyhow::Result<()> {
    let catalog = ScenarioCatalog::from_default_fixtures();
    let paths = if all {
        catalog
            .yaml_paths()
            .into_iter()
            .filter(|p| looks_like_scenario(p))
            .collect()
    } else if let Some(path) = scenario {
        vec![catalog.resolve_path(path)]
    } else {
        anyhow::bail!("Specify --scenario <path> or --all");
    };

    if paths.is_empty() {
        println!("No scenarios found to validate.");
        return Ok(());
    }

    let mut error_count = 0;
    let mut warning_count = 0;

    for path in &paths {
        match validation::validate_scenario_file(path) {
            Ok(result) => {
                let location = if paths.len() > 1 {
                    format!(" ({})", result.path)
                } else {
                    String::new()
                };
                if result.warnings.is_empty() {
                    println!("  {} {}{}", green_check(), result.name, location);
                } else {
                    println!(
                        "  {} {}{} (with warnings)",
                        yellow_bang(),
                        result.name,
                        location
                    );
                    for warning in &result.warnings {
                        println!("    {}: {}", dim(&warning.field), warning.message);
                        warning_count += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("  {} {}", red_x(), e);
                error_count += 1;
            }
        }
    }

    println!();
    if error_count == 0 {
        println!(
            "Validated {} scenario(s), {} warning(s)",
            paths.len(),
            warning_count
        );
        Ok(())
    } else {
        println!(
            "Validated {} scenario(s): {} error(s), {} warning(s)",
            paths.len(),
            error_count,
            warning_count
        );
        anyhow::bail!("{error_count} scenario(s) failed validation");
    }
}

fn green_check() -> &'static str {
    if supports_color() {
        "\x1b[32m\u{2713}\x1b[0m"
    } else {
        "ok"
    }
}
fn red_x() -> &'static str {
    if supports_color() {
        "\x1b[31m\u{2717}\x1b[0m"
    } else {
        "FAIL"
    }
}
fn yellow_bang() -> &'static str {
    if supports_color() {
        "\x1b[33m!\x1b[0m"
    } else {
        "WARN"
    }
}
fn dim(s: &str) -> String {
    if supports_color() {
        format!("\x1b[2m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}
fn supports_color() -> bool {
    std::io::stdout().is_terminal()
}
