# MCP Targets

**Status: Stable**

## Purpose

Define how ax-eval evaluates an agent's use of an MCP server, extending the
existing CLI target model without forking the evaluation philosophy or the
three-layer profile.

Originally a scenario target was always a CLI binary. The implemented schema now
supports both CLI and MCP targets: CLI targets are matched from shell command
events, while MCP targets are matched from structured `tools/call` events
discovered through the host agent's MCP config.

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
- Auto-generating scenarios from a raw `tools/list` inventory. Discovery for
  MCP *is* in scope (see Discovery for MCP below), but it is agent-mediated;
  the harness-side protocol client, and the declared-vs-understood delta
  report it enables, are follow-ups.
- Authenticating to protected servers. This spec covers **unauthenticated**
  http/stdio targets. Credentials for protected servers — static bearer/API-key
  from the environment, or a host session established out of band — are
  designed separately in [mcp-auth.md](mcp-auth.md)
  ([ADR-0006](adr/0006-authenticated-mcp-targets.md)). ax-eval does not
  implement OAuth; the harness is the OAuth client.

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
    url: "https://mcp.example.com/mcp"
    headers:
      X-API-Key: "${SEARCH_API_KEY}"
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
| `http` | `url`, `headers` (optional) | Streamable HTTP transport. `headers` is a static string map rendered into each host's header mechanism. |

There is deliberately **no `sse` variant**. The MCP specification deprecated
the standalone HTTP+SSE transport in protocol revision 2025-03-26 in favor of
Streamable HTTP (SSE survives only as an optional response mode *inside*
Streamable HTTP, which is transparent to config). Host support confirms the
call: codex supports streamable HTTP remotes only, opencode has a single
`remote` type with no SSE concept, and claude-code marks its `sse` type
deprecated. A transport that one of the three harnesses cannot run would break
the cross-harness comparison this tool exists to provide. If a legacy
SSE-only server ever matters, add the variant then, with an explicit
harness-support matrix and a `validate` error on unsupported hosts.

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

Remote (`http`) rendering, verified against current harness documentation
2026-07-18:

| Harness | Rendering for `http` |
|---|---|
| `opencode` | `{ "mcp": { "<name>": { "type": "remote", "url": "<url>", "headers": {...}, "enabled": true } } }` |
| `claude-code` | `{ "mcpServers": { "<name>": { "type": "http", "url": "<url>", "headers": {...} } } }` — claude-code also accepts `"streamable-http"` as an alias for `"http"`; render `"http"`. An entry with a `url` and no `type` is read as stdio and rejected, so `type` is always written. |
| `codex` | `[mcp_servers.<name>]` with `url = "<url>"` and `http_headers = { ... }` |

