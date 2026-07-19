pub(crate) mod normalize;

use super::{TargetProvision, ToolAdapter, ToolRunOutput};
use crate::mcp_auth::codex_http_headers;
use crate::scenario::McpAuth;
use crate::scenario::{McpTarget, McpTransport, Scenario, TargetConfig};
use crate::session::SessionRunner;
use crate::target_env::expand_target_env_value;
use crate::target_env::{AgentEnvironment, TargetEnvironment};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct CodexAdapter {
    config_path: Option<PathBuf>,
}

impl CodexAdapter {
    #[cfg(test)]
    fn with_config_path(config_path: PathBuf) -> Self {
        Self {
            config_path: Some(config_path),
        }
    }

    fn config_path(&self) -> anyhow::Result<PathBuf> {
        if let Some(path) = &self.config_path {
            return Ok(path.clone());
        }

        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME is not set; cannot locate Codex config"))?;
        Ok(home.join(".codex").join("config.toml"))
    }
}

impl ToolAdapter for CodexAdapter {
    fn requires_mcp_bearer_env(&self) -> bool {
        true
    }

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
            "OPENAI_BASE_URL",
            "OPENAI_ORGANIZATION",
            "OPENAI_PROJECT",
            "CODEX_HOME",
            "AZURE_OPENAI_API_KEY",
            "AZURE_OPENAI_ENDPOINT",
        ]
    }

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

    fn provision_target(
        &self,
        target: &TargetConfig,
        workspace: &Path,
        target_env: &TargetEnvironment,
    ) -> anyhow::Result<TargetProvision> {
        let Some(target) = target.mcp() else {
            return Ok(TargetProvision::none());
        };

        let config_path = self.config_path()?;
        provision_mcp_target(target, workspace, &config_path, target_env)
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

        let mut args = vec!["exec", "--json", "--full-auto", "--skip-git-repo-check"];
        if let Some(model) = model {
            if model != "default" {
                args.push("--model");
                args.push(model);
            }
        }
        args.push(&scenario.task.prompt);

        let agent_env = agent_env.to_session_env();

        let result = runner.run_command_result_with_projected_env(
            "codex",
            &args,
            cwd,
            timeout_secs,
            &agent_env,
        )?;
        let output = result.output();
        let exit_code = result.exit_code;

        Ok(normalize::normalize(output, exit_code))
    }
}

fn results_dir_for_workspace(workspace: &Path) -> &Path {
    workspace.parent().unwrap_or(workspace)
}

