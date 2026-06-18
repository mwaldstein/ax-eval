use super::*;
use crate::adapter::registry::AdapterRegistry;
use crate::results::test_helpers::create_test_record;
use crate::scenario::Scenario;
use std::path::{Path, PathBuf};

fn write_temp_scenario(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(format!("{name}.yaml"));
    std::fs::write(&path, body).expect("write scenario");
    path
}

#[test]
fn scenario_run_request_resolves_effective_timeout() {
    let scenario_yaml = r#"
name: request_timeout_test
description: "Test scenario timeout resolution"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Create a note"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
run:
  timeout_secs: 120
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let scenario_path = PathBuf::from("ax-eval-fixtures/request_timeout_test.yaml");
    let base_dir = PathBuf::from("target/test_timeout");
    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    let request = ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock",
        dry_run: false,
        use_cache: false,
        timeout_secs: 300,
        no_judge: false,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    };

    assert_eq!(request.effective_timeout(), 120);

    let mut cli_timeout_scenario = scenario.clone();
    cli_timeout_scenario.run = None;
    let request = ScenarioRunRequest {
        scenario: &cli_timeout_scenario,
        ..request
    };

    assert_eq!(request.effective_timeout(), 300);
}

#[test]
fn scenario_run_request_resolves_results_dir() {
    let scenario_yaml = r#"
name: request_results_dir_test
description: "Test scenario results dir resolution"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Create a note"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let scenario_path = PathBuf::from("ax-eval-fixtures/request_results_dir_test.yaml");
    let base_dir = PathBuf::from("target/test_results_dir");
    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    let request = ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock-model",
        dry_run: false,
        use_cache: false,
        timeout_secs: 300,
        no_judge: false,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    };

    let results_dir = request.results_dir();
    let dir_name = results_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("results dir name");

    assert_eq!(
        results_dir.parent(),
        Some(std::path::Path::new("ax-eval-results"))
    );
    assert!(
        dir_name.ends_with("-mock-mock-model-request_results_dir_test"),
        "{}",
        dir_name
    );
    assert_eq!(dir_name.chars().nth(8), Some('-'));
    assert!(
        dir_name
            .chars()
            .take(15)
            .all(|ch| ch.is_ascii_digit() || ch == '-'),
        "{}",
        dir_name
    );
}

#[test]
fn scenario_run_request_builds_run_plan() {
    let scenario_yaml = r#"
name: request_plan_test
description: "Test scenario run planning"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Create a note"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
run:
  timeout_secs: 90
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let scenario_path = PathBuf::from("ax-eval-fixtures/request_plan_test.yaml");
    let base_dir = PathBuf::from("target/test_plan");
    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    let request = ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock-model",
        dry_run: false,
        use_cache: false,
        timeout_secs: 300,
        no_judge: false,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    };

    let plan = request.plan();

    assert_eq!(plan.effective_timeout, 90);
    assert_eq!(plan.results_dir, request.results_dir());
}

#[test]
fn scenario_run_request_respects_cache_policy() {
    let scenario_yaml = r#"
name: request_cache_policy_test
description: "Test scenario cache policy"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Create a note"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let scenario_path = PathBuf::from("ax-eval-fixtures/request_cache_policy_test.yaml");
    let base_dir = tempfile::tempdir().expect("base dir");
    let results_db = ResultsDB::new(base_dir.path());
    let cache = Cache::new(base_dir.path());
    let cache_key = crate::results::CacheKey::compute("scenario", "prompt", "mock", "mock");
    let cached = create_test_record("cached-run");
    cache.put(&cache_key, &cached).expect("cache record");

    let request = ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock",
        dry_run: false,
        use_cache: true,
        timeout_secs: 300,
        no_judge: false,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    };

    assert_eq!(
        request
            .cached_record(&cache_key)
            .expect("cache lookup")
            .unwrap()
            .id,
        "cached-run"
    );

    let request = ScenarioRunRequest {
        use_cache: false,
        ..request
    };

    assert!(request
        .cached_record(&cache_key)
        .expect("cache lookup")
        .is_none());
}

