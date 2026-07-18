# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- MCP server targets (in progress; design: `docs/mcp-targets.md`). Scenarios can declare `target.kind: mcp` with a server `name`, an agent-agnostic `transport` (`stdio` or `http`), and a required `tools` list. Existing flat CLI `target: { binary: ... }` scenarios parse unchanged. Stage 2 adds adapter-owned target provisioning for opencode, claude-code, and codex, and adapters now capture MCP tool-call evidence from their structured output streams.
- MCP interaction evidence now supports structured tool-call events, persists MCP events with run artifacts, and feeds the same interaction metrics as CLI evidence through a shared action/outcome projection.

### Changed
- Result caching is now opt-in (`--cache`) rather than default. Runs are always fresh by default because ax-eval evaluates a non-deterministic system — caching a single past run and replaying it misrepresents current agent capability. The results database (`results.jsonl`) remains the source of truth for run history. See ADR-0004 for rationale.
- `run --help` now notes that the harness does not execute the target binary — the LLM agent does — and that it should be on PATH or in the fixture workspace.
- Guardrail metrics now report a three-valued `gate_status` plus named failures instead of gate counts. This is a breaking `metrics.json` and `results.jsonl` schema change.
- Composite scoring now uses judge and interaction weights only, renormalizes over available signals, and is omitted when a guardrail fails. This is a breaking scenario `composite` schema change.

### Removed

- Removed gate statistics and the `evaluation.composite.gate_weight` configuration field. Old YAML that still sets `gate_weight` now receives a migration error telling authors to delete it.

### Fixed

- opencode adapter now passes `--dir <fixture>` so the agent anchors to the isolated fixture workspace. Previously it set the process cwd but not `--dir`, causing opencode to anchor to the git root and write outside the fixture, invalidating all opencode runs.
- Judge runs now parse correctly with opencode. The opencode normalizer only captured bash command events, so text-only judge runs produced an empty transcript and fell back to raw NDJSON where `<judge_result>` JSON was escaped inside text-event strings. The normalizer now extracts text-event payloads into the transcript, and `judge_output_text` prefers the normalized transcript over raw output.

## [0.3.0-beta.4] - 2026-06-16

### Added

- Agent-driven evaluation guidance: README and user guide now show how to point an AI coding tool at ax-eval so it self-discovers the CLI (`guidance`, `template`, `discover`) and runs the evaluate-modify-repeat loop

### Changed

- Default `fixtures_path` is now `ax-eval-fixtures` (was the generic `fixtures`), matching the `ax-eval-results` namespace and avoiding collisions with other tools' `fixtures/` directories. The repo's own fixtures directory was renamed to match. Set `fixtures_path = "fixtures"` in `ax-eval-config.toml` to keep the previous default.

## [0.3.0-beta.3] - 2026-06-16

### Added

- `--verbose` flag and `RUST_LOG` environment variable for fine-grained debug output
- CLI help snapshot tests and auto-generated reference documentation (`docs/reference/cli-commands.md`)
- `ax-eval template rubric` prints a copyable rubric schema (weighted criteria and judge output contract) for LLM-as-judge evaluation

### Changed

- `ax-eval template scenario` now documents the optional `evaluation.judge.rubric` field instead of hiding it in a comment

### Fixed

- `show` command: renamed `NAME` positional arg to `ID` to match actual lookup behavior
- Stale `qipu` references replaced with `mytool` in user-facing CLI help and docs
- `validate --all` now skips non-scenario YAML files (e.g. rubrics) using a `>=2` distinctive-key heuristic, matching the behavior of `run --all`
- Rubric path resolution: now CWD-agnostic. Relative paths are resolved first against the scenario YAML file, then the workspace (env_root), then fall back to fixtures_path. Previously relied on CWD-dependent behavior that differed between discovery-generated and hand-written scenarios.
- `validate` now checks that referenced rubric files exist and are valid (YAML parseable, criteria weights sum to 1.0). Missing or invalid rubrics are reported as hard errors.

## [0.3.0-beta.2] - 2026-05-23

### Added

- `ax-eval validate` command: schema validation for scenario YAML files without fixture setup or LLM spend. Checks required fields, gate types (with typo suggestions), regex compilation, judge configuration, composite weights, and semantic warnings. Supports `--scenario <path>` and `--all`.

### Fixed

- `target.binary` documented as interaction-matching label, not execution path
- Tutorial removed broken `PATH: "${AX_EVAL_FIXTURE_DIR}:${PATH}"` (unsupported expansion)

## [0.3.0-beta.1] - 2026-05-23

### Changed

- Renamed project from `llm-tool-test` to `ax-eval` (binary, crate, config, docs)
- Reframed codebase and documentation as an evaluation framework

## [0.2.0-beta.1] - 2026-05-23

### Added

