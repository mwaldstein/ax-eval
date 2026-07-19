use crate::adapter::registry::{AdapterRegistry, CheckedAdapter};
use crate::output;
use crate::results::{CacheKey, ResultRecord};
use crate::run::execution::{determine_outcome, run_attempt, RunAttemptInput};
use crate::run::records::{
    finalize_execution, handle_dry_run, ResultRecordInput, SetupFailureRecordInput,
};
use crate::run::setup::{prepare_writer_and_setup, PreparedRunContext, PreparedScenarioRun};
use crate::run::status::setup_failure_outcome;
use crate::run::transcript::{write_transcript_files, TranscriptFilesInput};
use crate::run::{ScenarioRunPlan, ScenarioRunRequest};
use crate::transcript::types::{EfficiencyReport, SetupCommandResult};
use crate::transcript::{RunMetadata, RunReport};

pub struct ScenarioRunLifecycle<'a> {
    request: ScenarioRunRequest<'a>,
    plan: ScenarioRunPlan,
    adapter: Option<&'a CheckedAdapter>,
}

impl<'a> ScenarioRunLifecycle<'a> {
    pub fn new(request: ScenarioRunRequest<'a>) -> Self {
        let plan = request.plan();
        Self {
            request,
            plan,
            adapter: None,
        }
    }

    pub fn new_with_adapter(request: ScenarioRunRequest<'a>, adapter: &'a CheckedAdapter) -> Self {
        let plan = request.plan();
        Self {
            request,
            plan,
            adapter: Some(adapter),
        }
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
        let owned_adapter;
        let mut adapter_registry;
        let checked_adapter = if let Some(adapter) = self.adapter {
            if adapter.tool() != self.request.tool {
                anyhow::bail!(
                    "Prechecked adapter tool mismatch: request uses {}, adapter is {}",
                    self.request.tool,
                    adapter.tool()
                );
            }
            adapter
        } else {
            adapter_registry = AdapterRegistry::new();
            owned_adapter = adapter_registry.resolve_checked(self.request.tool)?;
            &owned_adapter
        };
        let setup_started = std::time::Instant::now();
        let prepared =
            prepare_writer_and_setup(&context, self.request.scenario, self.plan.effective_timeout)?;

        if !prepared.setup_success {
            return self.finalize_setup_failure(prepared, cache_key, setup_started.elapsed());
        }

        let evaluation = run_attempt(RunAttemptInput {
            adapter: checked_adapter.adapter(),
            scenario: self.request.scenario,
            scenario_path: self.request.scenario_path,
            context: &context,
            prepared: &prepared,
            tool: self.request.tool,
            model: self.request.model,
            effective_timeout: self.plan.effective_timeout,
            no_judge: self.request.no_judge,
            judge_model: self.request.judge_model,
            judge_tool: self.request.judge_tool,
        })?;

        self.finalize_prepared_run(prepared, evaluation, cache_key)
    }

    fn finalize_setup_failure(
        &self,
        prepared: PreparedScenarioRun,
        cache_key: CacheKey,
        duration: std::time::Duration,
    ) -> anyhow::Result<ResultRecord> {
        let outcome = setup_failure_outcome(&prepared.setup_commands);
        let timestamp = chrono::Utc::now().to_rfc3339();
        let duration_secs = duration.as_secs_f64();

        prepared.writer.write_run_metadata(&RunMetadata {
            scenario_id: self.request.scenario.name.clone(),
            scenario_hash: cache_key.scenario_hash.clone(),
            tool: self.request.tool.to_string(),
            model: self.request.model.to_string(),
            timestamp: timestamp.clone(),
            duration_secs,
            cost_estimate_usd: None,
            token_usage: None,
        })?;
        prepared.writer.write_report(&RunReport {
            scenario_id: self.request.scenario.name.clone(),
            tool: self.request.tool.to_string(),
            model: self.request.model.to_string(),
            timestamp,
            duration_secs,
            cost_usd: None,
            token_usage: None,
            gate_status: crate::evaluation::GateStatus::NotConfigured,
            judge_score: None,
            judge_passed: None,
            judge_threshold: None,
            judge_error: None,
            composite_score: None,
            gate_details: vec![],
            efficiency: EfficiencyReport {
                total_commands: 0,
                unique_commands: 0,
                error_count: 0,
                tool_reuse_count: 0,
                help_invocations: 0,
                first_try_success_rate: 0.0,
                iteration_ratio: 0.0,
                completed: false,
            },
            setup_success: false,
            setup_commands: prepared
                .setup_commands
                .iter()
                .map(|command| SetupCommandResult {
                    command: command.command.clone(),
                    success: command.success,
                    output: command.output.clone(),
                })
                .collect(),
        })?;

        let transcript_path = prepared
            .artifacts
            .artifacts_dir()
            .to_string_lossy()
            .to_string();
        let record = SetupFailureRecordInput {
            scenario: self.request.scenario,
            tool: self.request.tool,
            model: self.request.model,
            cache_key: &cache_key,
            outcome,
            duration_secs,
            transcript_path,
        }
        .build();

        finalize_execution(
            self.request.results_db,
            self.request.cache,
            &cache_key,
            &record,
            &self.plan.results_dir,
            false,
            self.request.use_cache,
        )
    }

    fn finalize_prepared_run(
        &self,
        prepared: PreparedScenarioRun,
        evaluation: crate::run::execution::RunAttemptResult,
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
            self.request.use_cache,
        )
    }
}
