# TODO

## Medium-impact

- [ ] ~~`--dry-run` for `discover`~~ — Not feasible: discover inherently requires LLM calls for inspection, scenario authoring, and summary generation.
- [ ] `AX_EVAL_RESULTS_DIR` runtime env var — Currently only a template placeholder in `target.env`; custom scripts and evaluators cannot access it at runtime.
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
