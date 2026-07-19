use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetEnvironment {
    vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentEnvironment {
    vars: HashMap<String, String>,
}

impl AgentEnvironment {
    pub fn projected(
        required_names: &[&str],
        additional_names: &[String],
        target: &crate::scenario::TargetConfig,
        target_env: &TargetEnvironment,
        include_mcp_bearer_env: bool,
    ) -> Self {
        let mut vars = HashMap::new();
        for name in required_names
            .iter()
            .copied()
            .chain(additional_names.iter().map(String::as_str))
        {
            if let Some(value) = std::env::var_os(name) {
                vars.insert(name.to_string(), value.to_string_lossy().into_owned());
            }
        }

        // CLI targets are launched by the evaluated agent, so their configured
        // environment remains visible. Preserve the same legacy behavior for
        // HTTP MCP targets; stdio target variables are rendered only into the
        // child config.
        if !matches!(
            target,
            crate::scenario::TargetConfig::Mcp(crate::scenario::McpTarget {
                transport: crate::scenario::McpTransport::Stdio { .. },
                ..
            })
        ) {
            vars.extend(target_env.as_map().clone());
        }

        // Codex's MCP configuration refers to bearer credentials by variable
        // name rather than persisting the resolved secret. Keep that declared
        // credential available to the MCP client process under env isolation.
        if include_mcp_bearer_env {
            if let crate::scenario::TargetConfig::Mcp(crate::scenario::McpTarget {
                auth: Some(crate::scenario::McpAuth::BearerEnv { env }),
                ..
            }) = target
            {
                if let Some(value) = std::env::var_os(env) {
                    vars.insert(env.clone(), value.to_string_lossy().into_owned());
                }
            }
        }

        Self { vars }
    }

    #[cfg(test)]
    pub fn as_map(&self) -> &HashMap<String, String> {
        &self.vars
    }

    pub fn to_session_env(&self) -> Vec<(String, String)> {
        self.vars
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

impl TargetEnvironment {
    pub fn expanded_from_config(
        target_env: Option<&HashMap<String, String>>,
        fixture_dir: &Path,
        results_dir: &Path,
    ) -> anyhow::Result<Self> {
        let vars = target_env
            .map(|vars| {
                vars.iter()
                    .map(|(key, value)| {
                        Ok((
                            key.clone(),
                            expand_target_env_value(value, fixture_dir, results_dir)?,
                        ))
                    })
                    .collect::<anyhow::Result<HashMap<_, _>>>()
            })
            .transpose()?
            .unwrap_or_default();

        Ok(Self { vars })
    }

    pub fn as_map(&self) -> &HashMap<String, String> {
        &self.vars
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

pub(crate) fn expand_target_env_value(
    value: &str,
    fixture_dir: &Path,
    results_dir: &Path,
) -> anyhow::Result<String> {
    let fixture_dir = absolute_path(fixture_dir);
    let results_dir = absolute_path(results_dir);
    let fixture_dir = fixture_dir.to_string_lossy();
    let results_dir = results_dir.to_string_lossy();

    let mut expanded = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(start) = rest.find("${") {
        expanded.push_str(&rest[..start]);
        rest = &rest[start..];

        if let Some(after) = rest.strip_prefix("${AX_EVAL_FIXTURE_DIR}") {
            expanded.push_str(&fixture_dir);
            rest = after;
            continue;
        }

        if let Some(after) = rest.strip_prefix("${AX_EVAL_RESULTS_DIR}") {
            expanded.push_str(&results_dir);
            rest = after;
            continue;
        }

        if let Some(after_prefix) = rest.strip_prefix("${env:") {
            if let Some(end) = after_prefix.find('}') {
                let name = &after_prefix[..end];
                if is_env_var_name(name) {
                    let value = std::env::var(name).map_err(|_| {
                        anyhow::anyhow!("environment variable ${{env:{name}}} is not set")
                    })?;
                    expanded.push_str(&value);
                    rest = &after_prefix[end + 1..];
                    continue;
                }
            }
        }

        expanded.push_str("${");
        rest = &rest[2..];
    }

    expanded.push_str(rest);
    Ok(expanded)
}

fn is_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Scenario;

    fn stdio_mcp_scenario(agent_env: &str) -> Scenario {
        yaml_serde::from_str(&format!(
            r#"
name: environment-separation
description: Test environment separation
template_folder: fixture
target:
  kind: mcp
  name: private-server
  transport:
    type: stdio
    command: private-server
  tools: [read]
  env:
    PRIVATE_SERVER_TOKEN: server-secret
task:
  prompt: Read data
evaluation:
  gates: []
agent_env: [{agent_env}]
"#
        ))
        .expect("scenario")
    }

    #[test]
    fn stdio_mcp_projection_excludes_target_private_values() {
        let allowed_name = "AX_EVAL_AGENT_ENV_ALLOWED_UNIQUE";
        std::env::set_var(allowed_name, "agent-visible");
        let scenario = stdio_mcp_scenario(allowed_name);
        let target_env = TargetEnvironment::expanded_from_config(
            scenario.target.env(),
            Path::new("fixture"),
            Path::new("results"),
        )
        .expect("target env");

        let agent_env = AgentEnvironment::projected(
            &["PATH"],
            &scenario.agent_env,
            &scenario.target,
            &target_env,
            false,
        );

        assert_eq!(
            agent_env.as_map().get(allowed_name).map(String::as_str),
            Some("agent-visible")
        );
        assert!(!agent_env.as_map().contains_key("PRIVATE_SERVER_TOKEN"));
        assert!(agent_env.as_map().contains_key("PATH"));
    }

    #[test]
    fn cli_projection_keeps_legacy_target_environment() {
        let target = crate::scenario::TargetConfig::cli_target("tool");
        let target_env = TargetEnvironment {
            vars: HashMap::from([("TOOL_ROOT".to_string(), "fixture-data".to_string())]),
        };

        let agent_env = AgentEnvironment::projected(&[], &[], &target, &target_env, false);

        assert_eq!(
            agent_env.as_map().get("TOOL_ROOT").map(String::as_str),
            Some("fixture-data")
        );
    }

    #[test]
    fn bearer_auth_variable_is_available_to_isolated_mcp_client() {
        let name = "AX_EVAL_AGENT_MCP_BEARER_UNIQUE";
        std::env::set_var(name, "mcp-token");
        let target = crate::scenario::TargetConfig::Mcp(crate::scenario::McpTarget {
            name: "remote".to_string(),
            transport: crate::scenario::McpTransport::Http {
                url: "https://example.invalid/mcp".to_string(),
                headers: None,
            },
            auth: Some(crate::scenario::McpAuth::BearerEnv {
                env: name.to_string(),
            }),
            tools: vec!["read".to_string()],
            env: None,
            health_check: None,
        });

        let agent_env =
            AgentEnvironment::projected(&[], &[], &target, &TargetEnvironment::default(), true);

        assert_eq!(
            agent_env.as_map().get(name).map(String::as_str),
            Some("mcp-token")
        );

        let config_rendering_host =
            AgentEnvironment::projected(&[], &[], &target, &TargetEnvironment::default(), false);
        assert!(!config_rendering_host.as_map().contains_key(name));
    }

    #[test]
    fn expands_fixture_and_results_placeholders() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fixture_dir = dir.path().join("results").join("fixture");
        let results_dir = dir.path().join("results");

        let mut target_env = HashMap::new();
        target_env.insert(
            "MYTOOL_ROOT_DIR".to_string(),
            "${AX_EVAL_FIXTURE_DIR}".to_string(),
        );
        target_env.insert(
            "MYTOOL_EXPORT".to_string(),
            "${AX_EVAL_RESULTS_DIR}/export.json".to_string(),
        );

        let expanded =
            TargetEnvironment::expanded_from_config(Some(&target_env), &fixture_dir, &results_dir)
                .expect("target env");

        assert_eq!(
            expanded.as_map().get("MYTOOL_ROOT_DIR"),
            Some(&fixture_dir.to_string_lossy().to_string())
        );
        assert_eq!(
            expanded.as_map().get("MYTOOL_EXPORT"),
            Some(&format!("{}/export.json", results_dir.to_string_lossy()))
        );
    }

    #[test]
    fn expands_relative_fixture_and_results_placeholders_to_absolute_paths() {
        let unique = format!(
            "target/target-env-abs-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let results_dir = Path::new(&unique).join("results");
        let fixture_dir = results_dir.join("fixture");
        std::fs::create_dir_all(&fixture_dir).expect("create fixture dir");

        let mut target_env = HashMap::new();
        target_env.insert(
            "MYTOOL_ROOT_DIR".to_string(),
            "${AX_EVAL_FIXTURE_DIR}".to_string(),
        );
        target_env.insert(
            "MYTOOL_EXPORT".to_string(),
            "${AX_EVAL_RESULTS_DIR}/export.json".to_string(),
        );

        let expanded =
            TargetEnvironment::expanded_from_config(Some(&target_env), &fixture_dir, &results_dir)
                .expect("target env");

        let expected_fixture = std::env::current_dir()
            .expect("current dir")
            .join(&fixture_dir);
        let expected_results = std::env::current_dir()
            .expect("current dir")
            .join(&results_dir);

        assert_eq!(
            expanded.as_map().get("MYTOOL_ROOT_DIR"),
            Some(&expected_fixture.to_string_lossy().to_string())
        );
        assert_eq!(
            expanded.as_map().get("MYTOOL_EXPORT"),
            Some(&format!(
                "{}/export.json",
                expected_results.to_string_lossy()
            ))
        );
    }

    #[test]
    fn expands_namespaced_env_var() {
        let name = "AX_EVAL_TARGET_ENV_TEST_SET_UNIQUE";
        std::env::set_var(name, "secret-value");

        let expanded = expand_target_env_value(
            "token-${env:AX_EVAL_TARGET_ENV_TEST_SET_UNIQUE}",
            Path::new("fixture"),
            Path::new("results"),
        )
        .expect("expand env");

        assert_eq!(expanded, "token-secret-value");
    }

    #[test]
    fn unset_namespaced_env_var_returns_error_naming_variable() {
        let name = "AX_EVAL_TARGET_ENV_TEST_UNSET_UNIQUE";
        std::env::remove_var(name);

        let error = expand_target_env_value(
            "token-${env:AX_EVAL_TARGET_ENV_TEST_UNSET_UNIQUE}",
            Path::new("fixture"),
            Path::new("results"),
        )
        .expect_err("missing env should fail");

        assert_eq!(
            error.to_string(),
            "environment variable ${env:AX_EVAL_TARGET_ENV_TEST_UNSET_UNIQUE} is not set"
        );
    }

    #[test]
    fn bare_env_like_placeholder_is_left_literal() {
        let expanded = expand_target_env_value(
            "token-${AX_EVAL_TARGET_ENV_TEST_BARE}",
            Path::new("fixture"),
            Path::new("results"),
        )
        .expect("expand env");

        assert_eq!(expanded, "token-${AX_EVAL_TARGET_ENV_TEST_BARE}");
    }

    #[test]
    fn expands_mixed_run_dir_and_namespaced_env_values() {
        let name = "AX_EVAL_TARGET_ENV_TEST_MIXED_UNIQUE";
        std::env::set_var(name, "artifact.json");

        let dir = tempfile::tempdir().expect("tempdir");
        let fixture_dir = dir.path().join("results").join("fixture");
        let results_dir = dir.path().join("results");

        let expanded = expand_target_env_value(
            "${AX_EVAL_RESULTS_DIR}/${env:AX_EVAL_TARGET_ENV_TEST_MIXED_UNIQUE}",
            &fixture_dir,
            &results_dir,
        )
        .expect("expand mixed");

        assert_eq!(
            expanded,
            format!("{}/artifact.json", results_dir.to_string_lossy())
        );
    }
}
