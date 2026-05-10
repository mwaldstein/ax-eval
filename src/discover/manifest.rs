use super::{DiscoverRequest, DEFAULT_SCENARIO_COUNT};
use crate::adapter::ToolRunOutput;
use crate::results::ResultRecord;
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct UsageTotals {
    pub input_tokens: usize,
    pub output_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

impl UsageTotals {
    pub(super) fn add_tool_output(&mut self, output: &ToolRunOutput) {
        if let Some(tokens) = &output.token_usage {
            self.input_tokens += tokens.input;
            self.output_tokens += tokens.output;
        }
        self.add_cost(output.cost_usd);
    }

    pub(super) fn add_record(&mut self, record: &ResultRecord) {
        if let Some(tokens) = &record.token_usage {
            self.input_tokens += tokens.input;
            self.output_tokens += tokens.output;
        }
        self.add_cost(record.cost_usd);
    }

    pub(super) fn add_cost(&mut self, cost: Option<f64>) {
        if let Some(cost) = cost {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + cost);
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct DiscoveryManifest {
    target: String,
    scenario_count_requested: usize,
    run_agent: AgentConfig,
    discovery_agent: AgentConfig,
    judge: JudgeSelection,
    artifacts: DiscoveryArtifacts,
    generated_scenarios: Vec<GeneratedScenarioManifest>,
    run_results: Vec<DiscoveryRunManifest>,
    pub(super) usage: DiscoveryUsage,
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
pub(super) struct GeneratedScenarioManifest {
    pub(super) path: String,
    pub(super) valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    pub(super) diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct DiscoveryRunManifest {
    pub(super) scenario_name: String,
    pub(super) scenario_path: String,
    pub(super) result_dir: String,
    pub(super) status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) judge_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) judge_passed: Option<bool>,
    pub(super) error_count: usize,
    pub(super) failed_call_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct DiscoveryUsage {
    pub(super) discovery_overhead: UsageTotals,
    pub(super) scenario_runs: UsageTotals,
    pub(super) combined: UsageTotals,
}

pub(super) fn create_discovery_dir(request: &DiscoverRequest<'_>) -> Result<PathBuf> {
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

pub(super) fn build_manifest(
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

pub(super) fn usage_summary(overhead: UsageTotals, scenario_runs: UsageTotals) -> DiscoveryUsage {
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

pub(super) fn write_partial_manifest(
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

pub(super) fn write_manifest(path: &Path, manifest: &DiscoveryManifest) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(manifest)?)?;
    Ok(())
}

pub(super) fn safe_segment(value: &str) -> String {
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

pub(super) fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
