use crate::adapter::registry::AdapterRegistry;
use crate::adapter::ToolRunOutput;
use crate::results::{Cache, ResultRecord, ResultsDB};
use crate::run;
use crate::scenario::{self, Evaluation, Scenario, TargetConfig, Task};
use crate::target_env::TargetEnvironment;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct UsageTotals {
    pub input_tokens: usize,
    pub output_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

impl UsageTotals {
    fn add_tool_output(&mut self, output: &ToolRunOutput) {
        if let Some(tokens) = &output.token_usage {
            self.input_tokens += tokens.input;
            self.output_tokens += tokens.output;
        }
        self.add_cost(output.cost_usd);
    }

    fn add_record(&mut self, record: &ResultRecord) {
        if let Some(tokens) = &record.token_usage {
            self.input_tokens += tokens.input;
            self.output_tokens += tokens.output;
        }
        self.add_cost(record.cost_usd);
    }

    fn add_cost(&mut self, cost: Option<f64>) {
        if let Some(cost) = cost {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + cost);
        }
    }
}

#[derive(Debug, Serialize)]
struct DiscoveryManifest {
    target: String,
    scenario_count_requested: usize,
    run_agent: AgentConfig,
    discovery_agent: AgentConfig,
    judge: JudgeSelection,
    artifacts: DiscoveryArtifacts,
    generated_scenarios: Vec<GeneratedScenarioManifest>,
    run_results: Vec<DiscoveryRunManifest>,
    usage: DiscoveryUsage,
}

#[derive(Debug, Serialize)]
struct AgentConfig {
    tool: String,
    model: String,
}

