use super::*;
use crate::scenario::Scenario;
use std::path::PathBuf;

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
    let scenario: Scenario = serde_yaml::from_str(scenario_yaml).unwrap();
    let base_dir = PathBuf::from("target/test_timeout");
    std::fs::create_dir_all(&base_dir).unwrap();

    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    let fixtures_dir = PathBuf::from("fixtures");
    std::fs::create_dir_all(&fixtures_dir).unwrap();
    let fixture_file = fixtures_dir.join("timeout_test_override.yaml");
    std::fs::write(&fixture_file, scenario_yaml).unwrap();

    let template_dir = PathBuf::from("fixtures/templates/qipu");
    std::fs::create_dir_all(&template_dir).unwrap();

    let cli_timeout = 300;
    let result = run_single_scenario(
        &scenario,
        &fixture_file,
        "mock",
        "mock",
        false,
        true,
        cli_timeout,
        false,
        None,
        None,
        &base_dir,
        &results_db,
        &cache,
    );

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
    let scenario: Scenario = serde_yaml::from_str(scenario_yaml).unwrap();
    let base_dir = PathBuf::from("target/test_timeout");
    std::fs::create_dir_all(&base_dir).unwrap();

    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    let fixtures_dir = PathBuf::from("fixtures");
    std::fs::create_dir_all(&fixtures_dir).unwrap();
    let fixture_file = fixtures_dir.join("timeout_test_cli.yaml");
    std::fs::write(&fixture_file, scenario_yaml).unwrap();

    let template_dir = PathBuf::from("fixtures/templates/qipu");
    std::fs::create_dir_all(&template_dir).unwrap();

    let cli_timeout = 60;
    let result = run_single_scenario(
        &scenario,
        &fixture_file,
        "mock",
        "mock",
        false,
        true,
        cli_timeout,
        false,
        None,
        None,
        &base_dir,
        &results_db,
        &cache,
    );

    let _ = std::fs::remove_file(&fixture_file);
    let _ = std::fs::remove_dir_all(&template_dir);

    assert!(
        result.is_ok(),
        "Should succeed with mock adapter: {:?}",
        result
    );
}
