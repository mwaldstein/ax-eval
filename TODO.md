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
- [ ] Stage 7 — `discover` for MCP targets: `--target` invocation, MCP inspect prompt (declared-vs-learned framing), target-block stamping into authored scenarios, summary attribution for description/schema problems. Agent-mediated; reuse the evaluation-time MCP inspector for authoritative metadata. Design: `docs/mcp-targets.md` (Discovery for MCP). The declared-vs-understood delta report stays deferred.
- [x] Persist advertised MCP surface — run `tools/list` before evaluation and save the full response, including descriptions, input schemas, and annotations, as `artifacts/mcp-tools-list.json`. `host_session` targets remain excluded because their credentials are held by the agent host.
- [x] Auto-validate `target.tools` against the server's actual `tools/list` during preflight. Unknown or stale declarations fail before agent execution with an actionable diagnostic. `host_session` targets retain their documented first-call validation behavior.
- [ ] Add MCP metadata gates so scenarios can assert advertised fields such as `readOnlyHint` and `openWorldHint` against `mcp-tools-list.json`.
- [x] Separate target-server environment from agent environment. Stdio MCP `target.env` is private to the child configuration; evaluated agents receive an adapter baseline plus the explicit top-level `agent_env` allowlist.
- [x] Harden Codex judge-result parsing around the expected `<judge_result>` envelope and cover normalized and raw recorded-output shapes.
- [ ] Model MCP resource operations as structured evidence distinct from
  `tools/call`, with resource-specific counts and judge context. Codex resource
  wrappers are excluded from tool declaration validation in the interim.

## 0.4.0 stabilization

- [x] Correct `first_try_success_rate` to divide first-occurrence successes by unique actions, and preserve unknown command outcomes when adapters omit exit codes.
- [x] Stop agent execution after the first setup command that exits nonzero while retaining setup artifacts and a failed result record.
- [x] Canonicalize OpenCode workspaces before applying both process `cwd` and
  `--dir`, and retain completed profiles with an explicit `judge_error` when
  judge execution or response parsing fails.
- [x] Serialize Codex MCP access to shared `~/.codex/config.toml` across ax-eval
  processes. Hold the lock from snapshot through restoration so overlapping
  runs cannot create duplicate MCP server tables or restore stale snapshots.
- [ ] Route every retained artifact through one redaction-aware sink, including raw transcripts, events, run metadata, and judge feedback.
- [ ] Replace decorative agent-CLI authentication checks with host-specific status probes where available.
- [ ] Finalize a failed or cancelled result with partial evidence for
  setup/health timeouts, agent timeouts, and non-judge post-run evaluation
  errors.

## Evaluation depth

Deepen the Evaluation Profile so it answers three questions independently:

- **Ability** — Did the agent achieve the task and use the Target correctly?
- **Efficiency** — How much effort, recovery, time, and cost did successful use
  require?
- **Agent-facing structure** — How well do the Target's help, descriptions,
  schemas, errors, and workflows support agent discovery and correct use?

Keep Gates as fail-fast guardrails. Do not collapse these dimensions into a
single required score. See `docs/evaluation.md` and ADR-0007.

### Evidence foundation

- [ ] Replace the lossy `(action, outcome)` projection with an ordered canonical
  interaction trace that retains tool identity, invocation fingerprints,
  arguments or normalized argv, outcome, duration, ordering, and evidence
  source.
- [ ] Preserve equivalent structured evidence for CLI and MCP Targets so
  Interaction Metrics and the Judge do not receive asymmetric views.
- [ ] Implement ADR-0007 recovery metrics: distinct actions, exact repeats,
  recovery retries, exact recovery retries, and adjusted recovery retries.
- [ ] Add target latency and total workflow effort signals. Connect target calls
  with existing run-level token, duration, and cost measurements without
  automatically treating repeated calls as waste.

### Evaluation lifecycle

- [ ] Assemble Scenario intent, the canonical interaction trace, Interaction
  Metrics, Gate results, evaluator findings, and run metadata before qualitative
  evaluation. Give the Judge and reports the same evidence.
- [ ] Run custom evaluators early enough for their authoritative outcome evidence
  to inform the Judge.
- [ ] Separate Judge cost policy from evaluation semantics. Support diagnosing a
  failed-Gate run instead of always suppressing its qualitative evaluation.

### Judge calibration

- [ ] Centralize rubric loading and semantic validation. Remove duplicated
  weight validation.
- [ ] Validate Judge responses against the rubric: criterion coverage, unknown
  criteria, numeric ranges, required fields, and confidence range.
- [ ] Compute weighted scores deterministically from validated criterion scores
  instead of trusting the model-provided aggregate.
- [ ] Support criterion-specific scoring anchors and evidence requirements for
  ability, efficiency, and agent-facing structure.
- [ ] Ground Judge findings in specific trace events or outcome evidence so
  issues and highlights remain auditable.

### Tool experience experiments

- [ ] Update Discovery scenarios to include authoritative outcome evidence where
  feasible. Do not require `gates: []` when a Gate or evaluator can verify the
  requested result.
- [ ] Add explicit Discovery assessment of discoverability, workflow clarity,
  schema and argument clarity, error recovery, state observability, and
  composability.
- [ ] Persist a measured-system fingerprint covering Target version, agent CLI,
  model, Adapter, environment, Scenario, fixtures, and guidance variant.
- [ ] Add repeated-run aggregation with sample count and spread before drawing
  conclusions about Target structure. Record an ADR before changing the current
  independent-run scope.
- [ ] Add matched comparison output for Target and guidance variants. Report the
  three evaluation dimensions separately and avoid declaring a winner when run
  variance overwhelms the observed delta.

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

- [ ] Harness materialization — implement the deferred `agent_guidance` and `skills` design in `docs/harness-materialization.md` for 0.5.0.
- [ ] `ax-eval compare` — Diff two runs or show trend across a series.
- [ ] Replace global Codex config mutation with a per-run Codex configuration
  root after defining how API credentials and `host_session` OAuth state are
  projected without leaking secrets. Keep the global lock as compatibility
  protection for hosts that cannot be isolated.
- [ ] Add cross-process locking for `results.jsonl` appends before parallel
  Scenario execution. Codex config locking protects the host config only; it
  does not make the results database safe for concurrent writers.
- [ ] Statistical significance testing across runs
- [ ] Automatic regression detection — results database supports trend analysis; automated alerting not yet implemented.
- [ ] Homebrew tap and formula
- [ ] Package managers (AUR, Nix, winget, Scoop, deb/rpm)
- [ ] Parallel scenario execution
- [ ] Better YAML parse error messages — serde errors wrapped in `anyhow` can be cryptic.
- [ ] `ax-eval init` — Scaffold a new project with example config and scenario.
- [ ] GPG/sigstore release signing
