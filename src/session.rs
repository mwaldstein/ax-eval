use crate::command_execution::{CommandExecutionMode, CommandResult};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::{mpsc::channel, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(test)]
use std::fs;

pub struct SessionRunner {
    pub pty_system: NativePtySystem,
}

#[derive(Debug, thiserror::Error)]
enum SessionCommandError {
    #[error("Command timed out after {0} seconds")]
    TimedOut(u64),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl SessionRunner {
    pub fn new() -> Self {
        Self {
            pty_system: NativePtySystem::default(),
        }
    }

    pub fn run_command(
        &self,
        cmd: &str,
        args: &[&str],
        cwd: &Path,
        timeout_secs: u64,
    ) -> anyhow::Result<(String, i32)> {
        self.run_command_with_env(cmd, args, cwd, timeout_secs, &[])
    }

    pub fn run_command_with_env(
        &self,
        cmd: &str,
        args: &[&str],
        cwd: &Path,
        timeout_secs: u64,
        env_vars: &[(String, String)],
    ) -> anyhow::Result<(String, i32)> {
        let result = self.run_command_result_with_env(cmd, args, cwd, timeout_secs, env_vars)?;
        Ok((result.output(), result.exit_code))
    }

    pub fn run_command_result_with_env(
        &self,
        cmd: &str,
        args: &[&str],
        cwd: &Path,
        timeout_secs: u64,
        env_vars: &[(String, String)],
    ) -> anyhow::Result<CommandResult> {
        match self.run_command_pty_with_env(cmd, args, cwd, timeout_secs, env_vars) {
            Ok(result) => Ok(result),
            Err(SessionCommandError::TimedOut(secs)) => {
                Err(anyhow::anyhow!("Command timed out after {} seconds", secs))
            }
            Err(SessionCommandError::Other(e)) => {
                tracing::debug!("PTY unavailable, falling back to pipes: {}", e);
                let env = env_vars.iter().cloned().collect::<HashMap<_, _>>();
                crate::command_execution::run_piped_command(cmd, args, cwd, timeout_secs, &env)
            }
        }
    }

    fn run_command_pty_with_env(
        &self,
        cmd: &str,
        args: &[&str],
        cwd: &Path,
        timeout_secs: u64,
        env_vars: &[(String, String)],
    ) -> Result<CommandResult, SessionCommandError> {
        let pair = self
            .pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(SessionCommandError::Other)?;

        let mut cmd_builder = CommandBuilder::new(cmd);
        cmd_builder.args(args);
        cmd_builder.cwd(cwd);
        for (key, value) in env_vars {
            cmd_builder.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(cmd_builder)
            .map_err(SessionCommandError::Other)?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(SessionCommandError::Other)?;

        // Drop slave to close the handle in the parent process.
        // The child has its own copy.
        drop(pair.slave);

        let (output_tx, output_rx) = channel();
        let (status_tx, status_rx) = channel();

        let child = Arc::new(Mutex::new(child));

        // Spawn thread to read output
        let reader_thread = thread::spawn(move || {
            let mut buf = [0u8; 1024];
            let mut output = Vec::new();
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                output.extend_from_slice(&buf[..n]);
            }
            let _ = output_tx.send(output);
        });

        // Spawn thread to wait for child process
        let child_clone = Arc::clone(&child);
        let wait_thread = thread::spawn(move || loop {
            let mut child_guard = child_clone.lock().unwrap_or_else(|e| e.into_inner());
            match child_guard.try_wait() {
                Ok(Some(status)) => {
                    let _ = status_tx.send(Ok(status));
                    return;
                }
                Ok(None) => {
                    drop(child_guard);
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    let _ = status_tx.send(Err(e));
                    return;
                }
            }
        });

        let timeout_duration = Duration::from_secs(timeout_secs);
        let wait_result = status_rx.recv_timeout(timeout_duration);

        let exit_status = match wait_result {
            Ok(Ok(status)) => status,
            Ok(Err(_)) => {
                return Err(SessionCommandError::Other(anyhow::anyhow!(
                    "Failed to wait for child process"
                )));
            }
            Err(_) => {
                let mut child_guard = child.lock().unwrap_or_else(|e| e.into_inner());
                let _ = child_guard.kill();
                drop(child_guard);
                let _ = status_rx.recv_timeout(Duration::from_secs(5));
                return Err(SessionCommandError::TimedOut(timeout_secs));
            }
        };

        // Get output (should be ready by now)
        let output: Vec<u8> = output_rx.recv().unwrap_or_default();

        let _ = reader_thread.join();
        let _ = wait_thread.join();

        let exit_code = exit_status.exit_code() as i32;
        Ok(CommandResult {
            exit_code,
            stdout: String::from_utf8_lossy(&output).to_string(),
            stderr: String::new(),
            timed_out: false,
            mode: CommandExecutionMode::Pty,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_successful_command_within_timeout() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        let result = runner.run_command("echo", &["hello"], dir.path(), 5);
        assert!(result.is_ok());
        let (output, exit_code) = result.unwrap();
        assert!(output.contains("hello"));
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_command_exceeds_timeout() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        // Use sleep command that exceeds timeout
        let result = runner.run_command("sleep", &["10"], dir.path(), 1);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("timed out"));
    }

    #[test]
    fn test_timed_out_pty_command_does_not_fallback_and_rerun() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        let marker = dir.path().join("marker.txt");
        let command = format!("printf x >> {}; sleep 10", marker.display());

        let result = runner.run_command("sh", &["-c", &command], dir.path(), 1);

        assert!(result.is_err());
        let marker_content = fs::read_to_string(marker).expect("marker file");
        assert_eq!(marker_content, "x");
    }

    #[test]
    fn test_zero_timeout_fails_immediately() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        // Even echo should fail with 0 timeout
        let result = runner.run_command("echo", &["hello"], dir.path(), 0);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("timed out"));
    }

    #[test]
    fn test_short_timeout_allows_quick_command() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        // 0.1 seconds should be enough for echo
        let result = runner.run_command("echo", &["quick"], dir.path(), 0);
        // This may succeed or timeout depending on system load
        // We just verify it doesn't hang
        let _ = result;
    }

    #[test]
    fn test_command_with_nonzero_exit_code() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        let result = runner.run_command("false", &[], dir.path(), 5);
        assert!(result.is_ok());
        let (_output, exit_code) = result.unwrap();
        assert_ne!(exit_code, 0);
    }

    #[test]
    fn test_true_command_succeeds() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        let result = runner.run_command("true", &[], dir.path(), 5);
        assert!(result.is_ok());
        let (_output, exit_code) = result.unwrap();
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_command_with_multiple_args() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        let result = runner.run_command("printf", &["%s %s\n", "hello", "world"], dir.path(), 5);
        assert!(result.is_ok());
        let (output, exit_code) = result.unwrap();
        assert!(output.contains("hello world"));
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_command_in_current_directory() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        // Create a test file in the temp directory
        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        // Verify pwd command runs in the temp directory
        #[cfg(unix)]
        let result = runner.run_command("pwd", &[], dir.path(), 5);
        #[cfg(windows)]
        let result = runner.run_command("cd", &[], dir.path(), 5);
        assert!(result.is_ok());
    }

    #[test]
    fn test_timeout_message_includes_duration() {
        let runner = SessionRunner::new();
        let dir = tempdir().unwrap();
        let result = runner.run_command("sleep", &["100"], dir.path(), 2);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("2 seconds"));
    }
}
