use super::super::*;
use crate::scenario::validation::validate_scenario_file;

fn yaml_with_target(target: &str) -> String {
    format!(
        r#"
name: target_test
description: "Target schema test"
template_folder: qipu
target:
{target}
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#
    )
}

fn write_scenario(dir: &tempfile::TempDir, target: &str) -> std::path::PathBuf {
    let path = dir.path().join("scenario.yaml");
    std::fs::write(&path, yaml_with_target(target)).expect("write scenario");
    path
}

#[test]
fn legacy_flat_target_coerces_to_cli() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  binary: qipu
  command_pattern: "qipu\\s+(\\S+)"
  health_check: "qipu --version"
  env:
    QIPU_HOME: "${AX_EVAL_FIXTURE_DIR}"
"#,
    ))
    .expect("parse legacy cli target");

    let cli = scenario.target.cli().expect("cli target");
    assert_eq!(cli.binary, "qipu");
    assert_eq!(cli.command_pattern.as_deref(), Some("qipu\\s+(\\S+)"));
    assert_eq!(cli.health_check.as_deref(), Some("qipu --version"));
    assert_eq!(
        cli.env.as_ref().and_then(|env| env.get("QIPU_HOME")),
        Some(&"${AX_EVAL_FIXTURE_DIR}".to_string())
    );
}

#[test]
fn explicit_cli_target_parses() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: cli
  binary: qipu
"#,
    ))
    .expect("parse explicit cli target");

    assert_eq!(
        scenario.target.cli().map(|target| target.binary.as_str()),
        Some("qipu")
    );
    assert_eq!(scenario.target.display_name(), "qipu");
}

#[test]
fn stdio_mcp_target_parses() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: mcp
  name: todo
  transport:
    type: stdio
    command: "${AX_EVAL_FIXTURE_DIR}/todo-mcp-server"
    args: ["--root", "${AX_EVAL_FIXTURE_DIR}"]
  tools: [add, list, complete, search]
  env:
    TODO_DB: "${AX_EVAL_FIXTURE_DIR}/todo.db"
  health_check: "./probe-todo.sh"
"#,
    ))
    .expect("parse stdio mcp target");

    let mcp = scenario.target.mcp().expect("mcp target");
    assert_eq!(mcp.name, "todo");
    assert_eq!(mcp.tools, vec!["add", "list", "complete", "search"]);
    assert_eq!(scenario.target.display_name(), "todo");
    assert_eq!(scenario.target.health_check(), Some("./probe-todo.sh"));
    assert!(matches!(
        &mcp.transport,
        McpTransport::Stdio { command, args }
            if command == "${AX_EVAL_FIXTURE_DIR}/todo-mcp-server"
                && args == &vec!["--root".to_string(), "${AX_EVAL_FIXTURE_DIR}".to_string()]
    ));
}

#[test]
fn http_mcp_target_with_headers_parses() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
    headers:
      X-API-Key: "${env:SEARCH_API_KEY}"
  tools: [query, index]
"#,
    ))
    .expect("parse http mcp target");

    let mcp = scenario.target.mcp().expect("mcp target");
    assert!(matches!(
        &mcp.transport,
        McpTransport::Http { url, headers }
            if url == "https://mcp.example.com/mcp"
                && headers
                    .as_ref()
                    .and_then(|headers| headers.get("X-API-Key"))
                    == Some(&"${env:SEARCH_API_KEY}".to_string())
    ));
}

#[test]
fn mcp_auth_can_be_omitted() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  tools: [query]
"#,
    ))
    .expect("parse mcp target without auth");

    let mcp = scenario.target.mcp().expect("mcp target");
    assert!(mcp.auth.is_none());
}

#[test]
fn mcp_auth_none_parses() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: none
  tools: [query]
"#,
    ))
    .expect("parse none auth");

    let mcp = scenario.target.mcp().expect("mcp target");
    assert!(matches!(mcp.auth, Some(McpAuth::None)));
}

#[test]
fn mcp_auth_bearer_env_parses() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: bearer_env
    env: MY_MCP_TOKEN
  tools: [query]
"#,
    ))
    .expect("parse bearer env auth");

    let mcp = scenario.target.mcp().expect("mcp target");
    assert!(matches!(
        &mcp.auth,
        Some(McpAuth::BearerEnv { env }) if env == "MY_MCP_TOKEN"
    ));
}

#[test]
fn mcp_auth_headers_parses() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: headers
    headers:
      X-API-Key: "${env:SEARCH_API_KEY}"
  tools: [query]
"#,
    ))
    .expect("parse headers auth");

    let mcp = scenario.target.mcp().expect("mcp target");
    assert!(matches!(
        &mcp.auth,
        Some(McpAuth::Headers { headers })
            if headers.get("X-API-Key") == Some(&"${env:SEARCH_API_KEY}".to_string())
    ));
}

