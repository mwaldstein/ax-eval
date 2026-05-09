mod support;

use crate::support::llm_tool_test;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn create_qipu_template(root: &std::path::Path) {
    let templates_dir = root.join("fixtures/templates/qipu");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(templates_dir.join("README.md"), "fixture").unwrap();
}

#[test]
fn test_cli_help() {
    llm_tool_test()
        .arg("--help")
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_cli_without_args_prints_guidance() {
    llm_tool_test()
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Use it to improve CLI help, docs, and AGENTS.md guidance",
        ))
        .stdout(predicate::str::contains("llm-tool-test template scenario"))
        .stdout(predicate::str::contains("LLM_TOOL_TEST_ENABLED=1"));
}

#[test]
fn test_run_help_includes_judge_tool_option() {
    llm_tool_test()
        .args(["run", "--help"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("--judge-tool"));
}

#[test]
fn test_cli_help_points_to_template_command() {
    llm_tool_test()
        .arg("--help")
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("template"));
}

#[test]
fn test_template_scenario_outputs_copyable_schema() {
    llm_tool_test()
        .args(["template", "scenario"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("template_folder:"))
        .stdout(predicate::str::contains("target:"))
        .stdout(predicate::str::contains("task:"))
        .stdout(predicate::str::contains("evaluation:"))
        .stdout(predicate::str::contains("substring:"));
}

#[test]
fn test_template_config_outputs_supported_config_shape() {
    llm_tool_test()
        .args(["template", "config"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("fixtures_path"))
        .stdout(predicate::str::contains("[tools.opencode]"))
        .stdout(predicate::str::contains("[profiles.quick]"))
        .stdout(predicate::str::contains("[models.").not());
}

#[test]
fn test_template_script_gate_outputs_json_contract() {
    llm_tool_test()
        .args(["template", "script-gate"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\""))
        .stdout(predicate::str::contains("\"message\""));
}

#[test]
fn test_cli_version() {
    llm_tool_test()
        .arg("--version")
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("llm-tool-test"));
}

#[test]
fn test_run_command_requires_env_var() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: test_basic
description: "Basic test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("test_basic.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["run", "--scenario", "fixtures/qipu/test_basic.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("LLM_TOOL_TEST_ENABLED"));
}

#[test]
fn test_run_command_with_all_flag_requires_scenarios() {
    let dir = tempdir().unwrap();
    llm_tool_test()
        .current_dir(dir.path())
        .args(["run", "--all"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_scenarios_command_no_fixtures() {
    let dir = tempdir().unwrap();
    llm_tool_test()
        .current_dir(dir.path())
        .args(["scenarios"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available scenarios"));
}

#[test]
fn test_scenarios_command_with_fixtures() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let scenario_content = r#"
name: test_scenario
description: "A test scenario"
tier: 0
tags:
  - test
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(fixtures_dir.join("test_scenario.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["scenarios"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("test_scenario"))
        .stdout(predicate::str::contains("A test scenario"));
}

#[test]
fn test_scenarios_command_with_tags_filter() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let scenario1_content = r#"
name: scenario1
description: "First scenario"
tier: 0
tags:
  - smoke
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario2_content = r#"
name: scenario2
description: "Second scenario"
tier: 0
tags:
  - integration
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;

    fs::write(fixtures_dir.join("scenario1.yaml"), scenario1_content).unwrap();
    fs::write(fixtures_dir.join("scenario2.yaml"), scenario2_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["scenarios", "--tags", "smoke"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("scenario1"))
        .stdout(predicate::str::contains("[smoke]"))
        .stdout(predicate::str::contains("scenario2").not());
}

#[test]
fn test_scenarios_command_with_tags_filter_matches_any_tag() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let smoke_scenario = r#"
name: smoke_scenario
description: "Smoke scenario"
tier: 0
tags:
  - smoke
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let integration_scenario = r#"
name: integration_scenario
description: "Integration scenario"
tier: 0
tags:
  - integration
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let docs_scenario = r#"
name: docs_scenario
description: "Docs scenario"
tier: 0
tags:
  - docs
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;

    fs::write(fixtures_dir.join("smoke_scenario.yaml"), smoke_scenario).unwrap();
    fs::write(
        fixtures_dir.join("integration_scenario.yaml"),
        integration_scenario,
    )
    .unwrap();
    fs::write(fixtures_dir.join("docs_scenario.yaml"), docs_scenario).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["scenarios", "--tags", "smoke", "--tags", "integration"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("smoke_scenario"))
        .stdout(predicate::str::contains("integration_scenario"))
        .stdout(predicate::str::contains("docs_scenario").not());
}

#[test]
fn test_run_command_dry_run() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: dry_run_test
description: "Dry run test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("dry_run_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "fixtures/qipu/dry_run_test.yaml",
            "--dry-run",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_tags() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario1_content = r#"
name: tagged_scenario
description: "Tagged scenario"
tier: 0
tags:
  - smoke
  - quick
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario2_content = r#"
name: untagged_scenario
description: "Untagged scenario"
tier: 0
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;

    fs::write(qipu_dir.join("tagged_scenario.yaml"), scenario1_content).unwrap();
    fs::write(qipu_dir.join("untagged_scenario.yaml"), scenario2_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["run", "--all", "--tags", "smoke", "--tool", "mock"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_tags_matches_any_tag() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let smoke_scenario = r#"
name: smoke_run_scenario
description: "Smoke run scenario"
tier: 0
tags:
  - smoke
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let integration_scenario = r#"
name: integration_run_scenario
description: "Integration run scenario"
tier: 0
tags:
  - integration
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let docs_scenario = r#"
name: docs_run_scenario
description: "Docs run scenario"
tier: 0
tags:
  - docs
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;

    fs::write(qipu_dir.join("smoke_run_scenario.yaml"), smoke_scenario).unwrap();
    fs::write(
        qipu_dir.join("integration_run_scenario.yaml"),
        integration_scenario,
    )
    .unwrap();
    fs::write(qipu_dir.join("docs_run_scenario.yaml"), docs_scenario).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--all",
            "--tags",
            "smoke",
            "--tags",
            "integration",
            "--dry-run",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Loaded scenario: smoke_run_scenario",
        ))
        .stdout(predicate::str::contains(
            "Loaded scenario: integration_run_scenario",
        ))
        .stdout(predicate::str::contains("Loaded scenario: docs_run_scenario").not());
}

#[test]
fn test_run_command_with_tool_option() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: tool_test
description: "Tool option test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("tool_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "fixtures/qipu/tool_test.yaml",
            "--tool",
            "mock",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_single_combination_reports_run_errors() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: single_error_test
description: "Single run errors should fail the command"
template_folder: qipu
target:
  binary: qipu
  health_check: "false"
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("single_error_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "fixtures/qipu/single_error_test.yaml",
            "--tool",
            "mock",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Run failed for mock / default"))
        .stderr(predicate::str::contains("target health check failed"));
}

#[test]
fn test_run_command_with_model_option() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: model_test
description: "Model option test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("model_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "fixtures/qipu/model_test.yaml",
            "--tool",
            "mock",
            "--model",
            "test-model",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_tier_filter() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario1_content = r#"
name: tier0_scenario
description: "Tier 0 scenario"
tier: 0
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario2_content = r#"
name: tier1_scenario
description: "Tier 1 scenario"
tier: 1
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;

    fs::write(qipu_dir.join("tier0_scenario.yaml"), scenario1_content).unwrap();
    fs::write(qipu_dir.join("tier1_scenario.yaml"), scenario2_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["run", "--all", "--tier", "0", "--tool", "mock"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_timeout() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: timeout_test
description: "Timeout test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("timeout_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "fixtures/qipu/timeout_test.yaml",
            "--tool",
            "mock",
            "--timeout-secs",
            "60",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_no_cache() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: no_cache_test
description: "No cache test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("no_cache_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "fixtures/qipu/no_cache_test.yaml",
            "--tool",
            "mock",
            "--no-cache",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_matrix_multiple_tools() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: matrix_test
description: "Matrix run test"
tool_matrix:
  - tool: mock
    models:
      - model1
      - model2
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("matrix_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["run", "--scenario", "fixtures/qipu/matrix_test.yaml"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Matrix run"));
}

#[test]
fn test_run_command_matrix_reports_errors_and_fails() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: matrix_error_test
description: "Matrix run errors should fail the command"
tool_matrix:
  - tool: mock
    models:
      - model1
      - model2
template_folder: qipu
target:
  binary: qipu
  health_check: "false"
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(qipu_dir.join("matrix_error_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args(["run", "--scenario", "fixtures/qipu/matrix_error_test.yaml"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Matrix Summary"))
        .stdout(predicate::str::contains(
            "Error: target health check failed",
        ))
        .stderr(predicate::str::contains("2 scenario run(s) failed"));
}

#[test]
fn test_clean_command_with_older_than() {
    let dir = tempdir().unwrap();
    llm_tool_test()
        .current_dir(dir.path())
        .args(["clean", "--older-than", "7d"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleaned 0 cache file(s)"));
}

#[test]
fn test_clean_command_with_older_than_keeps_new_cache_files() {
    let dir = tempdir().unwrap();
    let cache_dir = dir.path().join("llm-tool-test-results").join("cache");
    fs::create_dir_all(&cache_dir).unwrap();

    let old_cache_file = cache_dir.join("old-cache-entry");
    let new_cache_file = cache_dir.join("new-cache-entry");
    fs::write(&old_cache_file, "old").unwrap();
    fs::write(&new_cache_file, "new").unwrap();

    let status = Command::new("touch")
        .args(["-d", "2 hours ago"])
        .arg(&old_cache_file)
        .status()
        .unwrap();
    assert!(status.success());

    llm_tool_test()
        .current_dir(dir.path())
        .args(["clean", "--older-than", "1h"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Cleaned 1 cache file(s) older than 1h, kept 1",
        ));

    assert!(!old_cache_file.exists());
    assert!(new_cache_file.exists());
}

#[test]
fn test_clean_command_invalid_duration() {
    let dir = tempdir().unwrap();
    llm_tool_test()
        .current_dir(dir.path())
        .args(["clean", "--older-than", "invalid"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .failure();
}

// Helper function to recursively find a file in a directory
fn find_file_recursive(dir: &std::path::Path, filename: &str) -> Option<std::path::PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_file_recursive(&path, filename) {
                    return Some(found);
                }
            } else if path.file_name().map(|n| n == filename).unwrap_or(false) {
                return Some(path);
            }
        }
    }
    None
}

#[test]
fn test_run_command_with_post_scripts() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();

    // Create a scenario with post-execution scripts
    // Using a simpler approach - create a file in the fixture directory
    let scenario_content = r#"
name: post_script_test
description: "Post script execution test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
scripts:
  post:
    - command: "echo 'post_script_output' > post_script_marker.txt"
      timeout_secs: 10
evaluation:
  gates:
    - type: file_contains
      path: "post_script_marker.txt"
      substring: "post_script_output"
"#;
    fs::write(qipu_dir.join("post_script_test.yaml"), scenario_content).unwrap();

    // Create required template folder structure
    let templates_dir = dir.path().join("fixtures/templates/qipu");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(templates_dir.join("test.txt"), "test content").unwrap();

    // Copy scenario to the expected location for setup_scenario_env
    let fixtures_dir = dir.path().join("fixtures");
    fs::write(fixtures_dir.join("post_script_test.yaml"), scenario_content).unwrap();

    llm_tool_test()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "fixtures/qipu/post_script_test.yaml",
            "--tool",
            "mock",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();

    // Check that the post script created the marker file in the fixture directory
    // The fixture directory is within the temp dir
    // Actually, the marker file will be in the fixture subdirectory within the results
    // Let's search for it
    let found_file = find_file_recursive(dir.path(), "post_script_marker.txt");

    assert!(
        found_file.is_some(),
        "Post script should have created the marker file somewhere in {:?}",
        dir.path()
    );

    if let Some(ref path) = found_file {
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("post_script_output"));
    }
}
