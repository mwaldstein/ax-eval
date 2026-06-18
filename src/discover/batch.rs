use super::manifest::{display_path, safe_segment, DiscoveryRunManifest, UsageTotals};
use super::scenarios::ValidScenario;
use super::DiscoverRequest;
use crate::adapter::registry::AdapterRegistry;
use crate::run;
use std::path::Path;

pub(super) fn run_generated_scenarios(
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
                run_manifests.push(harness_error_manifest(valid, &result_dir, error));
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
                use_cache: false,
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
                run_manifests.push(harness_error_manifest(valid, &result_dir, error));
            }
        }
    }

    (run_manifests, usage)
}

fn harness_error_manifest(
    valid: &ValidScenario,
    result_dir: &Path,
    error: anyhow::Error,
) -> DiscoveryRunManifest {
    DiscoveryRunManifest {
        scenario_name: valid.scenario.name.clone(),
        scenario_path: display_path(&valid.path),
        result_dir: display_path(result_dir),
        status: "harness_error".to_string(),
        run_id: None,
        outcome: None,
        judge_score: None,
        judge_passed: None,
        error_count: 0,
        failed_call_count: 0,
        error: Some(format!("{error:#}")),
    }
}