#[test]
fn mcp_auth_host_session_parses() {
    let scenario: Scenario = yaml_serde::from_str(&yaml_with_target(
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: host_session
  tools: [query]
"#,
    ))
    .expect("parse host session auth");

    let mcp = scenario.target.mcp().expect("mcp target");
    assert!(matches!(mcp.auth, Some(McpAuth::HostSession)));
}

#[test]
fn unknown_target_kind_is_rejected_with_friendly_message() {
    let err = yaml_serde::from_str::<Scenario>(&yaml_with_target(
        r#"  kind: mpc
  name: todo
  transport:
    type: stdio
    command: todo-mcp
  tools: [add]
"#,
    ))
    .expect_err("unknown target kind should fail")
    .to_string();

    assert!(
        err.contains("unknown target kind: mpc, did you mean mcp?"),
        "{err}"
    );
}

#[test]
fn missing_mcp_tools_is_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: todo
  transport:
    type: stdio
    command: todo-mcp
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("missing tools should fail")
        .to_string();

    assert!(
        err.contains("target.tools is required for MCP targets"),
        "{err}"
    );
}

#[test]
fn empty_mcp_tools_is_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: todo
  transport:
    type: stdio
    command: todo-mcp
  tools: []
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("empty tools should fail")
        .to_string();

    assert!(
        err.contains("target.tools must contain at least one MCP tool"),
        "{err}"
    );
}

#[test]
fn missing_mcp_transport_is_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: todo
  tools: [add]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("missing transport should fail")
        .to_string();

    assert!(
        err.contains("target.transport is required for MCP targets"),
        "{err}"
    );
}

#[test]
fn empty_stdio_command_is_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: todo
  transport:
    type: stdio
    command: "  "
  tools: [add]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("empty stdio command should fail")
        .to_string();

    assert!(
        err.contains("target.transport.command cannot be empty for stdio MCP targets"),
        "{err}"
    );
}

#[test]
fn invalid_http_url_is_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "ftp://mcp.example.com/mcp"
  tools: [query]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("invalid http url should fail")
        .to_string();

    assert!(
        err.contains("target.transport.url must be a non-empty http:// or https:// URL"),
        "{err}"
    );
}

#[test]
fn auth_on_stdio_mcp_target_is_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: todo
  transport:
    type: stdio
    command: todo-mcp
  auth:
    type: bearer_env
    env: MY_MCP_TOKEN
  tools: [add]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("stdio auth should fail")
        .to_string();

    assert!(
        err.contains(
            "stdio MCP servers authenticate via `env`, not `auth`; move credentials to target.env"
        ),
        "{err}"
    );
}

#[test]
fn empty_bearer_env_is_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: bearer_env
    env: " "
  tools: [query]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("empty bearer env should fail")
        .to_string();

    assert!(err.contains("target.auth.env cannot be empty"), "{err}");
}

#[test]
fn empty_auth_headers_are_rejected_with_friendly_validation_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: headers
    headers: {}
  tools: [query]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("empty auth headers should fail")
        .to_string();

    assert!(
        err.contains("target.auth.headers must contain at least one header"),
        "{err}"
    );
}

#[test]
fn literal_secret_bearer_env_is_rejected_with_guidance_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: bearer_env
    env: sk-testtoken
  tools: [query]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("literal bearer secret should fail")
        .to_string();

    assert!(
        err.contains(
            "looks like a literal secret; reference an environment variable with ${env:NAME} instead"
        ),
        "{err}"
    );
}

#[test]
fn literal_secret_header_value_is_rejected_with_guidance_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: headers
    headers:
      Authorization: "Bearer ghp_1234567890abcdef"
  tools: [query]
"#,
    );

    let err = validate_scenario_file(&path)
        .expect_err("literal header secret should fail")
        .to_string();

    assert!(
        err.contains(
            "looks like a literal secret; reference an environment variable with ${env:NAME} instead"
        ),
        "{err}"
    );
}

#[test]
fn valid_bearer_env_and_env_reference_header_are_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bearer_path = write_scenario(
        &dir,
        r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: bearer_env
    env: MY_MCP_TOKEN
  tools: [query]
"#,
    );
    validate_scenario_file(&bearer_path).expect("bare env name should be valid");

    let header_path = dir.path().join("scenario_headers.yaml");
    std::fs::write(
        &header_path,
        yaml_with_target(
            r#"  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: headers
    headers:
      Authorization: "Bearer ${env:MY_MCP_TOKEN}"
      Accept: "application/json"
  tools: [query]
"#,
        ),
    )
    .expect("write header scenario");

    validate_scenario_file(&header_path).expect("env reference header should be valid");
}
