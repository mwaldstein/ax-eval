use crate::mcp_auth::merged_http_headers;
use crate::scenario::{McpTarget, McpTransport};
use crate::target_env::{expand_target_env_value, TargetEnvironment};
use anyhow::Context;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const PROTOCOL_VERSION: &str = "2025-06-18";
const SERVER_BASE_ENV: &[&str] = &[
    "HOME", "PATH", "USER", "LOGNAME", "SHELL", "TMPDIR", "TMP", "TEMP", "LANG", "LC_ALL",
    "LC_CTYPE",
];

pub(crate) fn inspect_tools(
    target: &McpTarget,
    fixture_dir: &Path,
    results_dir: &Path,
    target_env: &TargetEnvironment,
    timeout: Duration,
) -> anyhow::Result<Value> {
    match &target.transport {
        McpTransport::Stdio { command, args } => {
            inspect_stdio(command, args, fixture_dir, results_dir, target_env, timeout)
        }
        McpTransport::Http { url, headers } => inspect_http(
            target,
            url,
            headers.as_ref(),
            fixture_dir,
            results_dir,
            timeout,
        ),
    }
    .with_context(|| {
        format!(
            "failed to inspect MCP target '{}' with tools/list",
            target.name
        )
    })
}

pub(crate) fn validate_declared_tools(target: &McpTarget, response: &Value) -> anyhow::Result<()> {
    if let Some(error) = response.get("error") {
        anyhow::bail!(
            "MCP target '{}' tools/list returned an error: {error}",
            target.name
        );
    }

    let tools = response
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "MCP target '{}' tools/list response is missing result.tools",
                target.name
            )
        })?;
    let advertised = tools
        .iter()
        .map(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("tools/list returned a tool without a string name"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let unknown = target
        .tools
        .iter()
        .filter(|declared| !advertised.contains(&declared.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    if !unknown.is_empty() {
        let advertised = if advertised.is_empty() {
            "(none)".to_string()
        } else {
            advertised.join(", ")
        };
        anyhow::bail!(
            "MCP target '{}' declares unknown target.tools: {}. Server advertised: {}. Update target.tools or fix the server's tools/list response",
            target.name,
            unknown.join(", "),
            advertised
        );
    }

    Ok(())
}

fn initialize_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "ax-eval", "version": env!("CARGO_PKG_VERSION") }
        }
    })
}

fn initialized_notification() -> Value {
    json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
}

fn tools_list_request(id: u64, cursor: Option<&str>) -> Value {
    let params = cursor.map_or_else(|| json!({}), |cursor| json!({"cursor": cursor}));
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/list", "params": params})
}

fn inspect_stdio(
    command: &str,
    args: &[String],
    fixture_dir: &Path,
    results_dir: &Path,
    target_env: &TargetEnvironment,
    timeout: Duration,
) -> anyhow::Result<Value> {
    let command = expand_target_env_value(command, fixture_dir, results_dir)?;
    let args = args
        .iter()
        .map(|arg| expand_target_env_value(arg, fixture_dir, results_dir))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut child_command = Command::new(&command);
    child_command.env_clear();
    for name in SERVER_BASE_ENV {
        if let Some(value) = std::env::var_os(name) {
            child_command.env(name, value);
        }
    }
    let child = child_command
        .args(&args)
        .current_dir(fixture_dir)
        .envs(target_env.as_map())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("could not start stdio MCP server command '{command}'"))?;
    let mut client = StdioClient::new(child)?;

    client.send(&initialize_request())?;
    let initialize = client.receive_response(1, timeout)?;
    ensure_jsonrpc_success("initialize", &initialize)?;
    client.send(&initialized_notification())?;
    let mut pages = Vec::new();
    let mut cursor = None;
    let mut seen_cursors = HashSet::new();
    let mut id = 2;
    loop {
        client.send(&tools_list_request(id, cursor.as_deref()))?;
        let response = client.receive_response(id, timeout)?;
        ensure_jsonrpc_success("tools/list", &response)?;
        let next = next_cursor(&response)?;
        pages.push(response);
        let Some(next) = next else {
            break;
        };
        if !seen_cursors.insert(next.clone()) {
            anyhow::bail!("MCP tools/list repeated nextCursor: {next}");
        }
        cursor = Some(next);
        id += 1;
    }
    combine_tools_list_pages(pages)
}

