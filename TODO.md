# TODO

## MCP support

Evaluate agent use of MCP servers alongside CLI targets. Design: `docs/mcp-targets.md`, decision: `docs/adr/0005-generalize-target-identity-to-cli-and-mcp.md`.

- [x] Author ADR-0005 + `docs/mcp-targets.md` spec
- [x] Stage 1 — `TargetConfig` → tagged `Cli | Mcp` enum + backward-compat deserializer (flat `target.binary` → `Cli`); `McpTarget`/`McpTransport` types; deserialization coverage in `src/scenario/tests/`
- [x] Stage 2 — `ToolAdapter::provision_target` hook; wire into `src/run/execution.rs`; render host-native MCP config for opencode first, end-to-end a stdio scenario
- [x] Stage 3 — `McpToolCallEvent` + `InteractionInput::StructuredMcpToolCalls`; refactor metric engine onto `(action, outcome)` projection; MCP matching branch in `src/interaction_profile/target.rs`
- [x] Stage 4 — capture MCP tool events in `src/adapter/{opencode,codex,claude_code}/normalize.rs`
- [x] Stage 5 — parameterise judge prompt by target kind (`JudgeTargetView`); include structured args for MCP
- [x] Stage 6 — update `docs/scenarios.md`, `docs/evaluation.md`, `src/adapter/README.md`, `CONTEXT.md`, `SCENARIO_TEMPLATE`; add example MCP fixture; flip spec to Stable
- [ ] Stage 7 — `discover` for MCP targets: `--target` invocation, MCP inspect prompt (declared-vs-learned framing), target-block stamping into authored scenarios, summary attribution for description/schema problems. Agent-mediated; needs only Stages 1–2. Design: `docs/mcp-targets.md` (Discovery for MCP). Harness-side MCP client + declared-vs-understood delta report stay deferred.

## Authenticated MCP targets

Evaluate agent use of protected MCP servers. Design: `docs/mcp-auth.md`, decision: `docs/adr/0006-authenticated-mcp-targets.md`. ax-eval renders credential config; the harness is the OAuth client.

- [x] Author ADR-0006 + `docs/mcp-auth.md` spec
- [x] Prereq — `${env:NAME}` expansion in `src/target_env.rs` (also fixes the literal-`${VAR}` bug in mcp-targets.md's remote example); unset var is a provision-time error
- [x] `McpAuth` schema (`none | bearer_env | headers | host_session`) on `McpTarget`; http-only + literal-secret-rejection validation; deserialization tests
- [x] Auth rendering in each adapter's `provision_target` (bearer to host mechanism, disable auto-OAuth for static modes, codex passes env-var name); golden-config tests
- [x] Redaction: resolved secrets must never reach results artifacts (hard dependency on the audit's redaction-sink work)
- [x] Preflight: env-var presence for static modes; documented `host_session` gap
- [x] Docs/template/CHANGELOG lockstep; flip `docs/mcp-auth.md` to Stable when static modes ship
- [ ] Host-session status probes where available (`codex mcp get`, `opencode mcp debug`)
- [ ] Out-of-band `mint-token` / login helper

## Medium-impact

- [ ] ~~`--dry-run` for `discover`~~ — Not feasible: discover inherently requires LLM calls for inspection, scenario authoring, and summary generation.
- [ ] `AX_EVAL_RESULTS_DIR` runtime env var — Currently only a template placeholder in `target.env`; custom scripts and evaluators cannot access it at runtime.
- [ ] Global judge config — Apply judge settings (enabled, tool, model, rubric, threshold) to all scenarios without repeating a `judge:` block per scenario.
- [ ] JUnit/XML output format — CI dashboard integration.
- [ ] Shell completions — `ax-eval completions bash|zsh|fish`.

## Future

- [ ] `ax-eval compare` — Diff two runs or show trend across a series.
- [ ] Statistical significance testing across runs
- [ ] Automatic regression detection — results database supports trend analysis; automated alerting not yet implemented.
- [ ] Homebrew tap and formula
- [ ] Package managers (AUR, Nix, winget, Scoop, deb/rpm)
- [ ] Parallel scenario execution
- [ ] Better YAML parse error messages — serde errors wrapped in `anyhow` can be cryptic.
- [ ] `ax-eval init` — Scaffold a new project with example config and scenario.
- [ ] GPG/sigstore release signing
