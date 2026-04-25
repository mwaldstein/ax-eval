//! End-to-end tests using real LLM tools.
//!
//! These tests verify the complete llm-tool-test flow with actual LLM adapters.
//! They are gated behind the `LLM_TOOL_TEST_E2E` environment variable and
//! only run when a supported LLM tool is installed and authenticated.
//!
//! To run:
//!   LLM_TOOL_TEST_ENABLED=1 LLM_TOOL_TEST_E2E=1 cargo test --test e2e
//!
//! Supported tools (auto-detected):
//!   - opencode
//!   - claude / claude-code

mod support;

use assert_cmd::Command;
use std::env;
use std::path::PathBuf;

/// Detect available LLM tools on the system.
fn detect_available_tools() -> Vec<String> {
    let mut tools = Vec::new();

    for cmd in &["opencode", "claude", "claude-code"] {
        if std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {}", cmd))
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            tools.push(cmd.to_string());
        }
    }

    tools
}

/// Check if e2e tests are enabled via environment variable.
fn e2e_enabled() -> bool {
    env::var("LLM_TOOL_TEST_E2E").is_ok_and(|v| v == "1")
}

fn llm_tool_test() -> Command {
    support::llm_tool_test()
}

#[test]
fn test_e2e_scenario_discovery() {
    llm_tool_test()
        .args(["scenarios", "--tags", "e2e"])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicates::str::contains("example_e2e"));
}

#[test]
fn test_e2e_dry_run() {
    llm_tool_test()
        .args([
            "run",
            "--scenario",
            "fixtures/example_e2e.yaml",
            "--dry-run",
        ])
        .env("LLM_TOOL_TEST_ENABLED", "1")
        .assert()
        .success();
}

#[test]
fn test_e2e_with_real_llm_opencode() {
    if !e2e_enabled() {
        eprintln!("Skipping e2e test: LLM_TOOL_TEST_E2E=1 not set");
        return;
    }

    let available = detect_available_tools();
    if !available.contains(&"opencode".to_string()) {
        eprintln!("Skipping e2e test: opencode not installed");
        return;
    }

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("opencode --version")
        .output()
        .expect("check opencode");
    assert!(
        output.status.success(),
        "opencode should be available and authenticated"
    );

    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_e2e.yaml",
        "--tool",
        "opencode",
        "--timeout-secs",
        "60",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(
        stdout.contains("Running tool 'opencode'"),
        "Should have executed opencode: {}",
        stdout
    );
}

#[test]
fn test_e2e_with_real_llm_claude() {
    if !e2e_enabled() {
        eprintln!("Skipping e2e test: LLM_TOOL_TEST_E2E=1 not set");
        return;
    }

    let available = detect_available_tools();
    let has_claude = available.contains(&"claude".to_string())
        || available.contains(&"claude-code".to_string());
    if !has_claude {
        eprintln!("Skipping e2e test: claude not installed");
        return;
    }

    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_e2e.yaml",
        "--tool",
        "claude",
        "--timeout-secs",
        "60",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(
        stdout.contains("Running tool 'claude'"),
        "Should have executed claude: {}",
        stdout
    );
}

#[test]
fn test_e2e_reports_results_artifacts() {
    if !e2e_enabled() {
        eprintln!("Skipping e2e test: LLM_TOOL_TEST_E2E=1 not set");
        return;
    }

    let available = detect_available_tools();
    let tool = available.first().cloned();
    if tool.is_none() {
        eprintln!("Skipping e2e test: no LLM tool available");
        return;
    }
    let tool = tool.unwrap();

    let results_dir = PathBuf::from("llm-tool-test-results");
    let _ = std::fs::remove_dir_all(&results_dir);

    let mut cmd = llm_tool_test();
    cmd.args([
        "run",
        "--scenario",
        "fixtures/example_e2e.yaml",
        "--tool",
        &tool,
        "--timeout-secs",
        "60",
    ])
    .env("LLM_TOOL_TEST_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    // Verify artifacts were produced
    assert!(
        stdout.contains("Artifacts written to:"),
        "Should report artifacts path: {}",
        stdout
    );

    // Check that results directory exists and has expected files
    let entries: Vec<_> = std::fs::read_dir(&results_dir)
        .expect("results dir should exist")
        .filter_map(|e| e.ok())
        .collect();

    assert!(!entries.is_empty(), "Results directory should not be empty");

    // Find a run directory and check for expected artifacts
    let run_dir = entries
        .iter()
        .find(|e| e.path().is_dir())
        .map(|e| e.path())
        .expect("should have a run subdirectory");

    assert!(
        run_dir.join("transcript.raw.txt").exists(),
        "transcript.raw.txt should exist"
    );
    assert!(
        run_dir.join("events.jsonl").exists(),
        "events.jsonl should exist"
    );
    assert!(
        run_dir.join("metrics.json").exists(),
        "metrics.json should exist"
    );
    assert!(
        run_dir.join("evaluation.md").exists(),
        "evaluation.md should exist"
    );
    assert!(
        run_dir.join("fixture").exists(),
        "fixture directory should exist"
    );

    let _ = std::fs::remove_dir_all(&results_dir);
}
