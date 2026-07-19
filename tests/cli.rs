mod support;

use crate::support::ax_eval;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

const SNAPSHOTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/ax-eval-fixtures/cli-help-snapshots"
);

fn assert_help_snapshot(subcommand: Option<&str>, snapshot_name: &str) {
    let mut cmd = ax_eval();
    if let Some(sc) = subcommand {
        cmd.args([sc, "--help"]);
    } else {
        cmd.arg("--help");
    }
    let output = cmd.env_remove("AX_EVAL_ENABLED").output().unwrap();
    let actual = String::from_utf8_lossy(&output.stdout).into_owned();
    let expected = fs::read_to_string(format!("{SNAPSHOTS}/{snapshot_name}.txt"))
        .unwrap_or_else(|_| panic!("snapshot file not found: {SNAPSHOTS}/{snapshot_name}.txt"));
    if actual != expected {
        let diff = similar::TextDiff::from_lines(&expected, &actual);
        let mut diff_lines = Vec::new();
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            diff_lines.push(format!("{sign}{change}"));
        }
        panic!(
            "CLI help for '{snapshot_name}' has drifted from its snapshot.\n\
             \n\
             Help text in src/cli.rs no longer matches the committed fixture.\n\
             If this change is intentional:\n\
               1. Run: scripts/generate-cli-reference.sh\n\
               2. Commit the updated ax-eval-fixtures/cli-help-snapshots/ and docs/reference/cli-commands.md\n\
             \n\
             Diff (expected vs actual):\n\
             {}\n",
            diff_lines.join("")
        );
    }
}

fn create_qipu_template(root: &std::path::Path) {
    let templates_dir = root.join("ax-eval-fixtures/templates/qipu");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(templates_dir.join("README.md"), "fixture").unwrap();
}

#[test]
fn test_cli_help() {
    ax_eval()
        .arg("--help")
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_cli_without_args_prints_guidance() {
    ax_eval()
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Use it to improve self-documenting tool surfaces",
        ))
        .stdout(predicate::str::contains("ax-eval template scenario"))
        .stdout(predicate::str::contains("AX_EVAL_ENABLED=1"));
}