#[test]
fn scenario_run_request_identifies_dry_run_policy() {
    let scenario_yaml = r#"
name: request_dry_run_policy_test
description: "Test scenario dry-run policy"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Create a note"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let scenario_path = PathBuf::from("ax-eval-fixtures/request_dry_run_policy_test.yaml");
    let base_dir = tempfile::tempdir().expect("base dir");
    let results_db = ResultsDB::new(base_dir.path());
    let cache = Cache::new(base_dir.path());

    let request = ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock",
        dry_run: true,
        use_cache: false,
        timeout_secs: 300,
        no_judge: false,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    };

    assert!(request.should_dry_run());

    let request = ScenarioRunRequest {
        dry_run: false,
        ..request
    };

    assert!(!request.should_dry_run());
}

#[test]
fn test_scenario_timeout_overrides_cli() {
    let scenario_yaml = r#"
name: timeout_test_override
description: "Test scenario timeout overrides CLI"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Create a note"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
run:
  timeout_secs: 120
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let base_dir = PathBuf::from("target/test_timeout");
    std::fs::create_dir_all(&base_dir).unwrap();

    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    let fixtures_dir = PathBuf::from("ax-eval-fixtures");
    std::fs::create_dir_all(&fixtures_dir).unwrap();
    let fixture_file = fixtures_dir.join("timeout_test_override.yaml");
    std::fs::write(&fixture_file, scenario_yaml).unwrap();

    let template_dir = PathBuf::from("ax-eval-fixtures/templates/qipu");
    std::fs::create_dir_all(&template_dir).unwrap();

    let cli_timeout = 300;
    let result = run_single_scenario(ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &fixture_file,
        tool: "mock",
        model: "mock",
        dry_run: false,
        use_cache: false,
        timeout_secs: cli_timeout,
        no_judge: false,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    });

    let _ = std::fs::remove_file(&fixture_file);
    let _ = std::fs::remove_dir_all(&template_dir);

    assert!(
        result.is_ok(),
        "Should succeed with mock adapter: {:?}",
        result
    );
}

#[test]
fn test_cli_timeout_used_when_scenario_none() {
    let scenario_yaml = r#"
name: timeout_test_cli
description: "Test CLI timeout is used when scenario doesn't specify"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Create a note"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let base_dir = PathBuf::from("target/test_timeout");
    std::fs::create_dir_all(&base_dir).unwrap();

    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    let fixtures_dir = PathBuf::from("ax-eval-fixtures");
    std::fs::create_dir_all(&fixtures_dir).unwrap();
    let fixture_file = fixtures_dir.join("timeout_test_cli.yaml");
    std::fs::write(&fixture_file, scenario_yaml).unwrap();

    let template_dir = PathBuf::from("ax-eval-fixtures/templates/qipu");
    std::fs::create_dir_all(&template_dir).unwrap();

    let cli_timeout = 60;
    let result = run_single_scenario(ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &fixture_file,
        tool: "mock",
        model: "mock",
        dry_run: false,
        use_cache: false,
        timeout_secs: cli_timeout,
        no_judge: false,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    });

    let _ = std::fs::remove_file(&fixture_file);
    let _ = std::fs::remove_dir_all(&template_dir);

    assert!(
        result.is_ok(),
        "Should succeed with mock adapter: {:?}",
        result
    );
}

#[test]
fn scenario_run_lifecycle_dry_run_does_not_append_result() {
    let scenario_yaml = r#"
name: lifecycle_dry_run_test
description: Test dry-run lifecycle short-circuit
template_folder: example_basic
target:
  binary: taskmgr
task:
  prompt: "Create a task"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let dir = tempfile::tempdir().expect("tempdir");
    let scenario_path = write_temp_scenario(dir.path(), "lifecycle_dry_run_test", scenario_yaml);
    let results_db = ResultsDB::new(dir.path());
    let cache = Cache::new(dir.path());

    let record = run_single_scenario(ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock",
        dry_run: true,
        use_cache: false,
        timeout_secs: 300,
        no_judge: true,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    })
    .expect("dry run");

    assert_eq!(record.outcome, "Dry run");
    assert!(results_db
        .load_all()
        .expect("load result records")
        .is_empty());
}

