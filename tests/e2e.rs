//! End-to-end tests using real LLM tools.
//!
//! These tests verify the complete ax-eval flow with actual LLM adapters.
//! They require an installed and authenticated LLM tool, so they do NOT run
//! automatically in standard test suites. They are gated behind the
//! `AX_EVAL_E2E` environment variable.
//!
//! For automated regression tests that run in CI, see `src/fixture_tests.rs`
//! which exercises every fixture scenario with mock tools (no LLM required).
//!
//! To run these real-LLM tests:
//!   AX_EVAL_ENABLED=1 AX_EVAL_E2E=1 cargo test --test e2e -- --ignored
//!
//! Supported tools (auto-detected):
//!   - opencode
//!   - claude / claude-code
//!   - codex
//!
//! Model selection:
//!   Set AX_EVAL_MODEL to override the default model. For opencode, the
//!   default is `opencode/kimi-k2.6`.

mod support;

use assert_cmd::Command;
use std::env;
use std::path::PathBuf;

/// Detect available LLM tools on the system.
fn detect_available_tools() -> Vec<String> {
    let mut tools = Vec::new();

    for cmd in &["opencode", "claude", "claude-code", "codex"] {
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
    env::var("AX_EVAL_E2E").is_ok_and(|v| v == "1")
}

fn ax_eval() -> Command {
    support::ax_eval()
}

/// Resolve model for a given tool, respecting AX_EVAL_MODEL env var.
fn resolve_model(tool: &str) -> String {
    env::var("AX_EVAL_MODEL").unwrap_or_else(|_| match tool {
        "opencode" => "opencode/kimi-k2.6".to_string(),
        "codex" => "gpt-5-codex".to_string(),
        _ => "default".to_string(),
    })
}

#[test]
fn test_e2e_scenario_discovery() {
    ax_eval()
        .args(["scenarios", "--tags", "examples"])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success()
        .stdout(predicates::str::contains("example_e2e"))
        .stdout(predicates::str::contains("example_basic"));
}

#[test]
fn test_e2e_dry_run() {
    ax_eval()
        .args([
            "run",
            "--scenario",
            "ax-eval-fixtures/example_e2e.yaml",
            "--dry-run",
        ])
        .env("AX_EVAL_ENABLED", "1")
        .assert()
        .success();
}

#[test]
#[ignore = "requires installed and authenticated opencode plus AX_EVAL_E2E=1"]
fn test_e2e_with_real_llm_opencode() {
    if !e2e_enabled() {
        eprintln!("Skipping e2e test: AX_EVAL_E2E=1 not set");
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

    let mut cmd = ax_eval();
    cmd.args([
        "run",
        "--scenario",
        "ax-eval-fixtures/example_e2e.yaml",
        "--tool",
        "opencode",
        "--model",
        &resolve_model("opencode"),
        "--timeout-secs",
        "60",
        "--cache",
    ])
    .env("AX_EVAL_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(
        stdout.contains("Running tool 'opencode'"),
        "Should have executed opencode: {}",
        stdout
    );
}

#[test]
#[ignore = "requires installed and authenticated claude/claude-code plus AX_EVAL_E2E=1"]
fn test_e2e_with_real_llm_claude() {
    if !e2e_enabled() {
        eprintln!("Skipping e2e test: AX_EVAL_E2E=1 not set");
        return;
    }

    let available = detect_available_tools();
    let has_claude =
        available.contains(&"claude".to_string()) || available.contains(&"claude-code".to_string());
    if !has_claude {
        eprintln!("Skipping e2e test: claude not installed");
        return;
    }

    let mut cmd = ax_eval();
    cmd.args([
        "run",
        "--scenario",
        "ax-eval-fixtures/example_e2e.yaml",
        "--tool",
        "claude",
        "--timeout-secs",
        "60",
        "--cache",
    ])
    .env("AX_EVAL_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(
        stdout.contains("Running tool 'claude'"),
        "Should have executed claude: {}",
        stdout
    );
}

#[test]
#[ignore = "requires installed and authenticated codex plus AX_EVAL_E2E=1"]
fn test_e2e_with_real_llm_codex() {
    if !e2e_enabled() {
        eprintln!("Skipping e2e test: AX_EVAL_E2E=1 not set");
        return;
    }

    let available = detect_available_tools();
    if !available.contains(&"codex".to_string()) {
        eprintln!("Skipping e2e test: codex not installed");
        return;
    }

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("codex --version")
        .output()
        .expect("check codex");
    assert!(
        output.status.success(),
        "codex should be available and authenticated"
    );

    let mut cmd = ax_eval();
    cmd.args([
        "run",
        "--scenario",
        "ax-eval-fixtures/example_e2e.yaml",
        "--tool",
        "codex",
        "--model",
        &resolve_model("codex"),
        "--timeout-secs",
        "60",
        "--cache",
    ])
    .env("AX_EVAL_ENABLED", "1");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(
        stdout.contains("Running tool 'codex'"),
        "Should have executed codex: {}",
        stdout
    );
}

#[test]
#[ignore = "requires an installed and authenticated LLM tool plus AX_EVAL_E2E=1"]
fn test_e2e_reports_results_artifacts() {
    if !e2e_enabled() {
        eprintln!("Skipping e2e test: AX_EVAL_E2E=1 not set");
        return;
    }

    let available = detect_available_tools();
    // Prefer opencode when available so this test is deterministic
    let tool = if available.contains(&"opencode".to_string()) {
        "opencode".to_string()
    } else {
        match available.first().cloned() {
            Some(t) => t,
            None => {
                eprintln!("Skipping e2e test: no LLM tool available");
                return;
            }
        }
    };

    let results_dir = PathBuf::from("ax-eval-results");
    let _ = std::fs::remove_dir_all(&results_dir);

    let model = resolve_model(&tool);

    let mut cmd = ax_eval();
    cmd.args([
        "run",
        "--scenario",
        "ax-eval-fixtures/example_e2e.yaml",
        "--tool",
        &tool,
        "--model",
        &model,
        "--timeout-secs",
        "60",
    ])
    .env("AX_EVAL_ENABLED", "1");

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

    // Find the most recently created run directory, skipping the cache dir.
    let run_dir = entries
        .iter()
        .filter(|e| {
            let p = e.path();
            p.is_dir() && p.file_name() != Some(std::ffi::OsStr::new("cache"))
        })
        .max_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        })
        .map(|e| e.path())
        .expect("should have a run subdirectory");

    // Transcript and events live in the artifacts/ subdirectory
    let artifacts_dir = run_dir.join("artifacts");
    assert!(
        artifacts_dir.join("transcript.raw.txt").exists(),
        "transcript.raw.txt should exist in artifacts/"
    );
    assert!(
        artifacts_dir.join("events.jsonl").exists(),
        "events.jsonl should exist in artifacts/"
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
