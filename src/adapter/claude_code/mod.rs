pub(crate) mod normalize;

use super::{TargetProvision, ToolAdapter, ToolRunOutput};
use crate::mcp_auth::merged_http_headers;
use crate::scenario::{McpTarget, McpTransport, Scenario, TargetConfig};
use crate::session::SessionRunner;
use crate::target_env::expand_target_env_value;
use crate::target_env::{AgentEnvironment, TargetEnvironment};
use serde_json::{Map, Value};
use std::path::Path;

pub struct ClaudeCodeAdapter;

impl ToolAdapter for ClaudeCodeAdapter {
    fn required_agent_env(&self) -> &'static [&'static str] {
        &[
            "HOME",
            "PATH",
            "USER",
            "LOGNAME",
            "SHELL",
            "TERM",
            "TMPDIR",
            "TMP",
            "TEMP",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "COLORTERM",
            "XDG_CACHE_HOME",
            "XDG_DATA_HOME",
            "SSH_AUTH_SOCK",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CONFIG_DIR",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_REGION",
            "AWS_DEFAULT_REGION",
            "AWS_PROFILE",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "CLOUD_ML_REGION",
            "ANTHROPIC_VERTEX_PROJECT_ID",
        ]
    }

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
        target_env: &TargetEnvironment,
    ) -> anyhow::Result<TargetProvision> {
        let Some(target) = target.mcp() else {
            return Ok(TargetProvision::none());
        };

        provision_mcp_target(target, workspace, target_env)?;
        Ok(TargetProvision::none())
    }

    fn run(
        &self,
        scenario: &Scenario,
        cwd: &Path,
        model: Option<&str>,
        timeout_secs: u64,
        agent_env: &AgentEnvironment,
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

        let agent_env = agent_env.to_session_env();
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let result = runner.run_command_result_with_projected_env(
            "claude",
            &arg_refs,
            cwd,
            timeout_secs,
            &agent_env,
        )?;
        let output = result.output();
        let exit_code = result.exit_code;

        Ok(normalize::normalize(output, exit_code))
    }
}

fn mcp_config_path(workspace: &Path) -> std::path::PathBuf {
    workspace.join(".mcp.json")
}

fn results_dir_for_workspace(workspace: &Path) -> &Path {
    workspace.parent().unwrap_or(workspace)
}

fn provision_mcp_target(
    target: &McpTarget,
    workspace: &Path,
    target_env: &TargetEnvironment,
) -> anyhow::Result<()> {
    let fallback_env;
    let target_env = if target_env.is_empty() && target.env.is_some() {
        fallback_env = TargetEnvironment::expanded_from_config(
            target.env.as_ref(),
            workspace,
            results_dir_for_workspace(workspace),
        )?;
        &fallback_env
    } else {
        target_env
    };
    let config_path = mcp_config_path(workspace);
    let mut mcp_servers = Map::new();
    mcp_servers.insert(
        target.name.clone(),
        render_mcp_target(
            target,
            workspace,
            results_dir_for_workspace(workspace),
            target_env,
        )?,
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
    target_env: &TargetEnvironment,
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
            if !target_env.as_map().is_empty() {
                entry.insert(
                    "env".to_string(),
                    Value::Object(
                        target_env
                            .as_map()
                            .iter()
                            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                            .collect(),
                    ),
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
            if let Some(headers) = merged_http_headers(
                headers.as_ref(),
                target.auth.as_ref(),
                fixture_dir,
                results_dir,
            )? {
                entry.insert(
                    "headers".to_string(),
                    Value::Object(
                        headers
                            .into_iter()
                            .map(|(key, value)| (key, Value::String(value)))
                            .collect(),
                    ),
                );
            }
            Ok(Value::Object(entry))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{McpAuth, McpTarget, McpTransport, TargetConfig};
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
            .provision_target(&target, &workspace, &TargetEnvironment::default())
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
            .provision_target(&target, &workspace, &TargetEnvironment::default())
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

    #[test]
    fn provisions_bearer_env_auth_header() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let env_name = "AX_EVAL_CLAUDE_BEARER_AUTH_UNIQUE";
        std::env::set_var(env_name, "claude-secret-token");
        let target = TargetConfig::Mcp(McpTarget {
            name: "search".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.example.com/mcp".to_string(),
                headers: None,
            },
            auth: Some(McpAuth::BearerEnv {
                env: env_name.to_string(),
            }),
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        ClaudeCodeAdapter
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(mcp_config_path(&workspace)).expect("config");
        assert!(content.contains(r#""Authorization": "Bearer claude-secret-token""#));
    }

    #[test]
    fn provisions_auth_headers_over_transport_headers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let env_name = "AX_EVAL_CLAUDE_HEADER_AUTH_UNIQUE";
        std::env::set_var(env_name, "api-key-secret");
        let target = TargetConfig::Mcp(McpTarget {
            name: "search".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.example.com/mcp".to_string(),
                headers: Some(HashMap::from([
                    ("X-API-Key".to_string(), "transport-key".to_string()),
                    ("X-Trace".to_string(), "trace".to_string()),
                ])),
            },
            auth: Some(McpAuth::Headers {
                headers: HashMap::from([("X-API-Key".to_string(), format!("${{env:{env_name}}}"))]),
            }),
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        ClaudeCodeAdapter
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(mcp_config_path(&workspace)).expect("config");
        assert!(content.contains(r#""X-API-Key": "api-key-secret""#));
        assert!(content.contains(r#""X-Trace": "trace""#));
        assert!(!content.contains("transport-key"));
    }

    #[test]
    fn provisions_host_session_without_credentials() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let target = TargetConfig::Mcp(McpTarget {
            name: "search".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.example.com/mcp".to_string(),
                headers: None,
            },
            auth: Some(McpAuth::HostSession),
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        ClaudeCodeAdapter
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(mcp_config_path(&workspace)).expect("config");
        assert!(!content.contains("Authorization"));
        assert!(!content.contains("headers"));
    }
}