#[test]
fn scenario_run_lifecycle_bubbles_evaluation_failure_without_record() {
    let scenario_yaml = r#"
name: lifecycle_evaluation_failure_test
description: Test lifecycle evaluation failure behavior
template_folder: example_basic
target:
  binary: taskmgr
interaction:
  target_commands: forbidden
task:
  prompt: "Create a task"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let dir = tempfile::tempdir().expect("tempdir");
    let scenario_path = write_temp_scenario(
        dir.path(),
        "lifecycle_evaluation_failure_test",
        scenario_yaml,
    );
    let results_db = ResultsDB::new(dir.path());
    let cache = Cache::new(dir.path());

    let result = run_single_scenario(ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock",
        dry_run: false,
        use_cache: false,
        timeout_secs: 300,
        no_judge: true,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    });

    assert!(result
        .unwrap_err()
        .to_string()
        .contains("target-tool events are forbidden"));
    assert!(results_db
        .load_all()
        .expect("load result records")
        .is_empty());
}

#[test]
fn scenario_run_lifecycle_accepts_prechecked_adapter() {
    let scenario_yaml = r#"
name: lifecycle_prechecked_adapter_test
description: Test lifecycle can reuse an adapter checked by an orchestrator
template_folder: example_basic
target:
  binary: taskmgr
task:
  prompt: "Create a task"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let dir = tempfile::tempdir().expect("tempdir");
    let scenario_path = write_temp_scenario(
        dir.path(),
        "lifecycle_prechecked_adapter_test",
        scenario_yaml,
    );
    let results_db = ResultsDB::new(dir.path());
    let cache = Cache::new(dir.path());
    let mut registry = AdapterRegistry::new();
    let adapter = registry.resolve_checked("mock").expect("checked adapter");

    let record = run_single_scenario_with_adapter(
        ScenarioRunRequest {
            scenario: &scenario,
            scenario_path: &scenario_path,
            tool: "mock",
            model: "mock",
            dry_run: false,
            use_cache: false,
            timeout_secs: 300,
            no_judge: true,
            judge_model: None,
            judge_tool: None,
            results_db: &results_db,
            cache: &cache,
            results_dir_override: None,
        },
        &adapter,
    )
    .expect("run with prechecked adapter");

    assert_eq!(record.outcome, "Pass");
    assert_eq!(results_db.load_all().expect("load result records").len(), 1);
}

#[test]
fn scenario_run_lifecycle_rejects_mismatched_prechecked_adapter() {
    let scenario_yaml = r#"
name: lifecycle_prechecked_adapter_mismatch_test
description: Test lifecycle rejects an adapter for a different tool
template_folder: example_basic
target:
  binary: taskmgr
task:
  prompt: "Create a task"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let dir = tempfile::tempdir().expect("tempdir");
    let scenario_path = write_temp_scenario(
        dir.path(),
        "lifecycle_prechecked_adapter_mismatch_test",
        scenario_yaml,
    );
    let results_db = ResultsDB::new(dir.path());
    let cache = Cache::new(dir.path());
    let mut registry = AdapterRegistry::new();
    let adapter = registry.resolve_checked("mock").expect("checked adapter");

    let result = run_single_scenario_with_adapter(
        ScenarioRunRequest {
            scenario: &scenario,
            scenario_path: &scenario_path,
            tool: "opencode",
            model: "mock",
            dry_run: false,
            use_cache: false,
            timeout_secs: 300,
            no_judge: true,
            judge_model: None,
            judge_tool: None,
            results_db: &results_db,
            cache: &cache,
            results_dir_override: None,
        },
        &adapter,
    );

    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Prechecked adapter tool mismatch"));
}

#[test]
fn scenario_run_lifecycle_finalizes_when_setup_fails() {
    let scenario_yaml = r#"
name: lifecycle_setup_failure_test
description: Test lifecycle setup failure behavior
template_folder: example_basic
target:
  binary: taskmgr
task:
  prompt: "Create a task"
setup:
  commands:
    - "exit 7"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
    let dir = tempfile::tempdir().expect("tempdir");
    let scenario_path =
        write_temp_scenario(dir.path(), "lifecycle_setup_failure_test", scenario_yaml);
    let results_db = ResultsDB::new(dir.path());
    let cache = Cache::new(dir.path());

    let record = run_single_scenario(ScenarioRunRequest {
        scenario: &scenario,
        scenario_path: &scenario_path,
        tool: "mock",
        model: "mock",
        dry_run: false,
        use_cache: false,
        timeout_secs: 300,
        no_judge: true,
        judge_model: None,
        judge_tool: None,
        results_db: &results_db,
        cache: &cache,
        results_dir_override: None,
    })
    .expect("run with failed setup");

    assert_eq!(record.outcome, "Pass");
    assert_eq!(results_db.load_all().expect("load result records").len(), 1);
}
