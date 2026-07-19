pub(crate) mod normalize;

use super::{TargetProvision, ToolAdapter, ToolRunOutput};
use crate::mcp_auth::{merged_http_headers, static_auth_mode, StaticAuthMode};
use crate::scenario::{McpTarget, McpTransport, Scenario, TargetConfig};
use crate::session::SessionRunner;
use crate::target_env::expand_target_env_value;
use crate::target_env::{AgentEnvironment, TargetEnvironment};
use serde_json::{Map, Value};
use std::path::Path;

pub struct OpenCodeAdapter;

impl ToolAdapter for OpenCodeAdapter {
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
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GOOGLE_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "MISTRAL_API_KEY",
            "XAI_API_KEY",
            "OPENROUTER_API_KEY",
            "DEEPSEEK_API_KEY",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_REGION",
            "AWS_PROFILE",
        ]
    }

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
        let mut env_vars = agent_env.to_session_env();
        env_vars.push((
            "XDG_CONFIG_HOME".to_string(),
            xdg_config_dir.to_string_lossy().to_string(),
        ));

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let result = runner.run_command_result_with_projected_env(
            "opencode",
            &arg_refs,
            cwd,
            timeout_secs,
            &env_vars,
        )?;
        let output = result.output();
        let exit_code = result.exit_code;

        Ok(normalize::normalize(output, exit_code))
    }
}

fn opencode_config_dir(workspace: &Path) -> std::path::PathBuf {
    workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf())
        .join(".opencode_config")
}

fn opencode_config_path(workspace: &Path) -> std::path::PathBuf {
    opencode_config_dir(workspace).join("opencode.json")
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
    let config_path = opencode_config_path(workspace);
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut config = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str::<Value>(&content)?
    } else {
        Value::Object(Map::new())
    };

    let object = config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("opencode config must be a JSON object"))?;
    let mcp = object
        .entry("mcp")
        .or_insert_with(|| Value::Object(Map::new()));
    let mcp = mcp
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("opencode config field 'mcp' must be a JSON object"))?;

    mcp.insert(
        target.name.clone(),
        render_mcp_target(
            target,
            workspace,
            results_dir_for_workspace(workspace),
            target_env,
        )?,
    );

    std::fs::write(config_path, serde_json::to_string_pretty(&config)? + "\n")?;
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
            let command = std::iter::once(command)
                .chain(args.iter())
                .map(|value| {
                    Ok(Value::String(expand_target_env_value(
                        value,
                        fixture_dir,
                        results_dir,
                    )?))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let mut entry = Map::new();
            entry.insert("type".to_string(), Value::String("local".to_string()));
            entry.insert("command".to_string(), Value::Array(command));
            entry.insert("enabled".to_string(), Value::Bool(true));
            if !target_env.as_map().is_empty() {
                entry.insert(
                    "environment".to_string(),
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
            entry.insert("type".to_string(), Value::String("remote".to_string()));
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
            if static_auth_mode(target.auth.as_ref()) == StaticAuthMode::Static {
                entry.insert("oauth".to_string(), Value::Bool(false));
            }
            entry.insert("enabled".to_string(), Value::Bool(true));
            Ok(Value::Object(entry))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{McpAuth, McpTarget, McpTransport, TargetConfig};
    use std::collections::HashMap;

    fn stdio_target() -> TargetConfig {
        TargetConfig::Mcp(McpTarget {
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
        })
    }

    #[test]
    fn provisions_stdio_mcp_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");

        OpenCodeAdapter
            .provision_target(&stdio_target(), &workspace, &TargetEnvironment::default())
            .expect("provision");

        let expected = format!(
            r#"{{
  "mcp": {{
    "todo": {{
      "command": [
        "{fixture}/todo-mcp-server",
        "--root",
        "{fixture}"
      ],
      "enabled": true,
      "environment": {{
        "TODO_DB": "{results}/todo.db"
      }},
      "type": "local"
    }}
  }}
}}
"#,
            fixture = workspace.to_string_lossy(),
            results = workspace.parent().expect("results").to_string_lossy()
        );

        assert_eq!(
            std::fs::read_to_string(opencode_config_path(&workspace)).expect("config"),
            expected
        );
    }

    #[test]
    fn provisions_http_mcp_config_and_preserves_existing_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(opencode_config_dir(&workspace)).expect("config dir");
        std::fs::write(
            opencode_config_path(&workspace),
            r#"{
  "theme": "system",
  "mcp": {
    "existing": {
      "type": "remote",
      "url": "https://existing.example/mcp",
      "enabled": true
    }
  }
}
"#,
        )
        .expect("existing config");

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

        OpenCodeAdapter
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let expected = format!(
            r#"{{
  "mcp": {{
    "existing": {{
      "enabled": true,
      "type": "remote",
      "url": "https://existing.example/mcp"
    }},
    "search": {{
      "enabled": true,
      "headers": {{
        "X-API-Key": "token-{fixture}"
      }},
      "type": "remote",
      "url": "https://mcp.example.com/{results}/mcp"
    }}
  }},
  "theme": "system"
}}
"#,
            fixture = workspace.to_string_lossy(),
            results = workspace.parent().expect("results").to_string_lossy()
        );

        assert_eq!(
            std::fs::read_to_string(opencode_config_path(&workspace)).expect("config"),
            expected
        );
    }

    #[test]
    fn provisions_bearer_env_auth_with_oauth_disabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let env_name = "AX_EVAL_OPENCODE_BEARER_AUTH_UNIQUE";
        std::env::set_var(env_name, "opencode-secret-token");
        let target = TargetConfig::Mcp(McpTarget {
            name: "search".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.example.com/mcp".to_string(),
                headers: Some(HashMap::from([(
                    "Authorization".to_string(),
                    "Bearer transport-token".to_string(),
                )])),
            },
            auth: Some(McpAuth::BearerEnv {
                env: env_name.to_string(),
            }),
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        OpenCodeAdapter
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(opencode_config_path(&workspace)).expect("config");
        assert!(content.contains(r#""Authorization": "Bearer opencode-secret-token""#));
        assert!(content.contains(r#""oauth": false"#));
        assert!(!content.contains("transport-token"));
    }

    #[test]
    fn provisions_auth_headers_over_transport_headers_with_oauth_disabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let env_name = "AX_EVAL_OPENCODE_HEADER_AUTH_UNIQUE";
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

        OpenCodeAdapter
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(opencode_config_path(&workspace)).expect("config");
        assert!(content.contains(r#""X-API-Key": "api-key-secret""#));
        assert!(content.contains(r#""X-Trace": "trace""#));
        assert!(content.contains(r#""oauth": false"#));
        assert!(!content.contains("transport-key"));
    }

    #[test]
    fn provisions_host_session_without_credentials_or_oauth_override() {
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

        OpenCodeAdapter
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(opencode_config_path(&workspace)).expect("config");
        assert!(!content.contains("Authorization"));
        assert!(!content.contains(r#""oauth""#));
    }
}
