use crate::scenario::{McpAuth, McpTransport, TargetConfig};
use crate::target_env::expand_target_env_value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StaticAuthMode {
    None,
    Static,
    HostSession,
}

pub(crate) fn static_auth_mode(auth: Option<&McpAuth>) -> StaticAuthMode {
    match auth {
        Some(McpAuth::BearerEnv { .. } | McpAuth::Headers { .. }) => StaticAuthMode::Static,
        Some(McpAuth::HostSession) => StaticAuthMode::HostSession,
        Some(McpAuth::None) | None => StaticAuthMode::None,
    }
}

pub(crate) fn merged_http_headers(
    transport_headers: Option<&std::collections::HashMap<String, String>>,
    auth: Option<&McpAuth>,
    fixture_dir: &Path,
    results_dir: &Path,
) -> anyhow::Result<Option<BTreeMap<String, String>>> {
    let mut headers = BTreeMap::new();

    if let Some(transport_headers) = transport_headers {
        for (key, value) in transport_headers {
            headers.insert(
                key.clone(),
                expand_target_env_value(value, fixture_dir, results_dir)?,
            );
        }
    }

    match auth {
        Some(McpAuth::BearerEnv { env }) => {
            let token = std::env::var(env).map_err(|_| {
                anyhow::anyhow!("environment variable {env} required by target.auth.env is not set")
            })?;
            if token.is_empty() {
                anyhow::bail!("environment variable {env} required by target.auth.env is empty");
            }
            insert_header_override(
                &mut headers,
                "Authorization".to_string(),
                format!("Bearer {token}"),
            );
        }
        Some(McpAuth::Headers {
            headers: auth_headers,
        }) => {
            for (key, value) in auth_headers {
                insert_header_override(
                    &mut headers,
                    key.clone(),
                    expand_target_env_value(value, fixture_dir, results_dir)?,
                );
            }
        }
        Some(McpAuth::None | McpAuth::HostSession) | None => {}
    }

    Ok((!headers.is_empty()).then_some(headers))
}

pub(crate) fn codex_http_headers(
    transport_headers: Option<&std::collections::HashMap<String, String>>,
    auth: Option<&McpAuth>,
    fixture_dir: &Path,
    results_dir: &Path,
) -> anyhow::Result<Option<BTreeMap<String, String>>> {
    let mut headers = BTreeMap::new();

    if let Some(transport_headers) = transport_headers {
        for (key, value) in transport_headers {
            if matches!(auth, Some(McpAuth::BearerEnv { .. }))
                && key.eq_ignore_ascii_case("authorization")
            {
                continue;
            }
            headers.insert(
                key.clone(),
                expand_target_env_value(value, fixture_dir, results_dir)?,
            );
        }
    }

    if let Some(McpAuth::Headers {
        headers: auth_headers,
    }) = auth
    {
        for (key, value) in auth_headers {
            insert_header_override(
                &mut headers,
                key.clone(),
                expand_target_env_value(value, fixture_dir, results_dir)?,
            );
        }
    }

    Ok((!headers.is_empty()).then_some(headers))
}

fn insert_header_override(headers: &mut BTreeMap<String, String>, key: String, value: String) {
    headers.retain(|existing, _| !existing.eq_ignore_ascii_case(&key));
    headers.insert(key, value);
}

pub(crate) fn preflight_auth(target: &TargetConfig) -> anyhow::Result<()> {
    let Some(mcp) = target.mcp() else {
        return Ok(());
    };

    let McpTransport::Http { headers, .. } = &mcp.transport else {
        return Ok(());
    };

    if matches!(mcp.auth, Some(McpAuth::HostSession)) {
        eprintln!(
            "warning: target.auth host_session is not preflighted yet; the run may fail on the first MCP tool call if the host is not already logged in"
        );
    }

    if let Some(McpAuth::BearerEnv { env }) = &mcp.auth {
        require_non_empty_env(env, "target.auth.env")?;
    }

    if let Some(headers) = headers {
        for value in headers.values() {
            require_referenced_env_vars(value)?;
        }
    }

    if let Some(McpAuth::Headers { headers }) = &mcp.auth {
        for value in headers.values() {
            require_referenced_env_vars(value)?;
        }
    }

    Ok(())
}

