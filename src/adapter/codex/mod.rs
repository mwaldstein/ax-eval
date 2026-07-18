pub(crate) mod normalize;

use super::{TargetProvision, ToolAdapter, ToolRunOutput};
use crate::scenario::{McpTarget, McpTransport, Scenario, TargetConfig};
use crate::session::SessionRunner;
use crate::target_env::expand_target_env_value;
use crate::target_env::TargetEnvironment;
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
    ) -> anyhow::Result<TargetProvision> {
        let Some(target) = target.mcp() else {
            return Ok(TargetProvision::none());
        };

        let config_path = self.config_path()?;
        provision_mcp_target(target, workspace, &config_path)
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

fn results_dir_for_workspace(workspace: &Path) -> &Path {
    workspace.parent().unwrap_or(workspace)
}

fn provision_mcp_target(
    target: &McpTarget,
    workspace: &Path,
    config_path: &Path,
) -> anyhow::Result<TargetProvision> {
    let previous = match std::fs::read(config_path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let rendered = render_mcp_target(target, workspace, results_dir_for_workspace(workspace));
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
    },
}

fn render_mcp_target(
    target: &McpTarget,
    fixture_dir: &Path,
    results_dir: &Path,
) -> RenderedCodexMcpTarget {
    match &target.transport {
        McpTransport::Stdio { command, args } => RenderedCodexMcpTarget::Stdio {
            command: expand_target_env_value(command, fixture_dir, results_dir),
            args: args
                .iter()
                .map(|arg| expand_target_env_value(arg, fixture_dir, results_dir))
                .collect(),
            env: target.env.as_ref().map(|env| {
                env.iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            expand_target_env_value(value, fixture_dir, results_dir),
                        )
                    })
                    .collect()
            }),
        },
        McpTransport::Http { url, headers } => RenderedCodexMcpTarget::Http {
            url: expand_target_env_value(url, fixture_dir, results_dir),
            http_headers: headers.as_ref().map(|headers| {
                headers
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            expand_target_env_value(value, fixture_dir, results_dir),
                        )
                    })
                    .collect()
            }),
        },
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
        RenderedCodexMcpTarget::Http { url, http_headers } => {
            output.push_str("url = ");
            output.push_str(&toml_string(url));
            output.push('\n');
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
    use crate::scenario::{McpTarget, McpTransport, TargetConfig};
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
            tools: vec!["add".to_string()],
            env: Some(HashMap::from([(
                "TODO_DB".to_string(),
                "${AX_EVAL_RESULTS_DIR}/todo.db".to_string(),
            )])),
            health_check: None,
        });

        let provision = CodexAdapter::with_config_path(config_path.clone())
            .provision_target(&target, &workspace)
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
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        let provision = CodexAdapter::with_config_path(config_path.clone())
            .provision_target(&target, &workspace)
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
}
