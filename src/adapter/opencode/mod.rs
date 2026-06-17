pub(crate) mod normalize;

use super::{ToolAdapter, ToolRunOutput};
use crate::scenario::Scenario;
use crate::session::SessionRunner;
use crate::target_env::TargetEnvironment;
use std::path::Path;

pub struct OpenCodeAdapter;

impl ToolAdapter for OpenCodeAdapter {
    fn supports_structured_tool_calls(&self) -> bool {
        true
    }

    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        let runner = SessionRunner::new();
        match runner.run_command("opencode", &["--version"], Path::new("."), 10) {
            Ok(_) => Ok(super::ToolStatus {
                available: true,
                authenticated: true,
            }),
            Err(e) => Err(super::AdapterError::NotAvailable(format!(
                "OpenCode tool not found: {}",
                e
            ))),
        }
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

        let mut args: Vec<String> = vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--dir".to_string(),
            cwd.to_string_lossy().into_owned(),
        ];
        if let Some(model) = model {
            args.push("--model".to_string());
            args.push(model.to_string());
        }
        args.push(scenario.task.prompt.clone());

        let xdg_config_dir = cwd
            .canonicalize()
            .unwrap_or_else(|_| cwd.to_path_buf())
            .join(".opencode_config");
        std::fs::create_dir_all(&xdg_config_dir).ok();
        let mut env_vars: Vec<(String, String)> = vec![(
            "XDG_CONFIG_HOME".to_string(),
            xdg_config_dir.to_string_lossy().to_string(),
        )];
        target_env.append_to_session_env(&mut env_vars);

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (output, exit_code) =
            runner.run_command_with_env("opencode", &arg_refs, cwd, timeout_secs, &env_vars)?;

        Ok(normalize::normalize(output, exit_code))
    }
}
