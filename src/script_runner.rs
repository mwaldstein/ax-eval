//! Script runner utility for executing post-evaluation and custom evaluator scripts.
//!
//! This module provides a `ScriptRunner` that executes shell commands in the fixture
//! directory with the appropriate environment variables set. It supports timeout
//! enforcement using the `wait-timeout` crate.

use crate::command_execution::{run_piped_command, CommandResult};
use std::collections::HashMap;
use std::path::PathBuf;

/// Result of executing a script.
pub type ScriptResult = CommandResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptRunStatus {
    Succeeded,
    TimedOut { timeout_secs: u64 },
    Failed { exit_code: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRunReport {
    pub command: String,
    pub timeout_secs: u64,
    pub result: ScriptResult,
    pub status: ScriptRunStatus,
}

impl ScriptRunReport {
    fn new(command: &str, timeout_secs: u64, result: ScriptResult) -> Self {
        let status = if result.timed_out {
            ScriptRunStatus::TimedOut { timeout_secs }
        } else if result.exit_code == 0 {
            ScriptRunStatus::Succeeded
        } else {
            ScriptRunStatus::Failed {
                exit_code: result.exit_code,
            }
        };

        Self {
            command: command.to_string(),
            timeout_secs,
            result,
            status,
        }
    }

    pub fn succeeded(&self) -> bool {
        self.status == ScriptRunStatus::Succeeded
    }

    pub fn failure_summary(&self) -> Option<String> {
        match self.status {
            ScriptRunStatus::Succeeded => None,
            ScriptRunStatus::TimedOut { timeout_secs } => {
                Some(format!("Timed out after {} seconds", timeout_secs))
            }
            ScriptRunStatus::Failed { exit_code } => {
                Some(format!("Exit code {}: {}", exit_code, self.result.stderr))
            }
        }
    }

    pub fn event(&self, event_type: &str) -> serde_json::Value {
        serde_json::json!({
            "type": event_type,
            "command": self.command,
            "exit_code": self.result.exit_code,
            "timed_out": self.result.timed_out,
            "stdout": self.result.stdout,
            "stderr": self.result.stderr,
        })
    }
}

/// A runner for executing scripts in the fixture directory.
#[derive(Debug, Clone)]
pub struct ScriptRunner {
    fixture_dir: PathBuf,
    results_dir: PathBuf,
    scenario_name: String,
    agent: String,
    model: String,
    transcript_path: Option<PathBuf>,
    events_path: Option<PathBuf>,
    target_env: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ScriptRunnerConfig {
    pub fixture_dir: PathBuf,
    pub results_dir: PathBuf,
    pub scenario_name: String,
    pub agent: String,
    pub model: String,
    pub transcript_path: Option<PathBuf>,
    pub events_path: Option<PathBuf>,
    pub target_env: HashMap<String, String>,
}

impl ScriptRunner {
    /// Create a new script runner.
    pub fn new(config: ScriptRunnerConfig) -> Self {
        // Ensure fixture_dir is absolute so that scripts referencing
        // LLM_TOOL_TEST_FIXTURE_DIR resolve paths correctly regardless
        // of the script's working directory.
        let fixture_dir = config
            .fixture_dir
            .canonicalize()
            .unwrap_or(config.fixture_dir);
        Self {
            fixture_dir,
            results_dir: config.results_dir,
            scenario_name: config.scenario_name,
            agent: config.agent,
            model: config.model,
            transcript_path: config.transcript_path,
            events_path: config.events_path,
            target_env: config.target_env,
        }
    }

    /// Run a shell command with the configured environment.
    ///
    /// The command is executed via `sh -c` in the fixture directory with
    /// LLM_TOOL_TEST_* environment variables set. The timeout is enforced
    /// using the wait-timeout crate.
    pub fn run(&self, command: &str, timeout_secs: u64) -> anyhow::Result<ScriptResult> {
        run_piped_command(
            "sh",
            &["-c", command],
            &self.fixture_dir,
            timeout_secs,
            &self.build_env(),
        )
    }

    pub fn run_report(&self, command: &str, timeout_secs: u64) -> anyhow::Result<ScriptRunReport> {
        let result = self.run(command, timeout_secs)?;
        Ok(ScriptRunReport::new(command, timeout_secs, result))
    }

    /// Build the environment variables for script execution.
    fn build_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // LLM_TOOL_TEST_* variables
        env.insert(
            "LLM_TOOL_TEST_FIXTURE_DIR".to_string(),
            self.fixture_dir.to_string_lossy().to_string(),
        );
        env.insert(
            "LLM_TOOL_TEST_RESULTS_DIR".to_string(),
            self.results_dir.to_string_lossy().to_string(),
        );
        env.insert(
            "LLM_TOOL_TEST_SCENARIO".to_string(),
            self.scenario_name.clone(),
        );
        env.insert("LLM_TOOL_TEST_AGENT".to_string(), self.agent.clone());
        env.insert("LLM_TOOL_TEST_MODEL".to_string(), self.model.clone());

        if let Some(ref path) = self.transcript_path {
            env.insert(
                "LLM_TOOL_TEST_TRANSCRIPT".to_string(),
                path.to_string_lossy().to_string(),
            );
        }

        if let Some(ref path) = self.events_path {
            env.insert(
                "LLM_TOOL_TEST_EVENTS".to_string(),
                path.to_string_lossy().to_string(),
            );
        }

        // Merge target environment variables (they take precedence)
        for (key, value) in &self.target_env {
            env.insert(key.clone(), value.clone());
        }

        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_runner(fixture_dir: PathBuf) -> ScriptRunner {
        ScriptRunner::new(ScriptRunnerConfig {
            fixture_dir,
            results_dir: PathBuf::from("/tmp/results"),
            scenario_name: "test_scenario".to_string(),
            agent: "test_agent".to_string(),
            model: "test_model".to_string(),
            transcript_path: None,
            events_path: None,
            target_env: HashMap::new(),
        })
    }

    #[test]
    fn script_runner_config_builds_runner_environment() {
        let temp = TempDir::new().unwrap();
        let results = TempDir::new().unwrap();
        let transcript_path = results.path().join("transcript.raw.txt");
        let events_path = results.path().join("events.jsonl");
        let mut target_env = HashMap::new();
        target_env.insert("TARGET_OVERRIDE".to_string(), "target-value".to_string());

        let runner = ScriptRunner::new(ScriptRunnerConfig {
            fixture_dir: temp.path().to_path_buf(),
            results_dir: results.path().to_path_buf(),
            scenario_name: "scenario-a".to_string(),
            agent: "mock".to_string(),
            model: "model-a".to_string(),
            transcript_path: Some(transcript_path.clone()),
            events_path: Some(events_path.clone()),
            target_env,
        });

        let env = runner.build_env();

        assert_eq!(env["LLM_TOOL_TEST_SCENARIO"], "scenario-a");
        assert_eq!(env["LLM_TOOL_TEST_AGENT"], "mock");
        assert_eq!(env["LLM_TOOL_TEST_MODEL"], "model-a");
        assert_eq!(
            env["LLM_TOOL_TEST_TRANSCRIPT"],
            transcript_path.to_string_lossy()
        );
        assert_eq!(env["LLM_TOOL_TEST_EVENTS"], events_path.to_string_lossy());
        assert_eq!(env["TARGET_OVERRIDE"], "target-value");
    }

    #[test]
    fn test_script_echo() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let result = runner.run("echo 'hello world'", 10).unwrap();

        assert!(result.succeeded());
        assert!(result.stdout.contains("hello world"));
        assert!(!result.timed_out);
    }

    #[test]
    fn test_script_exit_code_success() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let result = runner.run("true", 10).unwrap();

        assert!(result.succeeded());
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_script_exit_code_failure() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let result = runner.run("false", 10).unwrap();

        assert!(!result.succeeded());
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_script_timeout() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        // Sleep for 2 seconds with a 1 second timeout
        let result = runner.run("sleep 2", 1).unwrap();

        assert!(!result.succeeded());
        assert!(result.timed_out);
    }

    #[test]
    fn test_script_report_classifies_success() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let report = runner.run_report("true", 10).unwrap();

        assert_eq!(report.status, ScriptRunStatus::Succeeded);
        assert!(report.succeeded());
        assert_eq!(report.failure_summary(), None);
    }

    #[test]
    fn test_script_report_classifies_failure() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let report = runner.run_report("echo nope >&2; exit 7", 10).unwrap();

        assert_eq!(report.status, ScriptRunStatus::Failed { exit_code: 7 });
        assert_eq!(
            report.failure_summary(),
            Some("Exit code 7: nope\n".to_string())
        );
    }

    #[test]
    fn test_script_report_classifies_timeout() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let report = runner.run_report("sleep 2", 1).unwrap();

        assert_eq!(report.status, ScriptRunStatus::TimedOut { timeout_secs: 1 });
        assert_eq!(
            report.failure_summary(),
            Some("Timed out after 1 seconds".to_string())
        );
    }

    #[test]
    fn test_script_captures_stderr() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let result = runner.run("echo 'error msg' >&2", 10).unwrap();

        assert!(result.succeeded());
        assert!(result.stderr.contains("error msg"));
    }

    #[test]
    fn test_script_runs_in_fixture_dir() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("testfile.txt"), "content").unwrap();

        let runner = create_test_runner(temp.path().to_path_buf());
        let result = runner.run("cat testfile.txt", 10).unwrap();

        assert!(result.succeeded());
        assert!(result.stdout.contains("content"));
    }

    #[test]
    fn test_script_env_vars() {
        let temp = TempDir::new().unwrap();
        let runner = create_test_runner(temp.path().to_path_buf());

        let result = runner.run("echo $LLM_TOOL_TEST_SCENARIO", 10).unwrap();

        assert!(result.succeeded());
        assert!(result.stdout.contains("test_scenario"));
    }

    #[test]
    fn test_target_env_override() {
        let temp = TempDir::new().unwrap();
        let mut target_env = HashMap::new();
        target_env.insert(
            "LLM_TOOL_TEST_SCENARIO".to_string(),
            "overridden".to_string(),
        );

        let runner = ScriptRunner::new(ScriptRunnerConfig {
            fixture_dir: temp.path().to_path_buf(),
            results_dir: PathBuf::from("/tmp/results"),
            scenario_name: "test_scenario".to_string(),
            agent: "test_agent".to_string(),
            model: "test_model".to_string(),
            transcript_path: None,
            events_path: None,
            target_env,
        });

        let result = runner.run("echo $LLM_TOOL_TEST_SCENARIO", 10).unwrap();

        assert!(result.succeeded());
        assert!(result.stdout.contains("overridden"));
    }
}
