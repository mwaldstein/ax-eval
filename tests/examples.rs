//! End-to-end tests that run all example scenarios with the smart-mock adapter.
//!
//! These tests verify that the example fixtures and scenarios work correctly
//! as automated e2e tests of the framework itself. They do not require a real LLM.
//!
//! To run:
//!   cargo test --test examples

mod support;

use assert_cmd::Command;

fn llm_tool_test() -> Command {
    support::llm_tool_test()
}

#[test]
fn test_example_e2e_passes() {
    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_e2e.yaml",
        "--tool",
        "smart-mock",
        "--no-cache",
        "--no-judge",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("Outcome: Pass"),
        "example_e2e should pass all gates:\n{}",
        stdout
    );
}

#[test]
fn test_example_basic_passes() {
    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_basic.yaml",
        "--tool",
        "smart-mock",
        "--no-cache",
        "--no-judge",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("Outcome: Pass"),
        "example_basic should pass all gates:\n{}",
        stdout
    );
}

#[test]
fn test_example_gates_comprehensive_passes() {
    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_gates_comprehensive.yaml",
        "--tool",
        "smart-mock",
        "--no-cache",
        "--no-judge",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("Outcome: Pass"),
        "example_gates_comprehensive should pass all gates:\n{}",
        stdout
    );
}

#[test]
fn test_example_judge_passes_without_judge() {
    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_judge.yaml",
        "--tool",
        "smart-mock",
        "--no-cache",
        "--no-judge",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("Outcome: Pass"),
        "example_judge should pass gates without judge:\n{}",
        stdout
    );
}

#[test]
fn test_example_full_config_passes() {
    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_full_config.yaml",
        "--tool",
        "smart-mock",
        "--no-cache",
        "--no-judge",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("Outcome: Pass"),
        "example_full_config should pass all gates:\n{}",
        stdout
    );
    // Verify that --tool smart-mock took precedence over the scenario's tool_matrix
    assert!(
        stdout.contains("smart-mock"),
        "Should have run with smart-mock adapter:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("Matrix run:"),
        "Should not run a matrix when --tool is specified:\n{}",
        stdout
    );
}