/// Resolved secret strings that static auth (and http transport headers) inject
/// into provisioned host config. Used to scrub the exact values from retained
/// workspace config files — this catches custom-named auth headers that the
/// pattern-based `redact_sensitive` would miss. Returns nothing for stdio,
/// host_session, or unauthenticated targets.
pub(crate) fn resolved_auth_secrets(target: &TargetConfig) -> Vec<String> {
    let mut secrets = Vec::new();
    let Some(mcp) = target.mcp() else {
        return secrets;
    };
    let McpTransport::Http { headers, .. } = &mcp.transport else {
        return secrets;
    };

    if let Some(headers) = headers {
        for value in headers.values() {
            collect_env_values(value, &mut secrets);
        }
    }

    match &mcp.auth {
        Some(McpAuth::BearerEnv { env }) => {
            if let Ok(value) = std::env::var(env) {
                if !value.is_empty() {
                    secrets.push(value);
                }
            }
        }
        Some(McpAuth::Headers { headers }) => {
            for value in headers.values() {
                collect_env_values(value, &mut secrets);
            }
        }
        _ => {}
    }

    secrets
}

fn collect_env_values(value: &str, out: &mut Vec<String>) {
    for name in referenced_env_vars(value) {
        if let Ok(resolved) = std::env::var(&name) {
            if !resolved.is_empty() {
                out.push(resolved);
            }
        }
    }
}

fn require_referenced_env_vars(value: &str) -> anyhow::Result<()> {
    for name in referenced_env_vars(value) {
        require_non_empty_env(&name, "${env:NAME}")?;
    }
    Ok(())
}

fn require_non_empty_env(name: &str, source: &str) -> anyhow::Result<()> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(()),
        Ok(_) => anyhow::bail!("environment variable {name} referenced by {source} is empty"),
        Err(_) => anyhow::bail!("environment variable {name} referenced by {source} is not set"),
    }
}

fn referenced_env_vars(value: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = value;

    while let Some(start) = rest.find("${env:") {
        rest = &rest[start + "${env:".len()..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let name = &rest[..end];
        if is_env_var_name(name) {
            names.push(name.to_string());
        }
        rest = &rest[end + 1..];
    }

    names
}

fn is_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{McpTarget, TargetConfig};
    use std::collections::HashMap;

    fn http_target(auth: Option<McpAuth>) -> TargetConfig {
        TargetConfig::Mcp(McpTarget {
            name: "search".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.example.com/mcp".to_string(),
                headers: None,
            },
            auth,
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        })
    }

    #[test]
    fn preflight_rejects_unset_bearer_env_before_run() {
        let name = "AX_EVAL_AUTH_PREFLIGHT_UNSET_UNIQUE";
        std::env::remove_var(name);

        let err = preflight_auth(&http_target(Some(McpAuth::BearerEnv {
            env: name.to_string(),
        })))
        .expect_err("missing bearer env should fail");

        assert!(err.to_string().contains(name));
        assert!(err.to_string().contains("not set"));
    }

    #[test]
    fn preflight_rejects_empty_header_env_reference_before_run() {
        let name = "AX_EVAL_AUTH_PREFLIGHT_EMPTY_UNIQUE";
        std::env::set_var(name, "");

        let err = preflight_auth(&http_target(Some(McpAuth::Headers {
            headers: HashMap::from([("X-API-Key".to_string(), format!("${{env:{name}}}"))]),
        })))
        .expect_err("empty env should fail");

        assert!(err.to_string().contains(name));
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn preflight_accepts_set_static_auth_env_vars() {
        let bearer = "AX_EVAL_AUTH_PREFLIGHT_SET_BEARER_UNIQUE";
        let header = "AX_EVAL_AUTH_PREFLIGHT_SET_HEADER_UNIQUE";
        std::env::set_var(bearer, "bearer-token");
        std::env::set_var(header, "header-token");

        let target = TargetConfig::Mcp(McpTarget {
            name: "search".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.example.com/mcp".to_string(),
                headers: Some(HashMap::from([(
                    "X-Static".to_string(),
                    format!("static-${{env:{header}}}"),
                )])),
            },
            auth: Some(McpAuth::BearerEnv {
                env: bearer.to_string(),
            }),
            tools: vec!["query".to_string()],
            env: None,
            health_check: None,
        });

        preflight_auth(&target).expect("set env vars pass preflight");
    }
}