struct StdioClient {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<anyhow::Result<Value>>,
}

impl StdioClient {
    fn new(mut child: Child) -> anyhow::Result<Self> {
        let stdin = child
            .stdin
            .take()
            .context("stdio MCP server stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("stdio MCP server stdout unavailable")?;
        let stderr = child.stderr.take();
        let (tx, responses) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let parsed = line
                    .context("could not read stdio MCP response")
                    .and_then(|line| {
                        serde_json::from_str(&line)
                            .with_context(|| format!("invalid JSON from stdio MCP server: {line}"))
                    });
                if tx.send(parsed).is_err() {
                    break;
                }
            }
        });
        if let Some(mut stderr) = stderr {
            std::thread::spawn(move || {
                let mut sink = std::io::sink();
                let _ = std::io::copy(&mut stderr, &mut sink);
            });
        }
        Ok(Self {
            child,
            stdin,
            responses,
        })
    }

    fn send(&mut self, message: &Value) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.stdin, message)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn receive_response(&self, id: u64, timeout: Duration) -> anyhow::Result<Value> {
        loop {
            let response =
                self.responses
                    .recv_timeout(timeout)
                    .map_err(|error| match error {
                        mpsc::RecvTimeoutError::Timeout => {
                            anyhow::anyhow!("timed out waiting for MCP response id {id}")
                        }
                        mpsc::RecvTimeoutError::Disconnected => {
                            anyhow::anyhow!(
                                "stdio MCP server closed stdout before response id {id}"
                            )
                        }
                    })??;
            if response.get("id").and_then(Value::as_u64) == Some(id) {
                return Ok(response);
            }
        }
    }
}

impl Drop for StdioClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn inspect_http(
    target: &McpTarget,
    url: &str,
    transport_headers: Option<&std::collections::HashMap<String, String>>,
    fixture_dir: &Path,
    results_dir: &Path,
    timeout: Duration,
) -> anyhow::Result<Value> {
    let url = expand_target_env_value(url, fixture_dir, results_dir)?;
    let headers = merged_http_headers(
        transport_headers,
        target.auth.as_ref(),
        fixture_dir,
        results_dir,
    )?
    .unwrap_or_default();
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();

    let (initialize, session_id) = http_post(
        &agent,
        &url,
        &headers,
        None,
        None,
        &initialize_request(),
        true,
    )?;
    ensure_jsonrpc_success("initialize", &initialize)?;
    let protocol_version = initialize
        .pointer("/result/protocolVersion")
        .and_then(Value::as_str)
        .context("MCP initialize response is missing result.protocolVersion")?;
    http_post(
        &agent,
        &url,
        &headers,
        session_id.as_deref(),
        Some(protocol_version),
        &initialized_notification(),
        false,
    )?;
    let mut pages = Vec::new();
    let mut cursor = None;
    let mut seen_cursors = HashSet::new();
    let mut id = 2;
    loop {
        let (response, _) = http_post(
            &agent,
            &url,
            &headers,
            session_id.as_deref(),
            Some(protocol_version),
            &tools_list_request(id, cursor.as_deref()),
            true,
        )?;
        ensure_jsonrpc_success("tools/list", &response)?;
        let next = next_cursor(&response)?;
        pages.push(response);
        let Some(next) = next else {
            break;
        };
        if !seen_cursors.insert(next.clone()) {
            anyhow::bail!("MCP tools/list repeated nextCursor: {next}");
        }
        cursor = Some(next);
        id += 1;
    }
    combine_tools_list_pages(pages)
}

fn next_cursor(response: &Value) -> anyhow::Result<Option<String>> {
    match response.pointer("/result/nextCursor") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(cursor)) if !cursor.is_empty() => Ok(Some(cursor.clone())),
        Some(_) => anyhow::bail!("MCP tools/list result.nextCursor must be a non-empty string"),
    }
}

