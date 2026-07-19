# Generalize Target Identity to CLI and MCP

A scenario target is the thing an agent is evaluated on. Today that is always a
CLI binary: the agent shells out to `target.binary`, and the harness identifies
target commands by finding that binary token in bash command strings
(`src/interaction_profile/target.rs`). MCP servers break the assumption — the
agent calls server tools (`tools/call`) over stdio or HTTP, discovered through
the host agent's own MCP config, with no shell string and no exit code.

We model the target as a discriminated union
(`TargetConfig::Cli | TargetConfig::Mcp`) so illegal states — an MCP server with
a `command_pattern`, a CLI binary with a transport — are unrepresentable. The
scenario carries an agent-agnostic transport (stdio/http; an sse variant was
considered and dropped 2026-07 — the MCP spec deprecated standalone HTTP+SSE
and codex does not support it, see `docs/mcp-targets.md`); each adapter
renders it into its host's native MCP config (opencode.json `mcp`,
claude-code `.mcp.json`, codex `mcp_servers`) through a new
`ToolAdapter::provision_target` hook. Provisioning belongs on the adapter for
the same reason [ADR-0001](0001-keep-adapter-normalization-with-adapters.md)
puts raw-output parsing there: each host's config schema is part of that
adapter's interface with its CLI.

Interaction evidence generalizes additively rather than by forking the profile.
`CommandEvent { command, exit_code }` remains canonical for CLI runs; a new
`McpToolCallEvent { server, tool, arguments, is_error }` carries MCP tool calls
without flattening away the structured arguments the judge needs. The metric
engine is refactored onto a small `(action, outcome)` projection so error rate,
tool reuse, first-try success, and command count mean the same thing across target
kinds and stay comparable across models and harnesses. MCP outcome maps
`is_error` to failure — MCP has no exit codes.

We chose this over flattening MCP calls into synthetic `CommandEvent`s because
that would destroy the structured `arguments` — the evidence Layer 3 judges and
per-tool analytics depend on — violating the spirit of
[ADR-0002](0002-prefer-structured-interaction-evidence.md). We chose a single
discriminated target over `targets: [...]` plural because no concrete scenario
yet demands mixed CLI+MCP evaluation, and a singular target keeps the
evaluation profile's primary comparison axis stable; plural can be revisited if
real scenarios require it.

The practical contract is:

- `target.kind` defaults to `cli` through a backward-compatible deserializer;
  existing flat `target: { binary: ... }` scenarios parse unchanged.
- CLI targets and their evidence path are unchanged; all existing fixtures and
  metrics keep their meaning.
- Adapters that declare `supports_structured_tool_calls` must emit
  `StructuredMcpToolCalls` for normal completed MCP runs, mirroring the
  no-silent-fallback contract from ADR-0002.
- `interaction_evidence_source` gains a `structured_mcp_tool_calls` value
  alongside the existing `structured_tool_calls` and
  `transcript_regex_fallback`.
- Health checks remain shell commands for v1; a first-class `mcp_ping`
  health/gate is deferred until a harness-side MCP client justifies it.

See `docs/mcp-targets.md` for the scenario schema, transport rendering, and
pipeline integration.
