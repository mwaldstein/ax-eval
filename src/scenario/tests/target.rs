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
