# AGENTS.md - ax-eval

This file is a routing guide for LLM agents working on `ax-eval`. Keep
durable details in the focused docs linked below rather than expanding this
file into a full manual.

## Project Overview

`ax-eval` evaluates how effectively LLM coding agents use CLI tools. It
produces dimensional evaluation profiles: quantitative metrics plus qualitative
scores. Gates are fail-fast sanity checks; the primary value is the profile that
shows how well a run went and what changed between tools, models, or guidance.

## Core Principle

This framework evaluates; it does not merely test. Gates answer "did a
catastrophic failure happen?" while the evaluation profile answers "how well did
the run go?" Preserve that distinction when adding scenarios, gates, metrics, or
docs. See `docs/evaluation.md`.

## Build Commands

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check

cargo run -- scenarios
cargo run -- run --scenario <name>
cargo run -- run --scenario <name> --tool opencode
```

## Where To Look

- Scenario schema: `docs/scenarios.md`
- Evaluation layers, gates, and scoring: `docs/evaluation.md`
- Script hooks, script gates, and evaluators: `docs/scripts.md`
- Harness architecture: `src/adapter/` and `src/run/`
- Runtime adapter organization: `src/adapter/README.md`
- Testing guidance: `docs/testing.md`
- Guidance-quality examples: `docs/guidance-testing.md`
- User-facing workflow: `docs/user-guide.md`
- Domain vocabulary: `CONTEXT.md`
- Architectural decisions: `docs/adr/`

## Common Tasks

### Adding a New Gate Type

1. Add the `Gate` variant in `src/scenario/types.rs`.
2. Add deserialization coverage in `src/scenario/tests/gates.rs`.
3. Implement evaluation dispatch in `src/evaluation.rs`.
4. Add focused behavior tests for the evaluator.
5. Update `docs/evaluation.md` and `docs/scenarios.md` when schema or semantics change.

### Adding a New Adapter

Follow `src/adapter/README.md`.

### Updating Scenario Schema

1. Modify types in `src/scenario/types.rs`.
2. Update YAML fixtures under `fixtures/`.
3. Update the relevant spec in `specs/`.
4. Run `cargo test`.

### Debugging Test Failures

1. Run a specific test: `cargo test <test_name>`.
2. Run with output: `cargo test <test_name> -- --nocapture`.
3. Check the scenario fixture under `fixtures/`.
4. Review generated artifacts under `ax-eval-results/`.

## Code Conventions

- Error handling: use `anyhow` for propagation.
- Async: keep async minimal; prefer sync where practical.
- Tests: unit tests alongside source, integration tests in `tests/`.
- Types: use `serde` for YAML/JSON serialization.
- Naming: `snake_case` for functions and variables, `PascalCase` for types.

## Commit Messages

Use concise imperative messages: `Fix cache invalidation`, not `Fixed cache invalidation`.
Keep the subject specific and usually under 72 characters; avoid vague subjects like `Docs update`.
For code changes, prefer a plain imperative subject.
Prefixes are optional; use them only when they clarify scope.
Allowed prefixes: `docs:`, `test:`, `ci:`, `chore:`.
Do not require Conventional Commits unless release automation later depends on it.
Commit one logical change at a time.
Add a body when the reason or tradeoff is not obvious from the diff.
Reference issues at the end of the body when relevant, e.g. `Refs #123`.
Capitalize the first word unless using an optional prefix.

## Versioning

Use strict semver for all version strings and git tags: `MAJOR.MINOR.PATCH` with
optional `-pre.N` pre-release suffix (e.g. `0.3.0-beta.1`). Never use
`-beta1`, `-rc1`, or other non-dotted pre-release forms — crates.io and many
tooling ecosystems reject them.

## Agent Skills

### Issue Tracker

Issues and PRDs are tracked in GitHub Issues for `mwaldstein/ax-eval`
using the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage Labels

Use the default canonical triage labels: `needs-triage`, `needs-info`,
`ready-for-agent`, `ready-for-human`, and `wontfix`. See
`docs/agents/triage-labels.md`.

### Domain Docs

This is a single-context repo: read root `CONTEXT.md` and root `docs/adr/`
when present. See `docs/agents/domain.md`.
