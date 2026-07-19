use super::manifest::{GeneratedScenarioManifest, UsageTotals};
use super::prompts::{author_prompt, inspect_prompt, summary_prompt, understanding_repair_prompt};
use super::scenarios::{validate_generated_scenarios, ValidScenario};
use super::understanding::validate_understanding_artifact;
use super::{DiscoverRequest, DEFAULT_SCENARIO_COUNT};
use crate::adapter::registry::AdapterRegistry;
use crate::adapter::ToolRunOutput;
use crate::scenario::{Evaluation, Scenario, TargetConfig, Task};
use crate::target_env::AgentEnvironment;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) struct AuthorStageResult {
    pub(super) scenario_manifests: Vec<GeneratedScenarioManifest>,
    pub(super) valid_scenarios: Vec<ValidScenario>,
}

pub(super) struct DiscoveryStageRunner<'a, 'request> {
    request: &'a DiscoverRequest<'request>,
    adapter_registry: &'a mut AdapterRegistry,
    root_dir: &'a Path,
    overhead_usage: &'a mut UsageTotals,
}

impl<'a, 'request> DiscoveryStageRunner<'a, 'request> {
    pub(super) fn new(
        request: &'a DiscoverRequest<'request>,
        adapter_registry: &'a mut AdapterRegistry,
        root_dir: &'a Path,
        overhead_usage: &'a mut UsageTotals,
    ) -> Self {
        Self {
            request,
            adapter_registry,
            root_dir,
            overhead_usage,
        }
    }

    pub(super) fn run_inspect(&mut self) -> Result<PathBuf> {
        let output = self
            .run_agent_stage("inspect", &inspect_prompt(self.request.target))
            .context("discovery inspect stage failed")?;
        self.record_stage_output("inspect", &output)?;

        let understanding_path = self.root_dir.join("understanding.md");
        self.ensure_understanding_artifact(&understanding_path)?;

        Ok(understanding_path)
    }

    pub(super) fn run_author(&mut self, understanding_path: &Path) -> Result<AuthorStageResult> {
        let output = self
            .run_agent_stage(
                "author",
                &author_prompt(
                    self.request.target,
                    DEFAULT_SCENARIO_COUNT,
                    understanding_path,
                ),
            )
            .context("discovery fixture authoring stage failed")?;
        self.record_stage_output("author", &output)?;

        let scenarios_dir = self.root_dir.join("scenarios");
        let (scenario_manifests, valid_scenarios) = validate_generated_scenarios(&scenarios_dir)
            .context("generated scenario validation failed")?;

        Ok(AuthorStageResult {
            scenario_manifests,
            valid_scenarios,
        })
    }

    pub(super) fn run_summary(
        &mut self,
        understanding_path: &Path,
        manifest_path: &Path,
    ) -> Result<()> {
        let output = self
            .run_agent_stage(
                "summary",
                &summary_prompt(understanding_path, manifest_path),
            )
            .context("discovery summary stage failed")?;
        self.record_stage_output("summary", &output)?;

        let summary_path = self.root_dir.join("discovery-summary.md");
        if !summary_path.exists() {
            fs::write(&summary_path, output.transcript.trim())?;
        }

        Ok(())
    }

    fn ensure_understanding_artifact(&mut self, understanding_path: &Path) -> Result<()> {
        if validate_understanding_artifact(understanding_path).is_ok() {
            return Ok(());
        }

        let initial_diagnostics = validate_understanding_artifact(understanding_path)
            .err()
            .unwrap_or_else(|| vec!["unknown understanding artifact issue".to_string()]);
        write_understanding_diagnostics(self.root_dir, "inspect", &initial_diagnostics)?;
        println!("Inspect stage did not produce a usable understanding.md; retrying synthesis...");

        let output = self
            .run_agent_stage(
                "inspect-repair",
                &understanding_repair_prompt(
                    self.request.target,
                    &stage_transcript_path(self.root_dir, "inspect"),
                    understanding_path,
                ),
            )
            .context("discovery understanding repair stage failed")?;
        self.record_stage_output("inspect-repair", &output)?;

        validate_understanding_artifact(understanding_path).map_err(|diagnostics| {
            let _ = write_understanding_diagnostics(self.root_dir, "inspect-repair", &diagnostics);
            anyhow::anyhow!(
                "Discovery inspect stage did not produce a usable understanding.md: {}",
                diagnostics.join("; ")
            )
        })
    }

    fn record_stage_output(&mut self, stage: &str, output: &ToolRunOutput) -> Result<()> {
        self.overhead_usage.add_tool_output(output);
        write_stage_output(self.root_dir, stage, output)
    }

    fn run_agent_stage(&mut self, stage: &str, prompt: &str) -> Result<ToolRunOutput> {
        let adapter = self
            .adapter_registry
            .resolve_checked(self.request.discover_tool)?;
        let scenario = discovery_stage_scenario(self.request.target, stage, prompt);
        let output = adapter.adapter().run(
            &scenario,
            self.root_dir,
            Some(self.request.discover_model).filter(|model| *model != "default"),
            self.request.timeout_secs,
            &AgentEnvironment::projected(
                adapter.adapter().required_agent_env(),
                &[],
                &scenario.target,
                &crate::target_env::TargetEnvironment::default(),
                adapter.adapter().requires_mcp_bearer_env(),
            ),
        )?;
        if output.exit_code != 0 {
            anyhow::bail!(
                "{} discovery stage '{}' exited with code {}",
                self.request.discover_tool,
                stage,
                output.exit_code
            );
        }
        Ok(output)
    }
}

fn discovery_stage_scenario(target: &str, stage: &str, prompt: &str) -> Scenario {
    Scenario {
        name: format!("discovery_{stage}_stage"),
        description: format!("Discovery workflow {stage} stage"),
        template_folder: ".".to_string(),
        target: TargetConfig::cli_target(target),
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
        agent_env: vec![],
    }
}

pub(super) fn stage_transcript_path(root_dir: &Path, stage: &str) -> PathBuf {
    root_dir
        .join("stages")
        .join(stage)
        .join("transcript.raw.txt")
}

pub(super) fn write_stage_output(
    root_dir: &Path,
    stage: &str,
    output: &ToolRunOutput,
) -> Result<()> {
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

pub(super) fn write_understanding_diagnostics(
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