Host-specific auth conveniences (codex's `bearer_token_env_var`/`auth`,
claude-code's `headersHelper`) are intentionally not modeled — the scenario's
generic `headers` map covers the evaluation use case, and anything richer is
host-local config the scenario should not depend on.

One asymmetry to preserve in the implementation: opencode and claude-code
configs are workspace-local, but codex reads `~/.codex/config.toml` — global
state. The codex adapter must write the `[mcp_servers.<name>]` entry before
the run and remove it after (restore-on-exit), and this touch-the-user's-config
behavior belongs in `docs/tradeoffs.md` alongside the existing
fixture-isolation entry.

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
  structured pays off. The excerpt is bounded to the first 50 captured MCP
  calls so exploratory runs cannot produce unbounded judge prompts; the prompt
  states the bound and total captured call count.

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

## Discovery for MCP

### Intent, not mechanism

`discover` is not a scenario generator that happens to need a target; it is
the framework's answer to a specific question — **how self-describing is this
artifact to an agent that has never seen it?** — plus a mirror
(`understanding.md` shows the author what their tool communicates unaided)
and a bootstrap (five goal-oriented, ungated, judge-primary scenarios). The
CLI inspect prompt's defining constraint ("work only from the executable
command surface") is what turns the agent's learning process into a
controlled measurement.

That intent transfers to MCP directly — and lands on a sharper target. A
CLI's self-description is emergent (help text, error messages, conventions).
An MCP server's self-description is **protocol-mandated and author-written**:
tool names, descriptions, input schemas, annotations, and the server's
`instructions` — injected into the agent's context by the host automatically.
Those descriptions are the server author's AGENTS.md-equivalent, the primary
authored artifact that determines agent success. So MCP discovery measures
**description sufficiency**: are the declared name/description/schema enough
for correct, efficient first use — and what did the agent have to learn by
trial that the declarations should have said?

### Why this does not need a harness-side MCP client

Discovery was never a protocol operation; it is an agent operation. The
inspect stage provisions the server into a scratch workspace via the same
`provision_target` hook (Stage 2) and lets the inspecting agent enumerate and
exercise tools through its host — exactly as CLI discovery probes `--help`.
This measures the *experienced* self-description surface (what the metadata
communicates through a real harness), which is truer to the framework's
philosophy than a raw `tools/list` dump would be — consistent with the
adapter principle that the harness never invokes the target; the agent does.

### The port, stage by stage

The five-stage flow (inspect → repair → author → run → summarize) is
unchanged; each stage's prompt and contract adapts:

- **Invocation.** `discover <binary>` keeps its shape for CLI. MCP discovery
  takes a target description instead of a positional binary (e.g.
  `discover --target mcp-target.yaml`, the file holding an `McpTarget`
  block) — the transport must be operator-supplied to launch the server at
  all, which has a useful corollary: generated scenarios don't ask the agent
  to guess the target block. The harness stamps the known `target` into each
  authored scenario, the same way discovery already rewrites
  `template_folder` to absolute paths.
- **Inspect.** The evidence constraint translates: *work only from what the
  server exposes — tool names, descriptions, schemas, instructions, and the
  observed behavior of calls you make; do not read the server's source code
  or external documentation.* (The CLI prompt has the same latent rule;
  for MCP it must be explicit because the server binary often sits in the
  fixture.) `understanding.md` keeps its headings and gains one:
  **Declared vs. Learned** — what the tool metadata stated versus what had
  to be discovered by calling: sequencing requirements, error semantics,
  implicit conventions, argument formats the schema permits but the server
  rejects. That gap *is* the description-improvement backlog, the direct
  analogue of help-text recommendations for CLI authors.
- **Author.** The discovery contract ports verbatim: `gates: []`, judge
  primary, goal-oriented prompts. The authoring agent writes the `tools`
  allow-list from what it observed in session; `validate` checks it as
  usual.
- **Run and summarize.** Unchanged machinery. The summary prompt's failure
  attribution gains one category: alongside fixture-authoring, agent-usage,
  and harness problems, **description/schema problems** — the category the
  server author can actually act on.

### What the protocol client adds later (and only this)

A harness-side MCP client is an *instrumentation* enhancement on top of
agent-mediated discovery, enabling:

- the **declared-vs-understood delta report**: diff the authoritative
  `tools/list` against the understanding document and the run evidence —
  "12 tools declared; the understanding covers 7 correctly; 3 were never
  exercised; `search`'s `date` parameter was misused in every scenario";
- automatic validation of the scenario `tools` allow-list against the live
  server;
- the `mcp_ping` health gate.

Sequencing: agent-mediated MCP discovery requires only Stages 1–2 (schema +
provisioning) plus prompt and stamping work, and slots in as a stage after
the core six. The client-gated delta report remains deferred.

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

## Resolved Questions

Resolved 2026-07-18; research notes in the Transport Provisioning section.

- **Remote transport rendering — resolved.** All three hosts have stable,
  documented remote config shapes (see the verified rendering table above).
  The `sse` transport variant is dropped from the schema: the MCP spec
  deprecated standalone HTTP+SSE in revision 2025-03-26, and codex never
  supported it, so it cannot participate in cross-harness comparison. v1
  ships `stdio` and `http`, both as stable contracts.
- **Tool-name namespacing — resolved.** Evidence identity for MCP actions is
  `(server, tool)` from day one: `McpToolCallEvent` already carries `server`,
  matching filters on `event.server == target.name`, and the `TargetAction`
  projection must treat the server name as part of action identity rather
  than discarding it after matching. With a singular target this changes no
  observable behavior, and it removes the ambiguity pre-emptively — any
  future multi-server extension inherits correct keying instead of a
  migration.

## Deferred

Not independently answerable today; each has an explicit revisit trigger.

- **The discovery delta report and `mcp_ping`.** Agent-mediated `discover`
  for MCP is designed and in scope (see Discovery for MCP) — it needs no
  protocol client. What stays deferred is the harness-side MCP client and
  the instrumentation it enables: the declared-vs-understood delta report,
  live validation of the `tools` allow-list, and the `mcp_ping` health gate.
  Revisit: after MCP discovery ships and real runs show what the delta
  report should contain.
- **Mixed CLI+MCP targets.** Singular target per scenario stands (ADR-0005
  chose it deliberately — it keeps the profile's comparison axis stable).
  Revisit trigger: a concrete scenario that cannot be expressed as two
  separate runs of the same fixture, one per target kind. Note the
  `(server, tool)` keying above means evidence identity will not need rework
  if this lands.

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
  a commented example in the printable scenario template. **Done.**
- **`docs/user-guide.md`**: add an MCP workflow section (declaring a transport,
  the probe-script convention, judge evidence shape).
- **`docs/mcp-targets.md`**: mark this document Stable when Stage 6 lands.
  **Done.**
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
- [MCP transports specification](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports) — stdio + Streamable HTTP; HTTP+SSE deprecated since revision 2025-03-26
- [opencode MCP config](https://opencode.ai/docs/mcp-servers/)
- [Claude Code MCP servers](https://code.claude.com/docs/en/mcp)
- [Codex MCP configuration](https://developers.openai.com/codex/mcp/) (redirects to learn.chatgpt.com)
- [ADR-0001: Keep adapter normalization with adapters](adr/0001-keep-adapter-normalization-with-adapters.md)
- [ADR-0002: Prefer structured interaction evidence](adr/0002-prefer-structured-interaction-evidence.md)
- [ADR-0005: Generalize target identity to CLI and MCP](adr/0005-generalize-target-identity-to-cli-and-mcp.md)
- [Harness materialization](harness-materialization.md) — complementary; explicitly defers MCP provisioning
