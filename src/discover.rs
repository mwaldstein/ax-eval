mod manifest;
mod prompts;

use self::manifest::{
    build_manifest, create_discovery_dir, display_path, safe_segment, usage_summary,
    write_manifest, write_partial_manifest, DiscoveryRunManifest, GeneratedScenarioManifest,
};
use self::prompts::{author_prompt, inspect_prompt, summary_prompt, understanding_repair_prompt};
use crate::adapter::registry::AdapterRegistry;
use crate::adapter::ToolRunOutput;
use crate::results::{Cache, ResultsDB};
use crate::run;
use crate::scenario::{self, Evaluation, Scenario, TargetConfig, Task};
use crate::target_env::TargetEnvironment;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub use self::manifest::UsageTotals;

const DEFAULT_SCENARIO_COUNT: usize = 5;
const UNDERSTANDING_HEADINGS: &[&str] = &[
    "## What the Tool Appears to Be For",
    "## Core Concepts and Mental Model",
    "## Primary Workflows",
    "## Useful Goal Areas",
    "## Evidence Consulted",
    "## Self-Description Quality",
    "## Ambiguities or Missing Information",
    "## Five Candidate Scenario Ideas",
];

pub struct DiscoverRequest<'a> {
    pub target: &'a str,
    pub run_tool: &'a str,
    pub run_model: &'a str,
    pub discover_tool: &'a str,
    pub discover_model: &'a str,
    pub judge_model: Option<&'a str>,
    pub judge_tool: Option<&'a str>,
    pub timeout_secs: u64,
    pub results_base_dir: &'a Path,
    pub results_db: &'a ResultsDB,
    pub cache: &'a Cache,
}

struct ValidScenario {
    path: PathBuf,
    scenario: Scenario,
}

pub fn run_discovery(request: DiscoverRequest<'_>) -> Result<PathBuf> {
    check_target_available(request.target)?;

    let root_dir = create_discovery_dir(&request)?;
    let scenarios_dir = root_dir.join("scenarios");
    let runs_dir = root_dir.join("runs");
    fs::create_dir_all(&scenarios_dir)?;
    fs::create_dir_all(&runs_dir)?;

    println!("Discovery artifacts: {}", root_dir.display());
    println!("Inspecting target: {}", request.target);

    let mut overhead_usage = UsageTotals::default();
    let mut adapter_registry = AdapterRegistry::new();
    let inspect_output = run_agent_stage(
        &mut adapter_registry,
        request.discover_tool,
        request.discover_model,
        request.target,
        &root_dir,
        request.timeout_secs,
        &inspect_prompt(request.target),
    )
    .context("discovery inspect stage failed")?;
    overhead_usage.add_tool_output(&inspect_output);
    write_stage_output(&root_dir, "inspect", &inspect_output)?;

    let understanding_path = root_dir.join("understanding.md");
    ensure_understanding_artifact(
        &request,
        &mut adapter_registry,
        &root_dir,
        &understanding_path,
        &mut overhead_usage,
    )?;

    println!("Authoring generated scenarios...");
    let author_output = run_agent_stage(
        &mut adapter_registry,
        request.discover_tool,
        request.discover_model,
        request.target,
        &root_dir,
        request.timeout_secs,
        &author_prompt(request.target, DEFAULT_SCENARIO_COUNT, &understanding_path),
    )
    .context("discovery fixture authoring stage failed")?;
    overhead_usage.add_tool_output(&author_output);
    write_stage_output(&root_dir, "author", &author_output)?;

    let (scenario_manifests, valid_scenarios) = validate_generated_scenarios(&scenarios_dir)
        .context("generated scenario validation failed")?;
    if valid_scenarios.is_empty() {
        write_partial_manifest(&request, &root_dir, scenario_manifests)?;
        anyhow::bail!("Discovery generated zero valid scenarios");
    }

    println!(
        "Running {} valid generated scenario(s) as one discovery batch...",
        valid_scenarios.len()
    );
    let (run_manifests, run_usage) =
        run_generated_scenarios(&request, &mut adapter_registry, &runs_dir, &valid_scenarios);

    let manifest_path = root_dir.join("discovery.json");
    let summary_path = root_dir.join("discovery-summary.md");
    let usage = usage_summary(overhead_usage, run_usage);
    let manifest = build_manifest(
        &request,
        &root_dir,
        &scenario_manifests,
        &run_manifests,
        usage,
    );
    write_manifest(&manifest_path, &manifest)?;

    println!("Summarizing discovery results...");
    let summary_output = run_agent_stage(
        &mut adapter_registry,
        request.discover_tool,
        request.discover_model,
        request.target,
        &root_dir,
        request.timeout_secs,
        &summary_prompt(&understanding_path, &manifest_path),
    )
    .context("discovery summary stage failed")?;
    write_stage_output(&root_dir, "summary", &summary_output)?;
    if !summary_path.exists() {
        fs::write(&summary_path, summary_output.transcript.trim())?;
    }

    let mut final_manifest = manifest;
    final_manifest
        .usage
        .discovery_overhead
        .add_tool_output(&summary_output);
    final_manifest
        .usage
        .combined
        .add_tool_output(&summary_output);
    write_manifest(&manifest_path, &final_manifest)?;

    println!("Discovery complete: {}", root_dir.display());
    Ok(root_dir)
}

