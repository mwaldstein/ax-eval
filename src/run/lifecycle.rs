use crate::output;
use crate::results::{CacheKey, ResultRecord};
use crate::run::execution::{
    create_adapter_and_check, determine_outcome, run_evaluation_flow, EvaluationFlowInput,
};
use crate::run::records::{finalize_execution, handle_dry_run, ResultRecordInput};
use crate::run::setup::{prepare_writer_and_setup, PreparedRunContext, PreparedScenarioRun};
use crate::run::transcript::{write_transcript_files, TranscriptFilesInput};
use crate::run::{ScenarioRunPlan, ScenarioRunRequest};

pub struct ScenarioRunLifecycle<'a> {
    request: ScenarioRunRequest<'a>,
    plan: ScenarioRunPlan,
}

impl<'a> ScenarioRunLifecycle<'a> {
    pub fn new(request: ScenarioRunRequest<'a>) -> Self {
        let plan = request.plan();
        Self { request, plan }
    }

    pub fn run(self) -> anyhow::Result<ResultRecord> {
        std::fs::create_dir_all(&self.plan.results_dir)?;

        let context = PreparedRunContext::new(
            self.request.scenario,
            self.request.scenario_path,
            &self.plan.results_dir,
        )?;
        let cache_key = context.cache_key(self.request.tool, self.request.model)?;

        if let Some(record) = self.short_circuit_record(&cache_key)? {
            return Ok(record);
        }

        self.execute_prepared(context, cache_key)
    }

    fn short_circuit_record(&self, cache_key: &CacheKey) -> anyhow::Result<Option<ResultRecord>> {
        if let Some(cached) = self.request.cached_record(cache_key)? {
            println!("Cache HIT! Using cached result: {}", cached.id);
            output::print_result_summary(&cached);
            return Ok(Some(cached));
        }

        if self.request.should_dry_run() {
            return handle_dry_run(
                self.request.scenario,
                self.request.tool,
                self.request.model,
                cache_key,
            )
            .map(Some);
        }

        Ok(None)
    }

    fn execute_prepared(
        &self,
        context: PreparedRunContext,
        cache_key: CacheKey,
    ) -> anyhow::Result<ResultRecord> {
        let adapter = create_adapter_and_check(self.request.tool)?;
        let prepared =
            prepare_writer_and_setup(&context, self.request.scenario, self.plan.effective_timeout)?;

        let evaluation = run_evaluation_flow(EvaluationFlowInput {
            adapter: adapter.as_ref(),
            scenario: self.request.scenario,
            env: &context.workspace.env,
            tool: self.request.tool,
            model: self.request.model,
            effective_timeout: self.plan.effective_timeout,
            no_judge: self.request.no_judge,
            judge_model: self.request.judge_model,
            judge_tool: self.request.judge_tool,
            writer: &prepared.writer,
            artifacts: &prepared.artifacts,
            target_env: &context.target_env,
        })?;

        self.finalize_prepared_run(prepared, evaluation, cache_key)
    }

    fn finalize_prepared_run(
        &self,
        prepared: PreparedScenarioRun,
        evaluation: crate::run::execution::EvaluationFlowResult,
        cache_key: CacheKey,
    ) -> anyhow::Result<ResultRecord> {
        let outcome = determine_outcome(&evaluation.metrics);

        write_transcript_files(TranscriptFilesInput {
            writer: &prepared.writer,
            scenario: self.request.scenario,
            tool: self.request.tool,
            model: self.request.model,
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
            scenario: self.request.scenario,
            tool: self.request.tool,
            model: self.request.model,
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
            self.request.results_db,
            self.request.cache,
            &cache_key,
            &record,
            &self.plan.results_dir,
            prepared.setup_success,
        )
    }
}