- Discovery workflow for scenario management:
  - Batch, manifest, prompts, stages, and understanding modules
  - Discovery scenario contract and stage artifact modularization
- Structured interaction evidence system:
  - `interaction_evidence.rs` module with evidence extraction and profiling
  - Target interaction evidence concentration and normalization
  - Interaction evidence gate evaluation
- Target health checks for scenario run validation
- Configurable judge tools with parameterized prompts and rationale field
- Default judge criteria when rubric omitted
- Scenario catalog module for scenario management
- Run lifecycle module with state machine and prepared run context
- Scenario run request abstraction with early-return policy
- Script contract interpretation and classified script execution results
- Execution transcript persistence extraction
- CLI templates and guidance for LLM tool authors
- Agent skill conventions and domain documentation (`docs/agents/`)
- User guide (`docs/user-guide.md`)
- Testing guidance (`docs/testing.md`)
- Guidance quality examples (`docs/guidance-testing.md`)
- New example scenario fixtures:
  - `example_e2e`, `example_full_config`, `example_gates_comprehensive`
  - `example_judge`, `example_guidance_minimal`, `example_guidance_rich`
- `CONTEXT.md` for domain vocabulary
- `Cargo.lock` tracking

### Changed

- Modularized evaluation: split evaluation runners, extracted gate evaluation module
- Moved adapter normalization into per-adapter modules (`normalize.rs`)
- Centralized adapter availability checks
- Centralized target environment handling and env conversion
- Moved cache key creation to workspace, run early-return policy to request
- Named input types across the codebase (metrics build, evaluation, script runner, judge execution/evaluation, setup command reports, target env conversion)
- Introduced result record input pattern for consistent return types
- Upgraded dependencies: portable-pty, sha2, toml parser, thiserror, YAML parser
- Made mock adapter use structured tool calls
- Documented adapter normalization ownership and mock test scope
- Updated README positioning and value proposition

### Fixed

- Five high-severity issues: metrics logic, orphaned processes, unwrap panics, env var mutation
- Cache invalidation and iteration metrics
- Judge response parsing
- Target environment placeholder paths
- Clean age filtering
- Codex default model handling
- Fixture transcript write error propagation
- Aligned documentation with implementation

### Removed

- SmartMockAdapter and examples tests (reverted)

## [0.1.0] - 2025-04-25

### Added

- Generic gate system with 9 gate types for tool-agnostic evaluation:
  - `command_succeeds`, `command_output_contains`, `command_output_matches`
  - `command_json_path` with JSON path assertion support (`exists`, `equals`, `contains`, `len >=/==/> N`)
  - `file_exists`, `file_contains`, `file_matches`
  - `no_transcript_errors`, `script`
- Scripts system for extensible evaluation:
  - Post-execution scripts that run after the agent exits
  - Script gates that return structured pass/fail via JSON or exit codes
  - Custom evaluators that produce arbitrary metrics and scores
- Target tool configuration per scenario (`binary`, `command_pattern`, `health_check`, `env`)
- Transcript analyzer with configurable command pattern matching
- Composite scoring with configurable weights (judge / gates / interaction)
- Conditional composite score display in reports and summaries
- `ax-eval-config.example.toml` with tool and profile configuration examples
- Example scenario (`fixtures/example_basic/`) demonstrating the scripts system

### Changed

- Made framework fully tool-agnostic; removed all qipu-specific code
- Removed snapshot mechanism and store analysis modules
- Replaced qipu-specific gates with generic CLI evaluation primitives
- Updated scenario schema to require `template_folder` and `target` configuration
- Simplified evaluation metrics by removing quality metrics and note/link counts

### Removed

- `src/store_analysis.rs` and `QualityMetrics` struct
- `create_store_snapshot` and related snapshot links in evaluation reports
- `qipu_version` and `qipu_commit` fields from run metadata and cache keys
- `get_prime_output` from fixture utilities
- `SPLIT_PLAN.md` (superseded by completed implementation)

[Unreleased]: https://github.com/mwaldstein/ax-eval/compare/v0.3.0-beta.4...HEAD
[0.3.0-beta.4]: https://github.com/mwaldstein/ax-eval/compare/v0.3.0-beta.3...v0.3.0-beta.4
[0.3.0-beta.3]: https://github.com/mwaldstein/ax-eval/compare/v0.3.0-beta.2...v0.3.0-beta.3
[0.3.0-beta.2]: https://github.com/mwaldstein/ax-eval/compare/v0.3.0-beta.1...v0.3.0-beta.2
[0.3.0-beta.1]: https://github.com/mwaldstein/ax-eval/compare/v0.2.0-beta.1...v0.3.0-beta.1
[0.2.0-beta.1]: https://github.com/mwaldstein/ax-eval/compare/v0.1.0...v0.2.0-beta.1
[0.1.0]: https://github.com/mwaldstein/ax-eval/releases/tag/v0.1.0
