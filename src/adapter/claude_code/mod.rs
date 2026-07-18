pub(crate) mod normalize;

use super::{TargetProvision, ToolAdapter, ToolRunOutput};
use crate::scenario::{McpTarget, McpTransport, Scenario, TargetConfig};
use crate::session::SessionRunner;
use crate::target_env::expand_target_env_value;
use crate::target_env::TargetEnvironment;
use serde_json::{Map, Value};
use std::path::Path;

pub struct ClaudeCodeAdapter;

impl ToolAdapter for ClaudeCodeAdapter {
    fn supports_structured_tool_calls(&self) -> bool {
        true
    }

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

    fn provision_target(
        &self,
        target: &TargetConfig,
        workspace: &Path,
    ) -> anyhow::Result<TargetProvision> {
        let Some(target) = target.mcp() else {
            return Ok(TargetProvision::none());
        };

        provision_mcp_target(target, workspace)?;
        Ok(TargetProvision::none())
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

        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--include-partial-messages".to_string(),
        ];
        if let Some(model) = model {
            args.push("--model".to_string());
            args.push(model.to_string());
        }
        if scenario.target.is_mcp() {
            args.push("--mcp-config".to_string());
            args.push(mcp_config_path(cwd).to_string_lossy().into_owned());
        }
        args.push(scenario.task.prompt.clone());

        let target_env = target_env.to_session_env();
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let (output, exit_code) =
            runner.run_command_with_env("claude", &arg_refs, cwd, timeout_secs, &target_env)?;

        Ok(normalize::normalize(output, exit_code))
    }
}

fn mcp_config_path(workspace: &Path) -> std::path::PathBuf {
    workspace.join(".mcp.json")
}

fn results_dir_for_workspace(workspace: &Path) -> &Path {
    workspace.parent().unwrap_or(workspace)
}

fn expanded_map(
    values: &std::collections::HashMap<String, String>,
    fixture_dir: &Path,
    results_dir: &Path,
) -> anyhow::Result<Map<String, Value>> {
    values
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                Value::String(expand_target_env_value(value, fixture_dir, results_dir)?),
            ))
        })
        .collect()
}

fn provision_mcp_target(target: &McpTarget, workspace: &Path) -> anyhow::Result<()> {
    let config_path = mcp_config_path(workspace);
    let mut mcp_servers = Map::new();
    mcp_servers.insert(
        target.name.clone(),
        render_mcp_target(target, workspace, results_dir_for_workspace(workspace))?,
    );

    let mut root = Map::new();
    root.insert("mcpServers".to_string(), Value::Object(mcp_servers));
    std::fs::write(
        config_path,
        serde_json::to_string_pretty(&Value::Object(root))? + "\n",
    )?;
    Ok(())
}

fn render_mcp_target(
    target: &McpTarget,
    fixture_dir: &Path,
    results_dir: &Path,
) -> anyhow::Result<Value> {
    match &target.transport {
        McpTransport::Stdio { command, args } => {
            let mut entry = Map::new();
            entry.insert("type".to_string(), Value::String("stdio".to_string()));
            entry.insert(
                "command".to_string(),
                Value::String(expand_target_env_value(command, fixture_dir, results_dir)?),
            );
            entry.insert(
                "args".to_string(),
                Value::Array(
                    args.iter()
                        .map(|arg| {
                            Ok(Value::String(expand_target_env_value(
                                arg,
                                fixture_dir,
                                results_dir,
                            )?))
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?,
                ),
            );
            if let Some(env) = &target.env {
                entry.insert(
                    "env".to_string(),
                    Value::Object(expanded_map(env, fixture_dir, results_dir)?),
                );
            }
            Ok(Value::Object(entry))
        }
        McpTransport::Http { url, headers } => {
            let mut entry = Map::new();
            entry.insert("type".to_string(), Value::String("http".to_string()));
            entry.insert(
                "url".to_string(),
                Value::String(expand_target_env_value(url, fixture_dir, results_dir)?),
            );
            if let Some(headers) = headers {
                entry.insert(
                    "headers".to_string(),
                    Value::Object(expanded_map(headers, fixture_dir, results_dir)?),
                );
            }
            Ok(Value::Object(entry))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{McpTarget, McpTransport, TargetConfig};
    use std::collections::HashMap;

    #[test]
    fn provisions_stdio_mcp_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let target = TargetConfig::Mcp(McpTarget {
            name: "todo".to_string(),
            transport: McpTransport::Stdio {
                command: "${AX_EVAL_FIXTURE_DIR}/todo-mcp-server".to_string(),
                args: vec!["--root".to_string(), "${AX_EVAL_FIXTURE_DIR}".to_string()],
            },
            auth: None,
            tools: vec!["add".to_string()],
            env: Some(HashMap::from([(
                "TODO_DB".to_string(),
                "${AX_EVAL_RESULTS_DIR}/todo.db".to_string(),
            )])),
            health_check: None,
        });

        ClaudeCodeAdapter
            .provision_target(&target, &workspace)
            .expect("provision");

        let expected = format!(
            r#"{{
  "mcpServers": {{
    "todo": {{
      "args": [
        "--root",
        "{fixture}"
      ],
      "command": "{fixture}/todo-mcp-server",
      "env": {{
        "TODO_DB": "{results}/todo.db"
      }},
      "type": "stdio"
    }}
  }}
}}
"#,
            fixture = workspace.to_string_lossy(),
            results = workspace.parent().expect("results").to_string_lossy()
        );

        assert_eq!(
            std::fs::read_to_string(mcp_config_path(&workspace)).expect("config"),
            expected
        );
    }

    #[test]
    fn provisions_http_mcp_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let target = TargetConfig::Mcp(McpTarget {
            name: "search".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.example.com/${AX_EVAL_RESULTS_DIR}/mcp".to_string(),
                headers: Some(HashMap::from([(
                    "X-API-Key".to_string(),
                    "token-${AX_EVAL_FIXTURE_DIR}".to_string(),
                )])),
            },
            auth: None,
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        ClaudeCodeAdapter
            .provision_target(&target, &workspace)
            .expect("provision");

        let expected = format!(
            r#"{{
  "mcpServers": {{
    "search": {{
      "headers": {{
        "X-API-Key": "token-{fixture}"
      }},
      "type": "http",
      "url": "https://mcp.example.com/{results}/mcp"
    }}
  }}
}}
"#,
            fixture = workspace.to_string_lossy(),
            results = workspace.parent().expect("results").to_string_lossy()
        );

        assert_eq!(
            std::fs::read_to_string(mcp_config_path(&workspace)).expect("config"),
            expected
        );
    }
}
