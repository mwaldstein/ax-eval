//! Script runner utility for executing post-evaluation and custom evaluator scripts.
//!
//! This module provides a `ScriptRunner` that executes shell commands in the fixture
//! directory with the appropriate environment variables set. It supports timeout
//! enforcement using the `wait-timeout` crate.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use wait_timeout::ChildExt;

/// Result of executing a script.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptResult {
    /// Exit code of the script (0 for success)
    pub exit_code: i32,
    /// Standard output captured from the script
    pub stdout: String,
    /// Standard error captured from the script
    pub stderr: String,
    /// Whether the script timed out
    pub timed_out: bool,
}

impl ScriptResult {
    /// Returns true if the script succeeded (exit code 0 and not timed out).
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
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

impl ScriptRunner {
    /// Create a new script runner.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fixture_dir: PathBuf,
        results_dir: PathBuf,
        scenario_name: String,
        agent: String,
        model: String,
        transcript_path: Option<PathBuf>,
        events_path: Option<PathBuf>,
        target_env: HashMap<String, String>,
    ) -> Self {
        // Ensure fixture_dir is absolute so that scripts referencing
        // LLM_TOOL_TEST_FIXTURE_DIR resolve paths correctly regardless
        // of the script's working directory.
        let fixture_dir = fixture_dir.canonicalize().unwrap_or(fixture_dir);
        Self {
            fixture_dir,
            results_dir,
            scenario_name,
            agent,
            model,
            transcript_path,
            events_path,
            target_env,
        }
    }

    /// Run a shell command with the configured environment.
    ///
    /// The command is executed via `sh -c` in the fixture directory with
    /// LLM_TOOL_TEST_* environment variables set. The timeout is enforced
    /// using the wait-timeout crate.
    pub fn run(&self, command: &str, timeout_secs: u64) -> anyhow::Result<ScriptResult> {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.fixture_dir)
            .envs(self.build_env())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn script: {}", e))?;

        let timeout = std::time::Duration::from_secs(timeout_secs);
        let result = match child.wait_timeout(timeout) {
            Ok(Some(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                let stdout = self.read_child_stdout(&mut child)?;
                let stderr = self.read_child_stderr(&mut child)?;
                ScriptResult {
                    exit_code,
                    stdout,
                    stderr,
                    timed_out: false,
                }
            }
            Ok(None) => {
                let _ = child.kill();
                let stdout = self.read_child_stdout(&mut child)?;
                let stderr = self.read_child_stderr(&mut child)?;
                ScriptResult {
                    exit_code: -1,
                    stdout,
                    stderr,
                    timed_out: true,
                }
            }
            Err(e) => {
                let _ = child.kill();
                return Err(anyhow::anyhow!("Error waiting for script: {}", e));
            }
        };

        Ok(result)
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

    fn read_child_stdout(&self, child: &mut std::process::Child) -> anyhow::Result<String> {
        let mut stdout = String::new();
        if let Some(ref mut pipe) = child.stdout {
            use std::io::Read;
            pipe.read_to_string(&mut stdout)
                .map_err(|e| anyhow::anyhow!("Failed to read stdout: {}", e))?;
        }
        Ok(stdout)
    }

    fn read_child_stderr(&self, child: &mut std::process::Child) -> anyhow::Result<String> {
        let mut stderr = String::new();
        if let Some(ref mut pipe) = child.stderr {
            use std::io::Read;
            pipe.read_to_string(&mut stderr)
                .map_err(|e| anyhow::anyhow!("Failed to read stderr: {}", e))?;
        }
        Ok(stderr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_runner(fixture_dir: PathBuf) -> ScriptRunner {
        ScriptRunner::new(
            fixture_dir,
            PathBuf::from("/tmp/results"),
            "test_scenario".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            HashMap::new(),
        )
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

        let runner = ScriptRunner::new(
            temp.path().to_path_buf(),
            PathBuf::from("/tmp/results"),
            "test_scenario".to_string(),
            "test_agent".to_string(),
            "test_model".to_string(),
            None,
            None,
            target_env,
        );

        let result = runner.run("echo $LLM_TOOL_TEST_SCENARIO", 10).unwrap();

        assert!(result.succeeded());
        assert!(result.stdout.contains("overridden"));
    }
}