#[derive(Debug, Serialize)]
struct JudgeSelection {
    #[serde(skip_serializing_if = "Option::is_none")]
    tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiscoveryArtifacts {
    root_dir: String,
    understanding_md: String,
    scenarios_dir: String,
    runs_dir: String,
    summary_md: String,
}

#[derive(Debug, Clone, Serialize)]
struct GeneratedScenarioManifest {
    path: String,
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiscoveryRunManifest {
    scenario_name: String,
    scenario_path: String,
    result_dir: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    judge_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    judge_passed: Option<bool>,
    error_count: usize,
    failed_call_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiscoveryUsage {
    discovery_overhead: UsageTotals,
    scenario_runs: UsageTotals,
    combined: UsageTotals,
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

fn inspect_prompt(target: &str) -> String {
    format!(
        r#"You are inspecting a CLI tool for llm-tool-test discovery.

Target executable: {target}

Work only from the executable command surface. Use the target command's help,
subcommand help, examples, errors, and any files the command itself points you
to. Do not rely on external documentation unless the command explicitly opens
or names it.

Create a Markdown file at ./understanding.md. Do this before attempting any
destructive or permission-sensitive experiments. Keep all scratch work inside
the current directory; do not use /tmp, parent directories, or other external
paths.

Focus on why the tool exists and how an LLM agent should think about using it.
Use exactly these section headings:

# Discovery Understanding

## What the Tool Appears to Be For

## Core Concepts and Mental Model

## Primary Workflows

## Useful Goal Areas

## Evidence Consulted

## Self-Description Quality

## Ambiguities or Missing Information

## Five Candidate Scenario Ideas

The final file must be a synthesized Markdown report, not a command transcript.
If an exploratory command is denied or fails, still write ./understanding.md
from the evidence already collected and mention the limitation under
Ambiguities or Missing Information.

Keep the artifact useful to the tool author as evidence of how self-describing
the executable is."#
    )
}

fn understanding_repair_prompt(
    target: &str,
    transcript_path: &Path,
    understanding_path: &Path,
) -> String {
    format!(
        r#"You are repairing the inspect artifact for llm-tool-test discovery.

Target executable: {target}
Inspect transcript: {}
Required output file: {}

Read the inspect transcript and synthesize the required Markdown understanding
artifact. Work only from the transcript and any command-surface evidence named
inside it. Do not run more experiments unless absolutely necessary, and keep any
scratch files inside the current directory.

Overwrite the required output file with a synthesized Markdown report using
exactly these section headings:

# Discovery Understanding

## What the Tool Appears to Be For

## Core Concepts and Mental Model

## Primary Workflows

## Useful Goal Areas

## Evidence Consulted

## Self-Description Quality

## Ambiguities or Missing Information

## Five Candidate Scenario Ideas

The output must not be a command transcript. Include concise evidence references
instead of pasting command output."#,
        transcript_path.display(),
        understanding_path.display()
    )
}

fn author_prompt(target: &str, count: usize, understanding_path: &Path) -> String {
    format!(
        r#"You are authoring llm-tool-test discovery fixtures.

Target executable: {target}
Understanding document: {}
Required scenario count: {count}

Read the understanding document, then create a coordinated set of {count}
complex, goal-oriented llm-tool-test scenarios under ./scenarios/.

Hard requirements:
- Write complete runnable YAML scenario files under ./scenarios/.
- Create any required template directories beside the YAML files.
- Each scenario must use target.binary: "{target}".
- Each scenario must have evaluation.gates: [].
- Each scenario must include evaluation.judge.enabled: true.
- Omit evaluation.judge.rubric unless the scenario needs custom criteria. The
  default judge rubric assesses goal achievement, CLI usage quality, and
  efficiency.
- If custom criteria are needed, prefer inline evaluation.judge.criteria. Use a
  separate rubric file only when the criteria need a reusable output contract.
- Use pass_threshold as a general rubric reference, not as the main discovery
  value. 0.70 is a reasonable default.
- Prefer goal-based tasks over command-prescriptive tasks. The prompt should
  ask for an outcome that requires understanding the tool's role, not a fixed
  command sequence.
- Avoid near-duplicate scenarios; cover different useful goal areas.
- If extra notes would help the tool author, write ./scenarios/README.md.

Useful scenario shape:

name: discover_example_goal
description: "Goal-oriented discovery scenario"
template_folder: templates/discover_example_goal
target:
  binary: "{target}"
task:
  prompt: |
    Achieve a realistic user goal with the target tool. Decide which commands
    are appropriate and verify your work.
evaluation:
  gates: []
  judge:
    enabled: true
    pass_threshold: 0.70
tags:
  - discovery

Use relative template paths beside each scenario file. If you create a custom
rubric path, keep it relative to the generated YAML; the harness will resolve it."#,
        understanding_path.display()
    )
}

fn summary_prompt(understanding_path: &Path, manifest_path: &Path) -> String {
    format!(
        r#"You are summarizing an llm-tool-test discovery run for the target tool author.

Read:
- Understanding: {}
- Discovery manifest: {}
- Scenario run reports and evaluations under ./runs/

Write ./discovery-summary.md as a concise but useful Markdown report. Focus on:
- What the inspecting agent understood about why the tool exists
- How self-describing the tool appeared to be
- Quality and diversity of the generated goal-oriented scenarios
- How well the evaluated agent used the target tool, emphasizing judge scores,
  judge rationales, confidence, issues, highlights, failed calls, retries, and
  help-seeking rather than only pass/fail
- Which failures are fixture-authoring problems, tool self-description
  problems, agent usage problems, or harness problems
- High-impact recommendations for improving the tool's LLM usability"#,
        understanding_path.display(),
        manifest_path.display()
    )
}

fn create_discovery_dir(request: &DiscoverRequest<'_>) -> Result<PathBuf> {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let dir_name = format!(
        "{}-discover-{}-{}-{}",
        timestamp,
        safe_segment(request.target),
        safe_segment(request.run_tool),
        safe_segment(request.run_model)
    );
    let dir = request.results_base_dir.join(dir_name);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn build_manifest(
    request: &DiscoverRequest<'_>,
    root_dir: &Path,
    scenarios: &[GeneratedScenarioManifest],
    run_results: &[DiscoveryRunManifest],
    usage: DiscoveryUsage,
) -> DiscoveryManifest {
    DiscoveryManifest {
        target: request.target.to_string(),
        scenario_count_requested: DEFAULT_SCENARIO_COUNT,
        run_agent: AgentConfig {
            tool: request.run_tool.to_string(),
            model: request.run_model.to_string(),
        },
        discovery_agent: AgentConfig {
            tool: request.discover_tool.to_string(),
            model: request.discover_model.to_string(),
        },
        judge: JudgeSelection {
            tool: request.judge_tool.map(str::to_string),
            model: request.judge_model.map(str::to_string),
        },
        artifacts: DiscoveryArtifacts {
            root_dir: display_path(root_dir),
            understanding_md: display_path(&root_dir.join("understanding.md")),
            scenarios_dir: display_path(&root_dir.join("scenarios")),
            runs_dir: display_path(&root_dir.join("runs")),
            summary_md: display_path(&root_dir.join("discovery-summary.md")),
        },
        generated_scenarios: scenarios.to_vec(),
        run_results: run_results.to_vec(),
        usage,
    }
}

fn usage_summary(overhead: UsageTotals, scenario_runs: UsageTotals) -> DiscoveryUsage {
    let mut combined = overhead;
    combined.input_tokens += scenario_runs.input_tokens;
    combined.output_tokens += scenario_runs.output_tokens;
    combined.add_cost(scenario_runs.cost_usd);
    DiscoveryUsage {
        discovery_overhead: overhead,
        scenario_runs,
        combined,
    }
}

fn write_partial_manifest(
    request: &DiscoverRequest<'_>,
    root_dir: &Path,
    scenario_manifests: Vec<GeneratedScenarioManifest>,
) -> Result<()> {
    let manifest = build_manifest(
        request,
        root_dir,
        &scenario_manifests,
        &[],
        usage_summary(UsageTotals::default(), UsageTotals::default()),
    );
    write_manifest(&root_dir.join("discovery.json"), &manifest)
}

fn write_manifest(path: &Path, manifest: &DiscoveryManifest) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(manifest)?)?;
    Ok(())
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

fn safe_segment(value: &str) -> String {
    let mut safe = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.len() > 80 {
        safe.truncate(80);
    }
    if safe.is_empty() {
        "unknown".to_string()
    } else {
        safe
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
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
