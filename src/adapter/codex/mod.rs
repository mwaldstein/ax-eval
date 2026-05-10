pub(crate) mod normalize;

use super::{ToolAdapter, ToolRunOutput};
use crate::scenario::Scenario;
use crate::session::SessionRunner;
use crate::target_env::TargetEnvironment;
use std::path::Path;

pub struct CodexAdapter;

impl ToolAdapter for CodexAdapter {
    fn supports_structured_tool_calls(&self) -> bool {
        true
    }

    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        let runner = SessionRunner::new();
        let available = match runner.run_command("codex", &["--version"], Path::new("."), 10) {
            Ok(_) => true,
            Err(e) => {
                return Err(super::AdapterError::NotAvailable(format!(
                    "Codex tool not found: {}",
                    e
                )))
            }
        };

        let authenticated = std::env::var("OPENAI_API_KEY").is_ok();

        Ok(super::ToolStatus {
            available,
            authenticated,
        })
    }

    fn run(
        &self,
        scenario: &Scenario,
        cwd: &Path,
        model: Option<&str>,
        timeout_secs: u64,
        target_env: &TargetEnvironment,
    ) -> anyhow::Result<ToolRunOutput> {
        let runner = SessionRunner::new();

        let mut args = vec!["exec", "--json", "--full-auto", "--skip-git-repo-check"];
        if let Some(model) = model {
            if model != "default" {
                args.push("--model");
                args.push(model);
            }
        }
        args.push(&scenario.task.prompt);

        let target_env = target_env.to_session_env();

        let (output, exit_code) =
            runner.run_command_with_env("codex", &args, cwd, timeout_secs, &target_env)?;

        Ok(normalize::normalize(output, exit_code))
    }
}
