# MCP Targets

**Status: Draft**

## Purpose

Define how ax-eval evaluates an agent's use of an MCP server, extending the
existing CLI target model without forking the evaluation philosophy or the
three-layer profile.

Today a scenario target is always a CLI binary. The agent shells out to
`target.binary`; the harness identifies target commands by finding that binary
token in bash command strings and derives outcome from exit codes
(`src/interaction_profile/target.rs`). MCP servers are a different surface: the
agent calls server tools (`tools/call`) over a transport, discovered through the
host agent's own MCP config. There is no shell string and no exit code.

What changes and what does not:

| Concern | CLI today | MCP needed |
|---|---|---|
| Provisioning | Agent finds the binary on PATH; harness sets env | Harness writes the host's native MCP config into the workspace before launch |
| Evidence shape | `CommandEvent { command, exit_code }` | `McpToolCallEvent { server, tool, arguments, is_error }` |
| Target matching | Tokenize command, find `binary` | Match by server name; action = tool name |
| Outcome signal | `exit_code == 0` | `!is_error` |
| Health check | `binary --version` shell command | Often no shell surface; v1 uses a shell probe script |

The three evaluation layers (interaction metrics, gates, judge), the
"evaluate, don't test" principle, and the comparative profile are unchanged.
Only **target identity**, **evidence projection**, and **workspace
provisioning** generalize.

This design is complementary to [harness-materialization.md](harness-materialization.md),
which explicitly defers MCP config provisioning as a non-goal.

---

## Goals

- Model CLI and MCP targets as one target abstraction so the profile stays the
  primary comparison axis across both.
- Keep the scenario's transport description agent-agnostic; the adapter renders
  it into whichever host runs.
- Preserve structured MCP tool arguments for the judge and per-tool analytics.
- Keep the CLI evidence path and all existing fixtures unchanged.

## Non-Goals

- Plural targets per scenario (`targets: [...]`). A scenario targets one server
  or one CLI. Revisit if concrete scenarios demand mixed CLI+MCP evaluation.
- Transforming tool descriptions or schemas between host dialects. The scenario
  describes the transport; the host discovers tool metadata via `tools/list`.
- A first-class `mcp_ping` health/gate in v1. Shell `health_check` only; the
  ping client is deferred.
- Auto-generating scenarios from a server's `tools/list`. That is a future
  `discover` integration, out of scope here.

---

## Design Overview

Three changes, each additive:

1. **Target as a discriminated union.** `TargetConfig` becomes
   `Cli(CliTarget) | Mcp(McpTarget)`. The CLI variant is the existing struct;
   the MCP variant carries an agent-agnostic transport.
2. **Adapter-owned provisioning.** A new `ToolAdapter::provision_target` hook
   writes the host's native MCP config into the workspace before `run()`.
3. **Additive evidence + unified metrics.** A new `McpToolCallEvent` variant
   carries structured MCP calls; the metric engine is refactored onto an
   `(action, outcome)` projection shared by both kinds.

```text
fixture copy → materialize artifacts (--tool) → provision_target (--tool) → adapter.run()
```

`provision_target` runs after materialization and before the adapter, so MCP
config lands in the same workspace the agent reads.

---

## Scenario Schema

`target` becomes a tagged union. `kind` discriminates the variant.

```yaml
# CLI target — unchanged behavior
target:
  kind: cli
  binary: mytool
  command_pattern: "mytool\\s+(add|list)"
  health_check: "mytool --version"
  env:
    MYTOOL_ROOT_DIR: "${AX_EVAL_FIXTURE_DIR}"
```

```yaml
# MCP target — stdio server
target:
  kind: mcp
  name: todo
  transport:
    type: stdio
    command: "${AX_EVAL_FIXTURE_DIR}/todo-mcp-server"
    args: ["--root", "${AX_EVAL_FIXTURE_DIR}"]
  tools: [add, list, complete, search]
  env:
    TODO_DB: "${AX_EVAL_FIXTURE_DIR}/todo.db"
```

```yaml
# MCP target — remote HTTP server
target:
  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/sse"
  tools: [query, index]
```

### Fields

#### `CliTarget`

