pub mod artifacts;
pub mod cache;
pub mod execution;
pub mod lifecycle;
pub mod records;
pub mod setup;
pub mod status;
pub mod transcript;
pub mod utils;

use crate::adapter::registry::CheckedAdapter;
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
    pub results_dir_override: Option<PathBuf>,
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
        self.results_dir_override.clone().unwrap_or_else(|| {
            crate::run::utils::get_results_dir(self.tool, self.model, &self.scenario.name)
        })
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
    lifecycle::ScenarioRunLifecycle::new(request).run()
}

pub fn run_single_scenario_with_adapter(
    request: ScenarioRunRequest<'_>,
    adapter: &CheckedAdapter,
) -> anyhow::Result<ResultRecord> {
    lifecycle::ScenarioRunLifecycle::new_with_adapter(request, adapter).run()
}

#[cfg(test)]
mod tests;
