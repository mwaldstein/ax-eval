pub mod artifacts;
pub mod cache;
pub mod execution;
pub mod records;
pub mod setup;
pub mod transcript;
pub mod utils;

use crate::output;
use crate::results::{Cache, CacheKey, ResultRecord, ResultsDB};
use crate::scenario::Scenario;
use std::path::{Path, PathBuf};

pub struct ScenarioRunRequest<'a> {
    pub scenario: &'a Scenario,
    pub scenario_path: &'a Path,
    pub tool: &'a str,
    pub model: &'a str,
    pub dry_run: bool,
    pub no_cache: bool,
    pub timeout_secs: u64,
    pub no_judge: bool,
    pub judge_model: Option<&'a str>,
    pub judge_tool: Option<&'a str>,
    pub results_db: &'a ResultsDB,
    pub cache: &'a Cache,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioRunPlan {
    pub effective_timeout: u64,
    pub results_dir: PathBuf,
}

impl ScenarioRunRequest<'_> {
    pub fn effective_timeout(&self) -> u64 {
        self.scenario
            .run
            .as_ref()
            .and_then(|r| r.timeout_secs)
            .unwrap_or(self.timeout_secs)
    }

    pub fn results_dir(&self) -> PathBuf {
        crate::run::utils::get_results_dir(self.tool, self.model, &self.scenario.name)
    }

    pub fn plan(&self) -> ScenarioRunPlan {
        ScenarioRunPlan {
            effective_timeout: self.effective_timeout(),
            results_dir: self.results_dir(),
        }
    }

    pub fn cached_record(&self, cache_key: &CacheKey) -> anyhow::Result<Option<ResultRecord>> {
        if self.no_cache {
            return Ok(None);
        }

        crate::run::cache::check_cache(self.cache, cache_key)
    }

    pub fn should_dry_run(&self) -> bool {
        self.dry_run
    }
}

pub fn run_single_scenario(request: ScenarioRunRequest<'_>) -> anyhow::Result<ResultRecord> {
    use crate::run::execution::{
        create_adapter_and_check, determine_outcome, run_evaluation_flow, EvaluationFlowInput,
    };
    use crate::run::records::{finalize_execution, handle_dry_run, ResultRecordInput};
    use crate::run::setup::{prepare_writer_and_setup, setup_scenario_env};
    use crate::run::transcript::{write_transcript_files, TranscriptFilesInput};
    use crate::target_env::TargetEnvironment;

    let s = request.scenario;
    let tool = request.tool;
    let model = request.model;
    let plan = request.plan();

    let results_dir = plan.results_dir;
    std::fs::create_dir_all(&results_dir)?;

    let workspace = setup_scenario_env(s, request.scenario_path, &results_dir)?;
    let cache_key = workspace.cache_key(tool, model)?;

    let mut scenario = s.clone();
    scenario.target.env = TargetEnvironment::expanded_from_config(
        s.target.env.as_ref(),
        &workspace.env.root,
        &results_dir,
    )
    .into_config_env();
    let s = &scenario;

    if let Some(cached) = request.cached_record(&cache_key)? {
        println!("Cache HIT! Using cached result: {}", cached.id);
        output::print_result_summary(&cached);
        return Ok(cached);
    }

    if request.should_dry_run() {
        return handle_dry_run(s, tool, model, &cache_key);
    }

    let adapter = create_adapter_and_check(tool)?;

    let prepared =
        prepare_writer_and_setup(&results_dir, &workspace.env, s, plan.effective_timeout)?;

    let evaluation = run_evaluation_flow(EvaluationFlowInput {
        adapter: adapter.as_ref(),
        scenario: s,
        env: &workspace.env,
        tool,
        model,
        effective_timeout: plan.effective_timeout,
        no_judge: request.no_judge,
        judge_model: request.judge_model,
        judge_tool: request.judge_tool,
        writer: &prepared.writer,
        artifacts: &prepared.artifacts,
    })?;

    let outcome = determine_outcome(&evaluation.metrics);

    write_transcript_files(TranscriptFilesInput {
        writer: &prepared.writer,
        scenario: s,
        tool,
        model,
        cache_key: &cache_key,
        evaluation: &evaluation,
        outcome: &outcome,
        setup_success: prepared.setup_success,
        setup_commands: prepared.setup_commands,
    })?;

    let transcript_path = prepared
        .artifacts
        .artifacts_dir()
        .to_string_lossy()
        .to_string();
    let record = ResultRecordInput {
        scenario: s,
        tool,
        model,
        cache_key: &cache_key,
        metrics: evaluation.metrics,
        outcome,
        duration_secs: evaluation.duration.as_secs_f64(),
        cost: evaluation.cost,
        token_usage: evaluation.token_usage,
        transcript_path,
    }
    .build();

    finalize_execution(
        request.results_db,
        request.cache,
        &cache_key,
        &record,
        &results_dir,
        prepared.setup_success,
    )
}

#[cfg(test)]
mod tests;