fn combine_tools_list_pages(pages: Vec<Value>) -> anyhow::Result<Value> {
    let Some(first) = pages.first() else {
        anyhow::bail!("MCP tools/list returned no pages");
    };
    let mut combined = first.clone();
    let mut tools = Vec::new();
    for page in &pages {
        let page_tools = page
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .context("MCP tools/list response is missing result.tools")?;
        tools.extend(page_tools.iter().cloned());
    }
    let result = combined
        .get_mut("result")
        .and_then(Value::as_object_mut)
        .context("MCP tools/list response result must be an object")?;
    result.insert("tools".to_string(), Value::Array(tools));
    result.remove("nextCursor");
    if pages.len() > 1 {
        combined
            .as_object_mut()
            .context("MCP tools/list response must be an object")?
            .insert("axEvalToolsListPages".to_string(), Value::Array(pages));
    }
    Ok(combined)
}

fn http_post(
    agent: &ureq::Agent,
    url: &str,
    headers: &std::collections::BTreeMap<String, String>,
    session_id: Option<&str>,
    protocol_version: Option<&str>,
    body: &Value,
    expect_body: bool,
) -> anyhow::Result<(Value, Option<String>)> {
    let mut request = agent
        .post(url)
        .set("Accept", "application/json, text/event-stream")
        .set("Content-Type", "application/json");
    for (name, value) in headers {
        request = request.set(name, value);
    }
    if let Some(session_id) = session_id {
        request = request.set("Mcp-Session-Id", session_id);
    }
    if let Some(protocol_version) = protocol_version {
        request = request.set("MCP-Protocol-Version", protocol_version);
    }

    let encoded_body = serde_json::to_string(body)?;
    let response = request
        .send_string(&encoded_body)
        .map_err(|error| match error {
            ureq::Error::Status(status, response) => {
                let detail = response.into_string().unwrap_or_default();
                anyhow::anyhow!(
                    "MCP HTTP request failed with status {status}: {}",
                    detail.trim()
                )
            }
            ureq::Error::Transport(error) => anyhow::anyhow!("MCP HTTP request failed: {error}"),
        })?;
    let session_id = response.header("Mcp-Session-Id").map(str::to_string);
    if !expect_body {
        return Ok((Value::Null, session_id));
    }
    let content_type = response.header("Content-Type").unwrap_or("").to_string();
    let mut content = String::new();
    response.into_reader().read_to_string(&mut content)?;
    let value = if content_type.starts_with("text/event-stream") {
        parse_sse_json(&content)?
    } else {
        serde_json::from_str(&content)
            .with_context(|| format!("invalid JSON from MCP HTTP server: {content}"))?
    };
    Ok((value, session_id))
}

fn parse_sse_json(body: &str) -> anyhow::Result<Value> {
    let normalized = body.replace("\r\n", "\n");
    for event in normalized.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if !data.is_empty() {
            return serde_json::from_str(&data)
                .with_context(|| format!("invalid JSON in MCP SSE response: {data}"));
        }
    }
    anyhow::bail!("MCP SSE response did not contain a data event")
}