fn run_agent_stage(
    adapter_registry: &mut AdapterRegistry,
    tool: &str,
    model: &str,
    target: &str,
    cwd: &Path,
    timeout_secs: u64,
    prompt: &str,
) -> Result<ToolRunOutput> {
    let adapter = adapter_registry.resolve_checked(tool)?;
    let scenario = Scenario {
        name: "discovery_stage".to_string(),
        description: "Discovery workflow agent stage".to_string(),
        template_folder: ".".to_string(),
        target: TargetConfig {
            binary: target.to_string(),
            command_pattern: None,
            health_check: None,
            env: None,
        },
        task: Task {
            prompt: prompt.to_string(),
        },
        evaluation: Evaluation {
            gates: vec![],
            judge: None,
            composite: None,
        },
        tier: 0,
        tool_matrix: None,
        setup: None,
        tags: vec!["discovery".to_string()],
        run: None,
        scripts: None,
        interaction: Default::default(),
    };
    let output = adapter.adapter().run(
        &scenario,
        cwd,
        Some(model).filter(|m| *m != "default"),
        timeout_secs,
        &TargetEnvironment::default(),
    )?;
    if output.exit_code != 0 {
        anyhow::bail!("{} exited with code {}", tool, output.exit_code);
    }
    Ok(output)
}

fn ensure_understanding_artifact(
    request: &DiscoverRequest<'_>,
    adapter_registry: &mut AdapterRegistry,
    root_dir: &Path,
    understanding_path: &Path,
    overhead_usage: &mut UsageTotals,
) -> Result<()> {
    if validate_understanding_artifact(understanding_path).is_ok() {
        return Ok(());
    }

    let initial_diagnostics = validate_understanding_artifact(understanding_path)
        .err()
        .unwrap_or_else(|| vec!["unknown understanding artifact issue".to_string()]);
    write_understanding_diagnostics(root_dir, "inspect", &initial_diagnostics)?;
    println!("Inspect stage did not produce a usable understanding.md; retrying synthesis...");

    let repair_output = run_agent_stage(
        adapter_registry,
        request.discover_tool,
        request.discover_model,
        request.target,
        root_dir,
        request.timeout_secs,
        &understanding_repair_prompt(
            request.target,
            &root_dir
                .join("stages")
                .join("inspect")
                .join("transcript.raw.txt"),
            understanding_path,
        ),
    )
    .context("discovery understanding repair stage failed")?;
    overhead_usage.add_tool_output(&repair_output);
    write_stage_output(root_dir, "inspect-repair", &repair_output)?;

    validate_understanding_artifact(understanding_path).map_err(|diagnostics| {
        let _ = write_understanding_diagnostics(root_dir, "inspect-repair", &diagnostics);
        anyhow::anyhow!(
            "Discovery inspect stage did not produce a usable understanding.md: {}",
            diagnostics.join("; ")
        )
    })
}

