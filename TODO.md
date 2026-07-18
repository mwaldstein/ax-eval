# TODO

## MCP support

Evaluate agent use of MCP servers alongside CLI targets. Design: `docs/mcp-targets.md`, decision: `docs/adr/0005-generalize-target-identity-to-cli-and-mcp.md`.

- [x] Author ADR-0005 + `docs/mcp-targets.md` spec
- [ ] Stage 1 — `TargetConfig` → tagged `Cli | Mcp` enum + backward-compat deserializer (flat `target.binary` → `Cli`); `McpTarget`/`McpTransport` types; deserialization coverage in `src/scenario/tests/`
- [ ] Stage 2 — `ToolAdapter::provision_target` hook; wire into `src/run/execution.rs`; render host-native MCP config for opencode first, end-to-end a stdio scenario
- [ ] Stage 3 — `McpToolCallEvent` + `InteractionInput::StructuredMcpToolCalls`; refactor metric engine onto `(action, outcome)` projection; MCP matching branch in `src/interaction_profile/target.rs`
- [ ] Stage 4 — capture MCP tool events in `src/adapter/{opencode,codex,claude_code}/normalize.rs`
- [ ] Stage 5 — parameterise judge prompt by target kind (`JudgeTargetView`); include structured args for MCP
- [ ] Stage 6 — update `docs/scenarios.md`, `docs/evaluation.md`, `src/adapter/README.md`, `CONTEXT.md`, `SCENARIO_TEMPLATE`; add example MCP fixture; flip spec to Stable
- [ ] Stage 7 — `discover` for MCP targets: `--target` invocation, MCP inspect prompt (declared-vs-learned framing), target-block stamping into authored scenarios, summary attribution for description/schema problems. Agent-mediated; needs only Stages 1–2. Design: `docs/mcp-targets.md` (Discovery for MCP). Harness-side MCP client + declared-vs-understood delta report stay deferred.

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
