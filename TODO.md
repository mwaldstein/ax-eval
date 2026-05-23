# v0.3 TODO — First Public Release

## High-impact

- [x] **Stabilize specs** — All 3 user-facing docs marked Stable and moved to `docs/`. Removed `specs/llm-user-validation.md` (architecture is now in code). Stripped Rust struct sections from `docs/evaluation.md`.
- [x] **Tutorial** — `docs/tutorial.md`: step-by-step walkthrough creating a mock CLI, writing a scenario, running evaluation, and iterating.
- [x] **crates.io publishing** — Metadata in Cargo.toml. CI runs `cargo publish --dry-run` on every push. Release workflow publishes to crates.io on tag push (both pre-release and stable).

## Medium-impact

- [ ] `ax-eval validate` — Quick scenario schema validation without fixture setup or LLM spend.
- [ ] `--dry-run` for `discover` — Validate discovery workflow without spending credits.
- [x] `RUST_LOG` / `--verbose` — Added global `-v/--verbose` flag and `RUST_LOG` support. Tracing initialized in main.rs with debug logging at key points: adapter resolution, scenario loading, gate evaluation, judge execution, interaction profiling.
- [ ] JUnit/XML output format — CI dashboard integration.
- [ ] Shell completions — `ax-eval completions bash|zsh|fish`.

## Future

- [ ] `ax-eval compare` — Diff two runs or show trend across a series. Spec explicitly marks regression detection as future work; the results database already supports it.
- [ ] Statistical significance testing across runs
- [ ] Homebrew tap
- [ ] Parallel scenario execution
- [ ] Better YAML parse error messages — serde errors wrapped in `anyhow` can be cryptic.
- [ ] `ax-eval init` — Scaffold a new project with example config and scenario.
- [ ] GPG/sigstore release signing
