# Testing

The framework is tested against itself using mock scenarios and focused unit
tests.

## Standard Checks

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Run a specific test:

```bash
cargo test <test_name>
cargo test <test_name> -- --nocapture
```

## Test Areas

- Scenario parsing: YAML to Rust types.
- Gate evaluation: all gate types and error modes.
- Script execution: environment variables, timeouts, exit codes.
- Adapter interfaces: mock adapter and normalization behavior.
- CLI commands: integration tests in `tests/cli.rs`.
- Transcript analysis: command extraction and interaction metrics.

## Fixture Scenario Regression Tests

Every example scenario in `fixtures/` has an automated regression test in
`src/fixture_tests.rs`. These run mock-tool commands directly without a real
LLM, execute setup and post scripts, and verify evaluation gates pass.

```bash
cargo test fixture_tests
```

These validate the framework and the built-in scenario definitions.

## Real-LLM End-To-End Tests

Tests in `tests/e2e.rs` exercise the framework with actual LLM adapters. They
require an installed and authenticated LLM tool and are gated behind
`AX_EVAL_E2E=1`:

```bash
AX_EVAL_ENABLED=1 AX_EVAL_E2E=1 cargo test --test e2e -- --ignored
```

Use these for manual validation or CI environments with LLM credentials. They
verify artifact generation such as transcript, metrics, and evaluation report.

## Scenario Testing Philosophy

Gates verify outcomes, not process. Because LLMs are nondeterministic, a
scenario should ask "did the task get done?" rather than "did the LLM follow my
expected command sequence?"

Good gate:

```yaml
- type: file_exists
  path: report.pdf
```

Avoid as a default correctness gate:

```yaml
- type: no_transcript_errors
```

`no_transcript_errors` is an interaction-quality guardrail. It is useful only
when zero target-tool command errors is the scenario's explicit quality
requirement. Do not use it as a unit-test-style substitute for checking final
state. Errors during learning may be informative; use interaction metrics to
compare error rate, retry rate, first-try success, token usage, and cost. See
`specs/evaluation.md`.
