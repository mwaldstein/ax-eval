use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandExecutionMode {
    Pty,
    Piped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub mode: CommandExecutionMode,
}

impl CommandResult {
    pub fn output(&self) -> String {
        let mut output = self.stdout.clone();
        output.push_str(&self.stderr);
        output
    }

    pub fn succeeded(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}

pub fn run_shell_with_session(
    command: &str,
    cwd: &Path,
    timeout_secs: u64,
    env: &HashMap<String, String>,
) -> anyhow::Result<CommandResult> {
    let env_vars = env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    crate::session::SessionRunner::new().run_command_result_with_env(
        "sh",
        &["-c", command],
        cwd,
        timeout_secs,
        &env_vars,
    )
}

pub fn run_shell_piped(
    command: &str,
    cwd: &Path,
    timeout_secs: u64,
    env: &HashMap<String, String>,
) -> anyhow::Result<CommandResult> {
    run_piped_command("sh", &["-c", command], cwd, timeout_secs, env)
}

pub fn run_piped_command(
    binary: &str,
    args: &[&str],
    cwd: &Path,
    timeout_secs: u64,
    env: &HashMap<String, String>,
) -> anyhow::Result<CommandResult> {
    let mut child = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .envs(env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn command '{}': {}", binary, e))?;

    let timeout = Duration::from_secs(timeout_secs);
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => {
            let exit_code = status.code().unwrap_or(-1);
            let output = child
                .wait_with_output()
                .map_err(|e| anyhow::anyhow!("Failed to read command output: {}", e))?;
            Ok(CommandResult {
                exit_code,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                timed_out: false,
                mode: CommandExecutionMode::Piped,
            })
        }
        Ok(None) => {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .map_err(|e| anyhow::anyhow!("Failed to read timed-out command output: {}", e))?;
            Ok(CommandResult {
                exit_code: -1,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                timed_out: true,
                mode: CommandExecutionMode::Piped,
            })
        }
        Err(e) => {
            let _ = child.kill();
            Err(anyhow::anyhow!(
                "Error waiting for command '{}': {}",
                binary,
                e
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_piped_runs_in_cwd_with_env_and_separate_stderr() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut env = HashMap::new();
        env.insert("COMMAND_EXECUTION_TEST".to_string(), "works".to_string());

        let result = run_shell_piped(
            "printf '%s' \"$COMMAND_EXECUTION_TEST\" > out.txt; printf 'warn' >&2",
            dir.path(),
            5,
            &env,
        )
        .expect("run shell");

        assert!(result.succeeded());
        assert_eq!(result.mode, CommandExecutionMode::Piped);
        assert_eq!(result.stderr, "warn");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("out.txt")).expect("output file"),
            "works"
        );
    }

    #[test]
    fn shell_piped_reports_timeout_without_erroring() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result =
            run_shell_piped("sleep 2", dir.path(), 1, &HashMap::new()).expect("timeout result");

        assert!(result.timed_out);
        assert!(!result.succeeded());
        assert_eq!(result.mode, CommandExecutionMode::Piped);
    }

    #[test]
    fn shell_session_runs_in_cwd_with_env() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut env = HashMap::new();
        env.insert("COMMAND_EXECUTION_TEST".to_string(), "works".to_string());

        let result = run_shell_with_session(
            "printf '%s' \"$COMMAND_EXECUTION_TEST\" > out.txt",
            dir.path(),
            5,
            &env,
        )
        .expect("run shell");

        assert!(result.succeeded());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("out.txt")).expect("output file"),
            "works"
        );
    }

    #[test]
    fn shell_session_timeout_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error =
            run_shell_with_session("sleep 2", dir.path(), 1, &HashMap::new()).expect_err("timeout");

        assert!(error.to_string().contains("timed out"));
    }
}
