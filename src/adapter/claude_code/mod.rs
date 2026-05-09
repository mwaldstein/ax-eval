pub(crate) mod normalize;

use super::{ToolAdapter, ToolRunOutput};
use crate::scenario::Scenario;
use crate::session::SessionRunner;
use std::path::Path;

pub struct ClaudeCodeAdapter;

impl ToolAdapter for ClaudeCodeAdapter {
    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        let runner = SessionRunner::new();
        match runner.run_command("claude", &["--version"], Path::new("."), 10) {
            Ok(_) => Ok(super::ToolStatus {
                available: true,
                authenticated: true,
            }),
            Err(e) => Err(super::AdapterError::NotAvailable(format!(
                "Claude Code tool not found: {}",
                e
            ))),
        }
    }

    fn check_availability(&self) -> anyhow::Result<()> {
        let runner = SessionRunner::new();
        match runner.run_command("claude", &["--version"], Path::new("."), 10) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Claude Code tool not found: {}", e)),
        }
    }

    fn run(
        &self,
        scenario: &Scenario,
        cwd: &Path,
        model: Option<&str>,
        timeout_secs: u64,
    ) -> anyhow::Result<ToolRunOutput> {
        let runner = SessionRunner::new();

        let mut args = vec![
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
        ];
        if let Some(model) = model {
            args.push("--model");
            args.push(model);
        }
        args.push(&scenario.task.prompt);

        let target_env = scenario
            .target
            .env
            .as_ref()
            .map(|vars| {
                vars.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Vec<(String, String)>>()
            })
            .unwrap_or_default();

        let (output, exit_code) =
            runner.run_command_with_env("claude", &args, cwd, timeout_secs, &target_env)?;

        Ok(normalize::normalize(output, exit_code))
    }
}