| Field | Type | Required | Description |
|---|---|---|---|
| `kind` | `"cli"` | yes | Discriminant. Optional via backward-compat deserializer (see Compatibility). |
| `binary` | string | yes | Binary name for evidence matching and judge prompts. Filename-only matching, as today. |
| `command_pattern` | string | no | Regex for transcript regex fallback only. Ignored by structured-capable adapters. |
| `health_check` | string | no | Shell command run before/after as a sanity gate. |
| `env` | map<string,string> | no | Environment variables for the target tool. |

#### `McpTarget`

| Field | Type | Required | Description |
|---|---|---|---|
| `kind` | `"mcp"` | yes | Discriminant. |
| `name` | string | yes | Server identity. Replaces `binary` for evidence matching and judge prompts. |
| `transport` | `McpTransport` | yes | Agent-agnostic connection description. |
| `tools` | list<string> | yes | Declared tool surface. Evidence allow-list and judge context. Enforces that observed calls map to the intended surface. |
| `env` | map<string,string> | no | Environment variables for the server process. `${AX_EVAL_FIXTURE_DIR}` / `${AX_EVAL_RESULTS_DIR}` placeholders expanded as today. |
| `health_check` | string | no | Shell command (v1). Typically a fixture probe script. |

#### `McpTransport`

| Variant | Fields | Notes |
|---|---|---|
| `stdio` | `command`, `args` | Local process over stdin/stdout. `command` and `args` support the run-directory placeholders. |
| `http` | `url` | Streamable HTTP transport. |
| `sse` | `url` | Server-Sent Events transport. |

`tools` is required for MCP targets. Unlike a CLI binary, an MCP server's
surface is not obvious from its name; declaring it lets the harness validate
observed calls against the intended surface and gives the judge a bounded tool
list.

---

## Compatibility Model

Additive and non-breaking for CLI.

- A custom deserializer accepts the legacy flat form (`target: { binary: ... }`
  with no `kind`) and coerces it to `TargetConfig::Cli`. Every existing fixture
  parses unchanged.
- `kind: cli` may be written explicitly but is never required.
- The CLI evidence path (`CommandEvent`, `structured_target_events`) is
  byte-for-byte unchanged; existing metrics and fixtures keep their meaning.
- `validate` treats an unknown `kind`, a missing MCP `transport`, or a missing
  `tools` list as hard errors, parallel to unknown gate types today.

---

## Transport Provisioning

The scenario transport is agent-agnostic. The adapter renders it into its
host's native MCP config format inside the workspace. This is adapter-local
knowledge (ADR-0001): the host config schema is part of the adapter's interface
with its CLI.

| Harness | Config location | Rendering for `stdio` |
|---|---|---|
| `opencode` | `opencode.json` (workspace, via `XDG_CONFIG_HOME`) | `{ "mcp": { "<name>": { "type": "local", "command": [<cmd>, <args...>], "enabled": true } } }` |
| `claude-code` | `.mcp.json` (workspace root) | `{ "mcpServers": { "<name>": { "type": "stdio", "command": "<cmd>", "args": [<args...>], "env": {...} } } }` |
| `codex` | `~/.codex/config.toml` (`[mcp_servers.<name>]`) | `[mcp_servers.<name>]` with `command`, `args`, `env` |

`env` entries and `${AX_EVAL_FIXTURE_DIR}` / `${AX_EVAL_RESULTS_DIR}`
placeholders reuse the existing expander in `src/target_env.rs`; no new
substitution mechanism.

Remote (`http`/`sse`) rendering varies by host and is evolving; implementations
must verify the remote form against current harness docs before shipping. The
stdio forms above are the stable v1 contract.

### Trait hook

```rust
pub trait ToolAdapter: Send + Sync {
    // ...existing members unchanged...

    /// Write agent-native config so the agent can discover target MCP servers.
    /// Default no-op: CLI targets need nothing.
    fn provision_target(&self, _target: &TargetConfig, _workspace: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
```

`src/run/execution.rs` calls `adapter.provision_target(&scenario.target,
env_root)` after materialization and before `adapter.run(...)`. CLI targets hit
the default no-op.

---

## Interaction Evidence

`CommandEvent` is unchanged and remains canonical for CLI runs. MCP runs emit a
parallel structured variant.