#[test]
fn test_run_help_includes_judge_tool_option() {
    ax_eval()
        .args(["run", "--help"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("--judge-tool"));
}

#[test]
fn test_run_help_documents_safety_and_examples() {
    ax_eval()
        .args(["run", "--help"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("AX_EVAL_ENABLED=1"))
        .stdout(predicate::str::contains(
            "ax-eval run --scenario ax-eval-fixtures/my_scenario.yaml --tool opencode",
        ))
        .stdout(predicate::str::contains("Artifacts are written"));
}

#[test]
fn test_discover_help_documents_agent_options() {
    ax_eval()
        .args(["discover", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Usage: ax-eval discover [OPTIONS] <TARGET>",
        ))
        .stdout(predicate::str::contains("--discover-tool"))
        .stdout(predicate::str::contains("--discover-model"))
        .stdout(predicate::str::contains("--judge-tool"));
}

#[test]
fn test_discover_requires_safety_env_var() {
    ax_eval()
        .args(["discover", "qipu", "--tool", "mock"])
        .env_remove("AX_EVAL_ENABLED")
        .assert()
        .failure()
        .stderr(predicate::str::contains("AX_EVAL_ENABLED=1"));
}

#[test]
fn test_cli_help_points_to_template_command() {
    ax_eval()
        .arg("--help")
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("template"));
}

#[test]
fn test_template_scenario_outputs_copyable_schema() {
    ax_eval()
        .args(["template", "scenario"])
        .env("AX_EVAL_ENABLED", "1")
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
    ax_eval()
        .args(["template", "config"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("fixtures_path"))
        .stdout(predicate::str::contains("[tools.opencode]"))
        .stdout(predicate::str::contains("[profiles.quick]"))
        .stdout(predicate::str::contains("[models.").not());
}

#[test]
fn test_template_script_gate_outputs_json_contract() {
    ax_eval()
        .args(["template", "script-gate"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\""))
        .stdout(predicate::str::contains("\"message\""));
}

#[test]
fn test_guidance_list_outputs_topics() {
    ax_eval()
        .args(["guidance", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("typed-errors"))
        .stdout(predicate::str::contains("Related:"));
}

#[test]
fn test_guidance_topic_shortcut_outputs_topic_body() {
    ax_eval()
        .args(["guidance", "start"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Start Here"))
        .stdout(predicate::str::contains("Related topics:"));
}

#[test]
fn test_guidance_commands_take_priority_over_topic_shortcut() {
    ax_eval()
        .args(["guidance", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Available guidance topics"))
        .stdout(predicate::str::contains("# Start Here").not());
}

#[test]
fn test_guidance_show_start_outputs_capsule_index() {
    ax_eval()
        .args(["guidance", "show", "start"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Start Here"))
        .stdout(predicate::str::contains("test-usage"))
        .stdout(predicate::str::contains("workflow-commands"))
        .stdout(predicate::str::contains("structured-output"))
        .stdout(predicate::str::contains("Related topics:"));
}

#[test]
fn test_guidance_show_test_usage_outputs_usage_quality_principle() {
    ax_eval()
        .args(["guidance", "show", "test-usage"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Usage Quality"))
        .stdout(predicate::str::contains(
            "give the agent a goal, not a command recipe",
        ))
        .stdout(predicate::str::contains("discoverability"))
        .stdout(predicate::str::contains(
            "Use gates as fail-fast guardrails",
        ));
}

#[test]
fn test_guidance_show_outputs_topic_body() {
    ax_eval()
        .args(["guidance", "show", "agent-instructions"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Agent Instructions"))
        .stdout(predicate::str::contains("AGENTS.md"))
        .stdout(predicate::str::contains("Related topics:"));
}

#[test]
fn test_cli_version() {
    ax_eval()
        .arg("--version")
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("ax-eval"));
}

#[test]
fn test_run_command_requires_env_var() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args(["run", "--scenario", "ax-eval-fixtures/qipu/test_basic.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("AX_EVAL_ENABLED"))
        .stderr(predicate::str::contains("--dry-run"))
        .stderr(predicate::str::contains("explicit safety consent"));
}

#[test]
fn test_run_command_with_all_flag_requires_scenarios() {
    let dir = tempdir().unwrap();
    ax_eval()
        .current_dir(dir.path())
        .args(["run", "--all"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_scenarios_command_no_fixtures() {
    let dir = tempdir().unwrap();
    ax_eval()
        .current_dir(dir.path())
        .args(["scenarios"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available scenarios"));
}

#[test]
fn test_scenarios_command_with_fixtures() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args(["scenarios"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("test_scenario"))
        .stdout(predicate::str::contains("A test scenario"));
}

#[test]
fn test_scenarios_command_with_tags_filter() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args(["scenarios", "--tags", "smoke"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("scenario1"))
        .stdout(predicate::str::contains("[smoke]"))
        .stdout(predicate::str::contains("scenario2").not());
}

#[test]
fn test_scenarios_command_with_tags_filter_matches_any_tag() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args(["scenarios", "--tags", "smoke", "--tags", "integration"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("smoke_scenario"))
        .stdout(predicate::str::contains("integration_scenario"))
        .stdout(predicate::str::contains("docs_scenario").not());
}

#[test]
fn test_run_command_dry_run() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/dry_run_test.yaml",
            "--dry-run",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_dry_run_does_not_require_safety_env_var() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: dry_run_without_env_test
description: "Dry run without real LLM consent"
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
    fs::write(
        qipu_dir.join("dry_run_without_env_test.yaml"),
        scenario_content,
    )
    .unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/dry_run_without_env_test.yaml",
            "--dry-run",
        ])
        .env_remove("AX_EVAL_ENABLED")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_tags() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args(["run", "--all", "--tags", "smoke", "--tool", "mock"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_tags_matches_any_tag() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
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
        .env("AX_EVAL_ENABLED", "1")
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

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/tool_test.yaml",
            "--tool",
            "mock",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_single_combination_reports_run_errors() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/single_error_test.yaml",
            "--tool",
            "mock",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Run failed for mock / default"))
        .stderr(predicate::str::contains("target health check failed"));
}

#[test]
fn test_run_command_with_model_option() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/model_test.yaml",
            "--tool",
            "mock",
            "--model",
            "test-model",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_tier_filter() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args(["run", "--all", "--tier", "0", "--tool", "mock"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_timeout() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/timeout_test.yaml",
            "--tool",
            "mock",
            "--timeout-secs",
            "60",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_with_cache_flag() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    let qipu_dir = fixtures_dir.join("qipu");
    fs::create_dir_all(&qipu_dir).unwrap();
    create_qipu_template(dir.path());

    let scenario_content = r#"
name: cache_flag_test
description: "Cache flag test"
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
    fs::write(qipu_dir.join("cache_flag_test.yaml"), scenario_content).unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/cache_flag_test.yaml",
            "--tool",
            "mock",
            "--cache",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_run_command_matrix_multiple_tools() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/matrix_test.yaml",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Matrix run"));
}

#[test]
fn test_run_command_matrix_reports_errors_and_fails() {
    let dir = tempdir().unwrap();

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/matrix_error_test.yaml",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Evaluation Profile Summary"))
        .stdout(predicate::str::contains("Judge"))
        .stdout(predicate::str::contains(
            "run error: target health check failed",
        ))
        .stderr(predicate::str::contains("2 scenario run(s) failed"));
}

#[test]
fn test_clean_command_with_older_than() {
    let dir = tempdir().unwrap();
    ax_eval()
        .current_dir(dir.path())
        .args(["clean", "--older-than", "7d"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleaned 0 cache file(s)"));
}

#[test]
fn test_clean_command_with_older_than_keeps_new_cache_files() {
    let dir = tempdir().unwrap();
    let cache_dir = dir.path().join("ax-eval-results").join("cache");
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

    ax_eval()
        .current_dir(dir.path())
        .args(["clean", "--older-than", "1h"])
        .env("AX_EVAL_ENABLED", "1")
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
    ax_eval()
        .current_dir(dir.path())
        .args(["clean", "--older-than", "invalid"])
        .env("AX_EVAL_ENABLED", "1")
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

    let fixtures_dir = dir.path().join("ax-eval-fixtures");
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
    let templates_dir = dir.path().join("ax-eval-fixtures/templates/qipu");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(templates_dir.join("test.txt"), "test content").unwrap();

    // Copy scenario to the expected location for setup_scenario_env
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::write(fixtures_dir.join("post_script_test.yaml"), scenario_content).unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/qipu/post_script_test.yaml",
            "--tool",
            "mock",
        ])
        .env("AX_EVAL_ENABLED", "1")
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

#[test]
fn snapshot_help_root() {
    assert_help_snapshot(None, "ax-eval");
}

#[test]
fn snapshot_help_run() {
    assert_help_snapshot(Some("run"), "run");
}

#[test]
fn snapshot_help_discover() {
    assert_help_snapshot(Some("discover"), "discover");
}

#[test]
fn snapshot_help_scenarios() {
    assert_help_snapshot(Some("scenarios"), "scenarios");
}

#[test]
fn snapshot_help_show() {
    assert_help_snapshot(Some("show"), "show");
}

#[test]
fn snapshot_help_clean() {
    assert_help_snapshot(Some("clean"), "clean");
}

#[test]
fn snapshot_help_validate() {
    assert_help_snapshot(Some("validate"), "validate");
}

#[test]
fn test_validate_valid_scenario() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let scenario_content = r#"
name: test_scenario
description: "A valid scenario"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario_path = fixtures_dir.join("test_scenario.yaml");
    fs::write(&scenario_path, scenario_content).unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args([
            "validate",
            "--scenario",
            "ax-eval-fixtures/test_scenario.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("test_scenario"))
        .stdout(predicate::str::contains("Validated 1 scenario(s)"));
}

#[test]
fn test_validate_invalid_yaml() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let scenario_content = r#"
name: test_bad
description: "Missing target"
template_folder: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario_path = fixtures_dir.join("bad.yaml");
    fs::write(&scenario_path, scenario_content).unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--scenario", "ax-eval-fixtures/bad.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing required field"));
}

#[test]
fn test_validate_unknown_gate_type_suggests_correction() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let scenario_content = r#"
name: typo_gate
description: "Gate type typo"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: file_exits
      path: out.txt
"#;
    let scenario_path = fixtures_dir.join("typo.yaml");
    fs::write(&scenario_path, scenario_content).unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--scenario", "ax-eval-fixtures/typo.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("did you mean file_exists"));
}

#[test]
fn test_validate_requires_scenario_or_all() {
    ax_eval()
        .args(["validate"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--scenario").or(predicate::str::contains("--all")));
}

#[test]
fn test_validate_all_with_multiple_scenarios() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let valid = r#"
name: valid_one
description: "Valid"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let invalid = r#"
name: invalid_one
description: "Missing target"
task:
  prompt: "Do something"
evaluation:
  gates: []
"#;
    fs::write(fixtures_dir.join("valid.yaml"), valid).unwrap();
    fs::write(fixtures_dir.join("invalid.yaml"), invalid).unwrap();

    let result = ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--all"])
        .output()
        .unwrap();

    assert!(!result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stdout.contains("valid_one") || stderr.contains("valid_one"),
        "should mention valid_one"
    );
    assert!(
        stderr.contains("invalid_one") || stderr.contains("missing required field"),
        "should report invalid_one error"
    );
    assert!(
        stdout.contains("Validated 2 scenario(s)"),
        "should count both scenarios"
    );
}

#[test]
fn test_validate_all_skips_non_scenarios() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let scenario = r#"
name: real_scenario
description: "A real scenario"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    fs::write(fixtures_dir.join("scenario.yaml"), scenario).unwrap();
    fs::write(
        fixtures_dir.join("rubric.yaml"),
        "criteria: []\npass_threshold: 0.7\n",
    )
    .unwrap();

    let result = ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--all"])
        .output()
        .unwrap();

    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stdout.contains("real_scenario"),
        "should mention real_scenario, got stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("rubric"),
        "should not report rubric as error, got stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Validated 1 scenario(s)"),
        "should count only 1 scenario, got stdout:\n{stdout}"
    );
}

#[test]
fn test_validate_all_reports_malformed_scenarios() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    // malformed scenario: has 2 scenario keys (name, target) but missing required fields
    let malformed = r#"
name: malformed_scenario
description: "Missing task and evaluation"
template_folder: qipu
target:
  binary: qipu
"#;
    fs::write(fixtures_dir.join("malformed.yaml"), malformed).unwrap();

    let result = ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--all"])
        .output()
        .unwrap();

    assert!(
        !result.status.success(),
        "should fail because malformed scenario has validation errors"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("missing required field"),
        "should report missing required field for malformed scenario, got stderr:\n{stderr}"
    );
}

#[test]
fn test_validate_explicit_path_always_validates_even_non_scenarios() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    // A rubric file is clearly not a scenario, but if passed explicitly,
    // validate should try anyway and report the error.
    fs::write(
        fixtures_dir.join("rubric.yaml"),
        "criteria: []\npass_threshold: 0.7\n",
    )
    .unwrap();

    let result = ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--scenario", "ax-eval-fixtures/rubric.yaml"])
        .output()
        .unwrap();

    assert!(
        !result.status.success(),
        "should fail because explicit non-scenario file is still validated"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("missing required field"),
        "should report missing required field for explicit non-scenario, got stderr:\n{stderr}"
    );
}

#[test]
fn test_validate_nonexistent_file() {
    ax_eval()
        .args([
            "validate",
            "--scenario",
            "ax-eval-fixtures/nonexistent.yaml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read"));
}

#[test]
fn test_validate_warnings_exit_zero() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let scenario = r#"
name: warn_me
description: "Has warnings"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "   "
evaluation:
  gates: []
"#;
    fs::write(fixtures_dir.join("warn.yaml"), scenario).unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--scenario", "ax-eval-fixtures/warn.yaml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("warn_me"))
        .stdout(predicate::str::contains("warning(s)"));
}

#[test]
fn test_validate_all_empty_fixtures() {
    let dir = tempdir().unwrap();
    let fixtures_dir = dir.path().join("ax-eval-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    ax_eval()
        .current_dir(dir.path())
        .args(["validate", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No scenarios found"));
}

#[test]
fn snapshot_help_guidance() {
    assert_help_snapshot(Some("guidance"), "guidance");
}

#[test]
fn snapshot_help_template() {
    assert_help_snapshot(Some("template"), "template");
}