fn validate_understanding_artifact(path: &Path) -> std::result::Result<(), Vec<String>> {
    let content = fs::read_to_string(path).map_err(|error| {
        vec![format!(
            "understanding.md is missing or unreadable: {error}"
        )]
    })?;
    diagnose_understanding_content(&content)
}

fn diagnose_understanding_content(content: &str) -> std::result::Result<(), Vec<String>> {
    let trimmed = content.trim();
    let mut diagnostics = Vec::new();

    if trimmed.is_empty() {
        diagnostics.push("understanding.md is empty".to_string());
    }

    let first_nonempty = content.lines().find(|line| !line.trim().is_empty());
    let shell_prompt_lines = content
        .lines()
        .filter(|line| line.trim_start().starts_with("$ "))
        .count();
    let exit_code_lines = content
        .lines()
        .filter(|line| line.trim_start().starts_with("exit code:"))
        .count();
    if first_nonempty.is_some_and(|line| line.trim_start().starts_with("$ "))
        || shell_prompt_lines >= 3
        || exit_code_lines >= 3
    {
        diagnostics.push(
            "understanding.md appears to be a command transcript rather than synthesized Markdown"
                .to_string(),
        );
    }

    let lowercase_content = content.to_lowercase();
    for heading in UNDERSTANDING_HEADINGS {
        if !lowercase_content.contains(&heading.to_lowercase()) {
            diagnostics.push(format!(
                "understanding.md is missing required heading: {heading}"
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn write_understanding_diagnostics(
    root_dir: &Path,
    stage: &str,
    diagnostics: &[String],
) -> Result<()> {
    let stage_dir = root_dir.join("stages").join(stage);
    fs::create_dir_all(&stage_dir)?;
    fs::write(
        stage_dir.join("understanding-diagnostics.txt"),
        diagnostics.join("\n"),
    )?;
    Ok(())
}

fn run_generated_scenarios(
    request: &DiscoverRequest<'_>,
    adapter_registry: &mut AdapterRegistry,
    runs_dir: &Path,
    valid_scenarios: &[ValidScenario],
) -> (Vec<DiscoveryRunManifest>, UsageTotals) {
    let mut run_manifests = Vec::new();
    let mut usage = UsageTotals::default();

    for valid in valid_scenarios {
        let result_dir = runs_dir.join(safe_segment(&valid.scenario.name));
        let adapter = match adapter_registry.resolve_checked(request.run_tool) {
            Ok(adapter) => adapter,
            Err(error) => {
                run_manifests.push(DiscoveryRunManifest {
                    scenario_name: valid.scenario.name.clone(),
                    scenario_path: display_path(&valid.path),
                    result_dir: display_path(&result_dir),
                    status: "harness_error".to_string(),
                    run_id: None,
                    outcome: None,
                    judge_score: None,
                    judge_passed: None,
                    error_count: 0,
                    failed_call_count: 0,
                    error: Some(format!("{error:#}")),
                });
                continue;
            }
        };
        let result = run::run_single_scenario_with_adapter(
            run::ScenarioRunRequest {
                scenario: &valid.scenario,
                scenario_path: &valid.path,
                tool: request.run_tool,
                model: request.run_model,
                dry_run: false,
                no_cache: true,
                timeout_secs: request.timeout_secs,
                no_judge: false,
                judge_model: request.judge_model,
                judge_tool: request.judge_tool,
                results_db: request.results_db,
                cache: request.cache,
                results_dir_override: Some(result_dir.clone()),
            },
            &adapter,
        );

        match result {
            Ok(record) => {
                usage.add_record(&record);
                run_manifests.push(DiscoveryRunManifest {
                    scenario_name: valid.scenario.name.clone(),
                    scenario_path: display_path(&valid.path),
                    result_dir: display_path(&result_dir),
                    status: "completed".to_string(),
                    run_id: Some(record.id),
                    outcome: Some(record.outcome),
                    judge_score: record.judge_score,
                    judge_passed: record.metrics.judge_passed,
                    error_count: record.metrics.efficiency.error_count,
                    failed_call_count: record.metrics.efficiency.error_count,
                    error: None,
                });
            }
            Err(error) => {
                run_manifests.push(DiscoveryRunManifest {
                    scenario_name: valid.scenario.name.clone(),
                    scenario_path: display_path(&valid.path),
                    result_dir: display_path(&result_dir),
                    status: "harness_error".to_string(),
                    run_id: None,
                    outcome: None,
                    judge_score: None,
                    judge_passed: None,
                    error_count: 0,
                    failed_call_count: 0,
                    error: Some(format!("{error:#}")),
                });
            }
        }
    }

    (run_manifests, usage)
}

fn validate_generated_scenarios(
    scenarios_dir: &Path,
) -> Result<(Vec<GeneratedScenarioManifest>, Vec<ValidScenario>)> {
    let mut paths = Vec::new();
    collect_scenario_yaml_paths(scenarios_dir, &mut paths)?;
    paths.sort();

    let mut manifests = Vec::new();
    let mut valid_scenarios = Vec::new();

    for path in paths {
        let mut diagnostics = Vec::new();
        match scenario::load(&path) {
            Ok(mut scenario) => {
                diagnostics.extend(scenario::discovery::apply_discovery_contract(
                    &path,
                    &mut scenario,
                ));
                let valid = diagnostics.is_empty();
                manifests.push(GeneratedScenarioManifest {
                    path: display_path(&path),
                    valid,
                    name: Some(scenario.name.clone()),
                    diagnostics,
                });
                if valid {
                    valid_scenarios.push(ValidScenario { path, scenario });
                }
            }
            Err(error) => {
                manifests.push(GeneratedScenarioManifest {
                    path: display_path(&path),
                    valid: false,
                    name: None,
                    diagnostics: vec![format!("{error:#}")],
                });
            }
        }
    }

    Ok((manifests, valid_scenarios))
}

fn write_stage_output(root_dir: &Path, stage: &str, output: &ToolRunOutput) -> Result<()> {
    let stage_dir = root_dir.join("stages").join(stage);
    fs::create_dir_all(&stage_dir)?;
    fs::write(stage_dir.join("transcript.raw.txt"), &output.transcript)?;
    if let Some(raw) = &output.raw_output {
        fs::write(stage_dir.join("tool-output.raw.txt"), raw)?;
    }
    if output.token_usage.is_some() || output.cost_usd.is_some() {
        let (input, out_tokens) = output
            .token_usage
            .as_ref()
            .map(|tokens| (tokens.input, tokens.output))
            .unwrap_or((0, 0));
        fs::write(
            stage_dir.join("usage.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "input_tokens": input,
                "output_tokens": out_tokens,
                "cost_usd": output.cost_usd,
            }))?,
        )?;
    }
    Ok(())
}

fn check_target_available(target: &str) -> Result<()> {
    let binary = target
        .split_whitespace()
        .next()
        .filter(|part| !part.is_empty())
        .context("Discovery target cannot be empty")?;
    let status = std::process::Command::new(binary)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match status {
        Ok(_) => Ok(()),
        Err(error) => anyhow::bail!("Discovery target is not executable: {binary}: {error}"),
    }
}

fn collect_scenario_yaml_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|name| name.to_str());
            if matches!(name, Some("rubrics" | "templates")) {
                continue;
            }
            collect_scenario_yaml_paths(&path, paths)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            paths.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_discovery_scenario_contract_and_normalizes_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = dir.path().join("scenarios");
        fs::create_dir_all(scenarios.join("templates/goal")).expect("template dir");
        fs::create_dir_all(scenarios.join("rubrics")).expect("rubrics dir");
        fs::write(
            scenarios.join("rubrics/goal.yaml"),
            r#"
criteria:
  - id: quality
    weight: 1.0
    description: "Quality"
output:
  format: json
  require_fields: [scores, weighted_score, confidence, issues, highlights]
"#,
        )
        .expect("rubric");
        fs::write(
            scenarios.join("goal.yaml"),
            r#"
name: goal
description: "Goal"
template_folder: templates/goal
target:
  binary: qipu
task:
  prompt: "Use qipu well"
evaluation:
  gates: []
  judge:
    enabled: true
    rubric: rubrics/goal.yaml
    pass_threshold: 0.70
"#,
        )
        .expect("scenario");

        let (manifests, valid) =
            validate_generated_scenarios(&scenarios).expect("validate scenarios");

        assert_eq!(manifests.len(), 1);
        assert!(manifests[0].valid);
        assert_eq!(valid.len(), 1);
        assert!(Path::new(&valid[0].scenario.template_folder).is_absolute());
        assert!(Path::new(
            valid[0]
                .scenario
                .evaluation
                .judge
                .as_ref()
                .expect("judge")
                .rubric
                .as_ref()
                .expect("rubric")
        )
        .is_absolute());
    }

    #[test]
    fn validates_discovery_scenario_with_default_judge_rubric() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = dir.path().join("scenarios");
        fs::create_dir_all(scenarios.join("templates/goal")).expect("template dir");
        fs::write(
            scenarios.join("goal.yaml"),
            r#"
name: goal
description: "Goal"
template_folder: templates/goal
target:
  binary: qipu
task:
  prompt: "Use qipu well"
evaluation:
  gates: []
  judge:
    enabled: true
    pass_threshold: 0.70
"#,
        )
        .expect("scenario");

        let (manifests, valid) =
            validate_generated_scenarios(&scenarios).expect("validate scenarios");

        assert_eq!(manifests.len(), 1);
        assert!(manifests[0].valid);
        assert_eq!(valid.len(), 1);
        assert!(Path::new(&valid[0].scenario.template_folder).is_absolute());
        assert_eq!(
            valid[0]
                .scenario
                .evaluation
                .judge
                .as_ref()
                .expect("judge")
                .rubric,
            None
        );
    }

    #[test]
    fn discovery_contract_rejects_gated_or_unjudged_scenarios() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = dir.path().join("scenarios");
        fs::create_dir_all(&scenarios).expect("scenarios dir");
        fs::write(
            scenarios.join("bad.yaml"),
            r#"
name: bad
description: "Bad"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Use qipu"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#,
        )
        .expect("scenario");

        let (manifests, valid) =
            validate_generated_scenarios(&scenarios).expect("validate scenarios");

        assert!(valid.is_empty());
        assert!(!manifests[0].valid);
        assert!(manifests[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("must not include deterministic gates")));
        assert!(manifests[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("must include an enabled judge")));
    }

    #[test]
    fn usage_summary_separates_overhead_and_scenario_runs() {
        let overhead = UsageTotals {
            input_tokens: 10,
            output_tokens: 5,
            cost_usd: Some(0.25),
        };
        let scenario_runs = UsageTotals {
            input_tokens: 7,
            output_tokens: 3,
            cost_usd: Some(0.50),
        };

        let usage = usage_summary(overhead, scenario_runs);

        assert_eq!(usage.discovery_overhead.input_tokens, 10);
        assert_eq!(usage.scenario_runs.input_tokens, 7);
        assert_eq!(usage.combined.input_tokens, 17);
        assert_eq!(usage.combined.output_tokens, 8);
        assert_eq!(usage.combined.cost_usd, Some(0.75));
    }

    #[test]
    fn understanding_validation_rejects_transcript_fallback() {
        let content = r#"
$ qipu --help
Knowledge graph CLI designed for scripts and agents

exit code: 0

$ qipu create --help
Create a new note

exit code: 0
"#;

        let diagnostics =
            diagnose_understanding_content(content).expect_err("transcript should be rejected");

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("appears to be a command transcript")));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("## What the Tool Appears to Be For")));
    }

    #[test]
    fn understanding_validation_accepts_required_sections() {
        let content = format!(
            "# Discovery Understanding\n\n{}\n",
            UNDERSTANDING_HEADINGS
                .iter()
                .map(|heading| format!("{heading}\n\nSynthesis for this section."))
                .collect::<Vec<_>>()
                .join("\n\n")
        );

        diagnose_understanding_content(&content).expect("valid understanding");
    }
}
