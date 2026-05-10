mod batch;
mod manifest;
mod prompts;
mod scenarios;
mod stages;
mod understanding;

use self::manifest::{
    build_manifest, create_discovery_dir, usage_summary, write_manifest, write_partial_manifest,
};
use self::stages::DiscoveryStageRunner;
use crate::adapter::registry::AdapterRegistry;
use crate::results::{Cache, ResultsDB};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub use self::manifest::UsageTotals;

const DEFAULT_SCENARIO_COUNT: usize = 5;
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
    let understanding_path = {
        let mut stage_runner = DiscoveryStageRunner::new(
            &request,
            &mut adapter_registry,
            &root_dir,
            &mut overhead_usage,
        );
        stage_runner.run_inspect()?
    };

    println!("Authoring generated scenarios...");
    let author_result = {
        let mut stage_runner = DiscoveryStageRunner::new(
            &request,
            &mut adapter_registry,
            &root_dir,
            &mut overhead_usage,
        );
        stage_runner.run_author(&understanding_path)?
    };
    if author_result.valid_scenarios.is_empty() {
        write_partial_manifest(&request, &root_dir, author_result.scenario_manifests)?;
        anyhow::bail!("Discovery generated zero valid scenarios");
    }

    println!(
        "Running {} valid generated scenario(s) as one discovery batch...",
        author_result.valid_scenarios.len()
    );
    let (run_manifests, run_usage) = batch::run_generated_scenarios(
        &request,
        &mut adapter_registry,
        &runs_dir,
        &author_result.valid_scenarios,
    );

    let manifest_path = root_dir.join("discovery.json");
    let usage = usage_summary(overhead_usage, run_usage);
    let manifest = build_manifest(
        &request,
        &root_dir,
        &author_result.scenario_manifests,
        &run_manifests,
        usage,
    );
    write_manifest(&manifest_path, &manifest)?;

    println!("Summarizing discovery results...");
    {
        let mut stage_runner = DiscoveryStageRunner::new(
            &request,
            &mut adapter_registry,
            &root_dir,
            &mut overhead_usage,
        );
        stage_runner.run_summary(&understanding_path, &manifest_path)?;
    }

    let final_manifest = build_manifest(
        &request,
        &root_dir,
        &author_result.scenario_manifests,
        &run_manifests,
        usage_summary(overhead_usage, run_usage),
    );
    write_manifest(&manifest_path, &final_manifest)?;

    println!("Discovery complete: {}", root_dir.display());
    Ok(root_dir)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