fn ensure_jsonrpc_success(method: &str, response: &Value) -> anyhow::Result<()> {
    if let Some(error) = response.get("error") {
        anyhow::bail!("MCP {method} returned an error: {error}");
    }
    if response.get("result").is_none() {
        anyhow::bail!("MCP {method} response is missing result");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::McpAuth;
    use std::collections::HashMap;
    use std::net::TcpListener;

    fn target(transport: McpTransport, tools: &[&str]) -> McpTarget {
        McpTarget {
            name: "inventory".to_string(),
            transport,
            auth: None,
            tools: tools.iter().map(|tool| (*tool).to_string()).collect(),
            env: None,
            health_check: None,
        }
    }

    #[test]
    fn validates_declared_tools_against_advertised_names() {
        let target = target(
            McpTransport::Stdio {
                command: "unused".to_string(),
                args: vec![],
            },
            &["lookup", "missing"],
        );
        let response = json!({"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"lookup"}]}});

        let error = validate_declared_tools(&target, &response).expect_err("unknown tool");

        assert!(error.to_string().contains("unknown target.tools: missing"));
        assert!(error.to_string().contains("Server advertised: lookup"));
    }

    #[test]
    fn inspects_stdio_with_expanded_args_and_target_environment() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = r#"read init
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"test","version":"1"}}}'
read initialized
read tools
printf '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"%s","description":"%s:%s"}],"nextCursor":"page-2"}}\n' "$1" "$MCP_TEST_DESCRIPTION" "${AX_EVAL_MCP_INSPECTION_AMBIENT_UNIQUE-unset}"
read tools_page_2
printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"tools":[{"name":"archive","description":"Second page"}]}}'
"#;
        let transport = McpTransport::Stdio {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                script.to_string(),
                "mcp-test".to_string(),
                "${env:AX_EVAL_MCP_TEST_TOOL}".to_string(),
            ],
        };
        std::env::set_var("AX_EVAL_MCP_TEST_TOOL", "lookup");
        std::env::set_var(
            "AX_EVAL_MCP_INSPECTION_AMBIENT_UNIQUE",
            "must-not-reach-server",
        );
        let configured_env = HashMap::from([(
            "MCP_TEST_DESCRIPTION".to_string(),
            "from-target-env".to_string(),
        )]);
        let target_env =
            TargetEnvironment::expanded_from_config(Some(&configured_env), dir.path(), dir.path())
                .expect("target env");

        let response = inspect_tools(
            &target(transport, &["lookup"]),
            dir.path(),
            dir.path(),
            &target_env,
            Duration::from_secs(2),
        )
        .expect("inspect stdio");

        assert_eq!(
            response.pointer("/result/tools/0/name"),
            Some(&json!("lookup"))
        );
        assert_eq!(
            response.pointer("/result/tools/0/description"),
            Some(&json!("from-target-env:unset"))
        );
        assert_eq!(
            response.pointer("/result/tools/1/name"),
            Some(&json!("archive"))
        );
        assert_eq!(
            response.pointer("/axEvalToolsListPages/0/result/nextCursor"),
            Some(&json!("page-2"))
        );
    }

    #[test]
    fn inspects_streamable_http_with_session_and_auth_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            for index in 0..4 {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut headers = String::new();
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).expect("header");
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        content_length = value.trim().parse().expect("content length");
                    }
                    headers.push_str(&line);
                }
                let mut body = vec![0; content_length];
                reader.read_exact(&mut body).expect("body");
                assert!(headers.contains("Authorization: Bearer test-token\r\n"));
                if index > 0 {
                    assert!(headers.contains("Mcp-Session-Id: session-1\r\n"));
                    assert!(headers.contains("MCP-Protocol-Version: 2025-06-18\r\n"));
                }
                let response_body = match index {
                    0 => r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"test","version":"1"}}}"#,
                    1 => "",
                    2 => "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"lookup\"}],\"nextCursor\":\"page-2\"}}\n\n",
                    _ => "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"tools\":[{\"name\":\"archive\"}]}}\n\n",
                };
                let status = if index == 1 { "202 Accepted" } else { "200 OK" };
                let content_type = if index >= 2 {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nMcp-Session-Id: session-1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                )
                .expect("response");
            }
        });
        std::env::set_var("AX_EVAL_MCP_HTTP_TOKEN", "test-token");
        let mut target = target(
            McpTransport::Http {
                url: format!("http://{address}/mcp"),
                headers: None,
            },
            &["lookup"],
        );
        target.auth = Some(McpAuth::BearerEnv {
            env: "AX_EVAL_MCP_HTTP_TOKEN".to_string(),
        });

        let response = inspect_tools(
            &target,
            Path::new("."),
            Path::new("."),
            &TargetEnvironment::default(),
            Duration::from_secs(2),
        )
        .expect("inspect HTTP");
        server.join().expect("server");

        assert_eq!(
            response.pointer("/result/tools/0/name"),
            Some(&json!("lookup"))
        );
        assert_eq!(
            response.pointer("/result/tools/1/name"),
            Some(&json!("archive"))
        );
    }
}
