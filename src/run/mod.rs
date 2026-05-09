pub mod artifacts;
pub mod cache;
pub mod execution;
pub mod records;
pub mod setup;
pub mod transcript;
pub mod utils;

use crate::output;
use crate::results::{Cache, ResultRecord, ResultsDB};
use crate::scenario::Scenario;

#[allow(clippy::too_many_arguments)]
pub fn run_single_scenario(
    s: &Scenario,
    scenario_path: &std::path::Path,
    tool: &str,
    model: &str,
    dry_run: bool,
    no_cache: bool,
    timeout_secs: u64,
    no_judge: bool,
    judge_model: Option<&str>,
    judge_tool: Option<&str>,
    _base_dir: &std::path::Path,
    results_db: &ResultsDB,
    cache: &Cache,
) -> anyhow::Result<ResultRecord> {
    use crate::run::cache::{check_cache, compute_cache_key_with_fixture};
    use crate::run::execution::{
        create_adapter_and_check, determine_outcome, run_evaluation_flow, EvaluationFlowInput,
    };
    use crate::run::records::{build_result_record, finalize_execution, handle_dry_run};
    use crate::run::setup::{prepare_writer_and_setup, setup_scenario_env};
    use crate::run::transcript::{write_transcript_files, TranscriptFilesInput};

    let effective_timeout = s
        .run
        .as_ref()
        .and_then(|r| r.timeout_secs)
        .unwrap_or(timeout_secs);

    let results_dir = crate::run::utils::get_results_dir(tool, model, &s.name);
    std::fs::create_dir_all(&results_dir)?;

    let workspace = setup_scenario_env(s, scenario_path, &results_dir)?;
    let cache_key = compute_cache_key_with_fixture(
        &workspace.scenario_yaml,
        &workspace.prompt,
        &workspace.env.root,
        tool,
        model,
    )?;

    if !no_cache {
        if let Some(cached) = check_cache(cache, &cache_key)? {
            println!("Cache HIT! Using cached result: {}", cached.id);
            output::print_result_summary(&cached);
            return Ok(cached);
        }
    }

    if dry_run {
        return handle_dry_run(s, tool, model, &cache_key);
    }

    let adapter = create_adapter_and_check(tool)?;

    let prepared = prepare_writer_and_setup(&results_dir, &workspace.env, s, effective_timeout)?;

    let evaluation = run_evaluation_flow(EvaluationFlowInput {
        adapter: adapter.as_ref(),
        scenario: s,
        env: &workspace.env,
        tool,
        model,
        effective_timeout,
        no_judge,
        judge_model,
        judge_tool,
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
    let record = build_result_record(
        s,
        tool,
        model,
        &cache_key,
        evaluation.metrics,
        outcome,
        evaluation.duration.as_secs_f64(),
        evaluation.cost,
        evaluation.token_usage,
        transcript_path,
    );

    finalize_execution(
        results_db,
        cache,
        &cache_key,
        &record,
        &results_dir,
        prepared.setup_success,
    )
}

#[cfg(test)]
mod tests;