```rust
pub enum InteractionInput {
    StructuredToolCalls(Vec<CommandEvent>),        // CLI bash, unchanged
    StructuredMcpToolCalls(Vec<McpToolCallEvent>), // new
    TranscriptRegex,
}

pub struct McpToolCallEvent {
    pub server: String,
    pub tool: String,
    pub arguments: serde_json::Value,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
}
```

A separate variant (rather than flattening MCP into `CommandEvent`) preserves
the structured `arguments` the judge and per-tool analytics need, consistent
with ADR-0002's preference for adapter-owned structured facts.

### Adapter normalizers

Each adapter's normalizer extends to capture MCP tool calls alongside bash
calls — opencode emits these as `part.tool == "mcp"` events; claude-code and
codex have their own shapes. Parsing stays adapter-local (ADR-0001). A
structured-capable adapter running an MCP scenario must emit
`StructuredMcpToolCalls` for normal completed runs; falling back to transcript
regex or returning no usable target-tool events is an evaluation failure, the
same contract ADR-0002 applies to CLI runs.

### Unified metrics via `(action, outcome)`

The metric engine is refactored onto a small projection so error rate, retry,
first-try success, iteration ratio, and command count mean the same thing
across kinds:

```rust
struct TargetAction { action: String, outcome: Outcome }
enum Outcome { Success, Failure, Unknown }
```

| Kind | action | outcome |
|---|---|---|
| CLI | bash subcommand after `binary` token (existing `target.rs` logic) | `exit_code == 0` |
| MCP | `tool` name | `!is_error` |

`EfficiencyMetrics` keeps its current shape, so cross-model and cross-harness
comparisons remain valid. `interaction_evidence_source` gains a
`structured_mcp_tool_calls` value alongside `structured_tool_calls` and
`transcript_regex_fallback`.

---

## Target Matching

`TargetInteractionSpec` (`src/interaction_evidence.rs`) generalizes to a kind:

```rust
pub enum TargetSpec {
    Cli { binary: String, command_pattern: Option<String> },
    Mcp { server: String, tools: HashSet<String> },
}
```

`src/interaction_profile/target.rs` keeps the CLI branch verbatim and adds an
MCP branch: match `event.server == server`, action = `event.tool`, and filter
observed calls to the declared `tools` allow-list. `src/evaluation.rs`
constructs whichever variant the scenario declares.

---

## Judge Prompt

`src/judge/eval.rs` currently hardcodes "the CLI tool `{tool_name}`". The
builder takes a small `JudgeTargetView { kind, name, summary }` instead of a
bare name:

- **CLI** — unchanged wording.
- **MCP** — "the MCP server `{name}` exposing tools `[add, list, ...]`", and
  the evidence excerpt includes the structured `arguments` of each call rather
  than a flattened command string. This is where keeping `McpToolCallEvent`
  structured pays off.

The rubric format, `JudgeResponse` schema, and `pass_threshold` semantics are
unchanged.

---

## Health Checks and Gates

- **Health:** v1 keeps `health_check` as an optional shell command. MCP
  scenarios typically ship a probe script in the fixture (e.g. a small CLI the
  server exposes, or a script that reads the backing store). A first-class
  `mcp_ping` health/gate — the harness does a JSON-RPC `initialize`/`ping` — is
  a follow-up that requires a harness-side MCP client.
- **Gates:** unchanged. MCP scenarios lean on `script` gates and `evaluators`
  (`docs/scripts.md`) because server state is usually not filesystem-visible.
  No new gate type is required for v1; an `mcp_tool_succeeds` gate is a natural
  later addition once an MCP client exists.

---

## Fixture and Guidance

The fixture stays the unit of guidance. For MCP, `AGENTS.md` no longer
documents CLI syntax — the agent discovers tools via `tools/list` — and instead
documents **when and why to use each tool**: workflow sequencing, tool
semantics, error-recovery guidance, and conventions. This slots directly into
the existing guidance-testing workflow (`docs/scenarios.md`, Guidance Testing
Workflow). The fixture commonly ships a probe script the agent and the gates
use to inspect server state.

---

## Pipeline Integration