fn provision_mcp_target(
    target: &McpTarget,
    workspace: &Path,
    config_path: &Path,
    target_env: &TargetEnvironment,
) -> anyhow::Result<TargetProvision> {
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
    let previous = match std::fs::read(config_path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let rendered = render_mcp_target(
        target,
        workspace,
        results_dir_for_workspace(workspace),
        target_env,
    )?;
    let next = append_codex_mcp_entry(
        previous.as_deref().map(std::str::from_utf8).transpose()?,
        &target.name,
        &rendered,
    )?;
    std::fs::write(config_path, next)?;

    let config_path = config_path.to_path_buf();
    Ok(TargetProvision::with_cleanup(move || {
        if let Some(previous) = previous {
            std::fs::write(config_path, previous)?;
        } else if config_path.exists() {
            std::fs::remove_file(config_path)?;
        }

        Ok(())
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RenderedCodexMcpTarget {
    Stdio {
        command: String,
        args: Vec<String>,
        env: Option<BTreeMap<String, String>>,
    },
    Http {
        url: String,
        http_headers: Option<BTreeMap<String, String>>,
        bearer_token_env_var: Option<String>,
    },
}

fn render_mcp_target(
    target: &McpTarget,
    fixture_dir: &Path,
    results_dir: &Path,
    target_env: &TargetEnvironment,
) -> anyhow::Result<RenderedCodexMcpTarget> {
    match &target.transport {
        McpTransport::Stdio { command, args } => Ok(RenderedCodexMcpTarget::Stdio {
            command: expand_target_env_value(command, fixture_dir, results_dir)?,
            args: args
                .iter()
                .map(|arg| expand_target_env_value(arg, fixture_dir, results_dir))
                .collect::<anyhow::Result<Vec<_>>>()?,
            env: (!target_env.as_map().is_empty()).then(|| {
                target_env
                    .as_map()
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            }),
        }),
        McpTransport::Http { url, headers } => Ok(RenderedCodexMcpTarget::Http {
            url: expand_target_env_value(url, fixture_dir, results_dir)?,
            http_headers: codex_http_headers(
                headers.as_ref(),
                target.auth.as_ref(),
                fixture_dir,
                results_dir,
            )?,
            bearer_token_env_var: match &target.auth {
                Some(McpAuth::BearerEnv { env }) => Some(env.clone()),
                _ => None,
            },
        }),
    }
}

fn append_codex_mcp_entry(
    existing: Option<&str>,
    name: &str,
    rendered: &RenderedCodexMcpTarget,
) -> anyhow::Result<String> {
    let mut output = existing.unwrap_or("").to_string();
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(&codex_mcp_entry_toml(name, rendered)?);
    Ok(output)
}

fn codex_mcp_entry_toml(name: &str, rendered: &RenderedCodexMcpTarget) -> anyhow::Result<String> {
    let mut output = String::new();
    output.push_str("[mcp_servers.");
    output.push_str(&toml_key(name));
    output.push_str("]\n");

    match rendered {
        RenderedCodexMcpTarget::Stdio { command, args, env } => {
            output.push_str("command = ");
            output.push_str(&toml_string(command));
            output.push('\n');
            output.push_str("args = ");
            output.push_str(&toml_array(args));
            output.push('\n');
            if let Some(env) = env {
                output.push_str("env = ");
                output.push_str(&toml_inline_table(env));
                output.push('\n');
            }
        }
        RenderedCodexMcpTarget::Http {
            url,
            http_headers,
            bearer_token_env_var,
        } => {
            output.push_str("url = ");
            output.push_str(&toml_string(url));
            output.push('\n');
            if let Some(env) = bearer_token_env_var {
                output.push_str("bearer_token_env_var = ");
                output.push_str(&toml_string(env));
                output.push('\n');
            }
            if let Some(headers) = http_headers {
                output.push_str("http_headers = ");
                output.push_str(&toml_inline_table(headers));
                output.push('\n');
            }
        }
    }

    Ok(output)
}

fn toml_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        key.to_string()
    } else {
        toml_string(key)
    }
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn toml_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn toml_inline_table(values: &BTreeMap<String, String>) -> String {
    let values = values
        .iter()
        .map(|(key, value)| format!("{} = {}", toml_key(key), toml_string(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {values} }}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{McpAuth, McpTarget, McpTransport, TargetConfig};
    use std::collections::HashMap;

    #[test]
    fn provisions_stdio_mcp_config_and_restores_existing_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        let config_path = dir.path().join("home").join(".codex").join("config.toml");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(config_path.parent().expect("config parent"))
            .expect("config parent");
        let existing = "# existing comment\nmodel = \"gpt-5-codex\"\n";
        std::fs::write(&config_path, existing).expect("existing config");

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

        let provision = CodexAdapter::with_config_path(config_path.clone())
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let expected = format!(
            r#"# existing comment
model = "gpt-5-codex"

[mcp_servers.todo]
command = "{fixture}/todo-mcp-server"
args = ["--root", "{fixture}"]
env = {{ TODO_DB = "{results}/todo.db" }}
"#,
            fixture = workspace.to_string_lossy(),
            results = workspace.parent().expect("results").to_string_lossy()
        );
        assert_eq!(
            std::fs::read_to_string(&config_path).expect("provisioned config"),
            expected
        );

        provision.cleanup().expect("cleanup");
        assert_eq!(
            std::fs::read_to_string(&config_path).expect("restored config"),
            existing
        );
    }

    #[test]
    fn provisions_http_mcp_config_and_removes_new_file_on_cleanup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        let config_path = dir.path().join("home").join(".codex").join("config.toml");
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

        let provision = CodexAdapter::with_config_path(config_path.clone())
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let expected = format!(
            r#"[mcp_servers.search]
url = "https://mcp.example.com/{results}/mcp"
http_headers = {{ X-API-Key = "token-{fixture}" }}
"#,
            fixture = workspace.to_string_lossy(),
            results = workspace.parent().expect("results").to_string_lossy()
        );
        assert_eq!(
            std::fs::read_to_string(&config_path).expect("provisioned config"),
            expected
        );

        provision.cleanup().expect("cleanup");
        assert!(!config_path.exists());
    }

    #[test]
    fn provisions_bearer_env_name_without_resolved_token() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        let config_path = dir.path().join("home").join(".codex").join("config.toml");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let env_name = "AX_EVAL_CODEX_BEARER_AUTH_UNIQUE";
        std::env::set_var(env_name, "codex-secret-token");
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

        let provision = CodexAdapter::with_config_path(config_path.clone())
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(&config_path).expect("provisioned config");
        assert!(content.contains(&format!(r#"bearer_token_env_var = "{env_name}""#)));
        assert!(!content.contains("codex-secret-token"));
        assert!(!content.contains("transport-token"));

        provision.cleanup().expect("cleanup");
    }

    #[test]
    fn provisions_auth_headers_into_http_headers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        let config_path = dir.path().join("home").join(".codex").join("config.toml");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let env_name = "AX_EVAL_CODEX_HEADER_AUTH_UNIQUE";
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

        let provision = CodexAdapter::with_config_path(config_path.clone())
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(&config_path).expect("provisioned config");
        assert!(content
            .contains(r#"http_headers = { X-API-Key = "api-key-secret", X-Trace = "trace" }"#));
        assert!(!content.contains("transport-key"));

        provision.cleanup().expect("cleanup");
    }

    #[test]
    fn provisions_host_session_without_static_credentials() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("results").join("fixture");
        let config_path = dir.path().join("home").join(".codex").join("config.toml");
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

        let provision = CodexAdapter::with_config_path(config_path.clone())
            .provision_target(&target, &workspace, &TargetEnvironment::default())
            .expect("provision");

        let content = std::fs::read_to_string(&config_path).expect("provisioned config");
        assert!(!content.contains("bearer_token_env_var"));
        assert!(!content.contains("http_headers"));

        provision.cleanup().expect("cleanup");
    }
}