```text
fixture copy (verbatim)
  → materialize artifacts (--tool)        [harness-materialization.md]
  → provision_target (--tool)             [this doc]
  → adapter.run()                         [ADR-0001]
  → persist transcript + command events   [existing]
  → evaluate (gates, profile, judge)      [existing, kind-aware]
```

`provision_target` is the only new step. It needs the selected `--tool`, the
workspace root, and `scenario.target` — all already threaded through the run
request. Adding a future harness means adding its MCP config rendering without
changing the scenario schema.

---

## Open Questions

- **Remote transport rendering.** The `http`/`sse` config shapes for opencode,
  codex, and (to a lesser degree) claude-code are still settling. v1 ships
  `stdio` as the stable contract; remote forms confirmed per harness before
  release.
- **`discover` for MCP.** `src/discover/prompts.rs` assumes a `binary`. A
  future `discover` mode could inspect a server's `tools/list` to author
  scenarios. Out of scope here.
- **Mixed CLI+MCP targets.** Singular target for v1; revisit if concrete
  scenarios need both in one evaluation.
- **Tool-name namespacing.** If two MCP servers in scope ever expose the same
  tool name, evidence matching should key on `(server, tool)`. Singular target
  makes this moot for v1 but the projection should not preclude it.

---

## In-System Documentation

Per [ADR-0003](adr/0003-keep-cli-self-documenting.md), schema and workflow
changes update help text, templates, docs, and tests together. Implementing
this feature touches the scenario schema, so update these surfaces in lockstep:

- **`docs/scenarios.md`**: add the `target` union to the schema reference
  (field tables for `CliTarget`, `McpTarget`, `McpTransport`) and a worked MCP
  example parallel to the existing CLI example.
- **`docs/evaluation.md`**: note the new `structured_mcp_tool_calls` evidence
  source and the `(action, outcome)` metric projection.
- **`src/commands.rs` (`SCENARIO_TEMPLATE`)**: surface an MCP `target` block as
  a commented example in the printable scenario template.
- **`docs/user-guide.md`**: add an MCP workflow section (declaring a transport,
  the probe-script convention, judge evidence shape).
- **`docs/mcp-targets.md`**: flip **Status** from Draft to Stable as open
  questions resolve.
- **`CHANGELOG.md`**: `Added` entries under the target release for the MCP
  target kind, transport rendering, and `StructuredMcpToolCalls` evidence.
- **`ax-eval-fixtures/`**: add an example MCP scenario (stdio server fixture +
  probe script) parallel to how `example_judge` demonstrates the judge.
- **`src/adapter/README.md`**: document the `provision_target` responsibility
  and MCP normalization expectations per adapter.
- **`CONTEXT.md`**: add glossary entries for **Target** (CLI binary vs MCP
  server) and **Tool Call Event** (bash vs MCP).
- **CLI help + snapshots**: run `scripts/generate-cli-reference.sh` if any CLI
  help text changes (likely none — these are YAML fields, not flags).
- **`AGENTS.md` (Common Tasks)**: add an "Adding an MCP Provisioning Target"
  entry parallel to the existing gate/adapter/schema tasks, listing the
  adapter trait method, transport rendering, docs, and tests to touch for a new
  host.

As with prior schema changes, update `src/scenario/types.rs`, add
deserialization coverage in `src/scenario/tests/` (including the legacy
flat-form coercion), and run `cargo test` before release.

---

## References

- [Model Context Protocol introduction](https://modelcontextprotocol.io/docs/getting-started/intro)
- [opencode MCP config](https://opencode.ai/docs/mcp-server/)
- [Claude Code MCP servers](https://docs.claude.com/en/docs/claude-code/mcp)
- [Codex MCP configuration](https://developers.openai.com/codex/mcp/)
- [ADR-0001: Keep adapter normalization with adapters](adr/0001-keep-adapter-normalization-with-adapters.md)
- [ADR-0002: Prefer structured interaction evidence](adr/0002-prefer-structured-interaction-evidence.md)
- [ADR-0005: Generalize target identity to CLI and MCP](adr/0005-generalize-target-identity-to-cli-and-mcp.md)
- [Harness materialization](harness-materialization.md) — complementary; explicitly defers MCP provisioning
