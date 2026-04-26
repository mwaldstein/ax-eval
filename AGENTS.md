# AGENTS.md - llm-tool-test

This file provides guidance for LLM agents working on the llm-tool-test codebase.

## Project Overview

`llm-tool-test` is an evaluation framework for measuring how effectively LLM coding agents use CLI tools. It produces dimensional evaluation profiles (quantitative + qualitative scalar measurements), not just binary pass/fail stamps. Gates exist as fail-fast checks; the real value is in the evaluation profile — enabling comparisons like "does a new model increase token usage?" or "did the richer AGENTS.md reduce the error rate?"

## Project Structure

```
llm-tool-test/
├── src/
│   ├── main.rs              # CLI entry point, command dispatch
│   ├── lib.rs               # Library exports
│   ├── commands.rs          # CLI command implementations
│   ├── config.rs            # Configuration loading/management
│   ├── evaluation.rs        # Gate evaluation and scoring
│   ├── eval_helpers.rs      # Evaluation helper functions
│   ├── eval_tests_score.rs  # Score-related tests
│   ├── fixture.rs           # Test fixture utilities
│   ├── judge.rs             # LLM-as-judge implementation
│   ├── output.rs            # Console output formatting
│   ├── run/                 # Run execution logic
│   │   ├── mod.rs           # Main run orchestration
│   │   ├── cache.rs         # Result caching
│   │   ├── execution.rs     # Scenario execution flow
│   │   ├── records.rs       # Result record building
│   │   ├── setup.rs         # Scenario setup
│   │   └── transcript.rs    # Transcript writing
│   ├── adapter/             # LLM tool adapters
│   │   ├── claude_code.rs   # Claude Code adapter
│   │   ├── codex.rs         # OpenAI Codex adapter
│   │   ├── mock.rs          # Mock adapter for testing
│   │   ├── mock_test.rs     # Mock adapter tests
│   │   ├── opencode.rs      # OpenCode adapter
│   │   └── types.rs         # Adapter types and traits
│   ├── scenario/            # Scenario loading/parsing
│   │   ├── mod.rs           # Scenario loading
│   │   ├── types.rs         # Scenario type definitions
│   │   └── tests/           # Scenario parsing tests
│   ├── transcript/          # Transcript processing
│   │   ├── analyzer.rs      # Command extraction/analysis
│   │   ├── logging.rs       # Event logging
│   │   ├── redact.rs        # Secret redaction
│   │   ├── types.rs         # Transcript type definitions
│   │   ├── writer.rs        # Report generation
│   │   └── tests/           # Transcript tests
│   ├── results/             # Results storage
│   │   ├── db.rs            # SQLite results database
│   │   ├── types/           # Result type definitions
│   │   └── utils.rs         # Result utilities
│   ├── session.rs           # Shell session management
│   └── script_runner.rs     # Script execution utility
├── specs/                   # Design specifications
│   ├── scenarios.md         # Scenario format spec
│   ├── evaluation.md        # Evaluation layer spec
│   ├── scripts.md           # Scripts system spec
│   ├── llm-user-validation.md  # Testing harness architecture
│   └── distribution.md      # Distribution/packaging spec
├── tests/cli.rs             # CLI integration tests
└── fixtures/                # Example scenarios and fixtures
    ├── example_basic.yaml
    ├── example_gates_comprehensive.yaml
    ├── example_judge.yaml
    ├── example_full_config.yaml
    ├── example_guidance_minimal.yaml
    ├── example_guidance_rich.yaml
    └── templates/             # Fixture template directories
        ├── example_basic/
        ├── example_gates_comprehensive/
        ├── example_judge/
        ├── example_full_config/
        ├── notes_guidance_minimal/
        └── notes_guidance_rich/
```

## Build Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run linting
cargo clippy -- -D warnings

# Check formatting
cargo fmt --check

# Run a specific scenario
cargo run -- run --scenario <name>

# Run with specific tool
cargo run -- run --scenario <name> --tool opencode

# List available scenarios
cargo run -- scenarios
```

## Key Concepts

### Evaluation, Not Testing

This framework evaluates, it does not merely test. The distinction:

- **Testing** asks _"Did it pass?"_ — binary, terminable, final.
- **Evaluation** asks _"How well did it go?"_ — scalar, dimensional, comparative.

The primary output is the evaluation profile (quantitative + qualitative measurements on continuous scales), not a Pass/Fail stamp. Gates are fail-fast — they catch catastrophic failures early so you don't waste expensive judge calls on dead runs. But gates are not the point. The point is the profile.

Binary gates are also a valid CI/CD use of the framework — a token-limit or cost gate that fails when a model or harness change breaks an assumption. The gate catches the regression; the profile tells you what changed and by how much.

### Scenarios
A scenario is a YAML file defining:
- **Target tool**: The CLI tool being evaluated
- **Task prompt**: Instructions given to the LLM agent
- **Gates**: Binary fail-fast assertions evaluated after the run
- **Scripts**: Custom evaluation logic (post scripts, evaluators, script gates)
- **Judge**: Optional rubric-based qualitative assessment

### Gates
Gates are binary fail-fast assertions that catch catastrophic failures early. They are not the primary evaluation output — the evaluation profile (scalar metrics + judge score) is:
- `command_succeeds`: Shell command returns exit 0
- `command_output_contains`: Command stdout contains substring
- `command_output_matches`: Command stdout matches regex
- `command_json_path`: Navigate JSON output, apply assertion
- `file_exists`: File exists in fixture directory
- `file_contains`: File contains substring
- `file_matches`: File content matches regex
- `no_transcript_errors`: No target-tool commands failed
- `script`: Custom script gate with structured output

### Adapters
Adapters interface with LLM tools (OpenCode, Claude Code, Codex). Each adapter:
- Spawns the tool as a child process
- Captures output via PTY
- Returns structured transcript and events

### Scripts System
Scripts extend the framework without modifying core code:
- **Post scripts**: Run after agent exits, before evaluation
- **Script gates**: Custom pass/fail logic
- **Evaluators**: Produce custom metrics/scores

## Common Tasks

### Adding a New Gate Type

1. Add variant to `Gate` enum in `src/scenario/types.rs`
2. Add deserialization test in `src/scenario/tests/gates.rs`
3. Implement evaluator in `src/evaluation.rs` gate dispatch
4. Add unit tests in `src/evaluation.rs` test module

### Adding a New Adapter

1. Create new file in `src/adapter/<name>.rs`
2. Implement `ToolAdapter` trait
3. Register in `src/adapter/mod.rs`
4. Add to adapter dispatch in `src/run/execution.rs`
5. Add integration test in `tests/cli.rs`
6. Add e2e test in `tests/e2e.rs` if the tool supports real-LLM execution

### Updating Scenario Schema

1. Modify types in `src/scenario/types.rs`
2. Update all YAML test fixtures
3. Update relevant spec in `specs/`
4. Run tests: `cargo test`

### Debugging Test Failures

1. Run specific test: `cargo test <test_name>`
2. Run with output: `cargo test <test_name> -- --nocapture`
3. Check scenario fixture: `fixtures/<name>/`
4. Review results: `llm-tool-test-results/`

## Code Conventions

- **Error handling**: Use `anyhow` for error propagation
- **Async**: Minimal async; prefer sync where possible
- **Tests**: Unit tests alongside source, integration tests in `tests/`
- **Types**: Use `serde` for YAML/JSON serialization
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types

## Guidance Testing Examples

The framework includes comparative examples for testing how AGENTS.md guidance quality affects LLM tool usage:

- **`example_guidance_minimal`** — Bare-bones AGENTS.md with just command names. The LLM must discover init requirements, ID capture, linking rules, and search behavior via trial and error.
- **`example_guidance_rich`** — Detailed AGENTS.md with examples, workflows, error handling tables, and constraints. Should produce better interaction metrics.

Both use the same `notes` mock tool, same task prompt, and same evaluation gates. Run them together:

```bash
llm-tool-test run --all --tags guidance-test
```

Compare `metrics.json` across results to see which guidance produces:
- Lower error rates and help-seeking
- Higher first-try success rate
- Fewer total commands and lower token usage
- Lower cost per task completion

## Specs Reference

- `specs/scenarios.md` - Scenario YAML format
- `specs/evaluation.md` - Evaluation layers (quantitative + qualitative)
- `specs/scripts.md` - Script hooks and contracts
- `specs/llm-user-validation.md` - Architecture overview

## Testing Approach

The framework is tested against itself using mock scenarios. Key test areas:
- Scenario parsing (YAML → Rust types)
- Gate evaluation (all gate types)
- Script execution (env vars, timeouts, exit codes)
- Adapter interfaces (mock adapter tests)
- CLI commands (integration tests in `tests/cli.rs`)
- Transcript analysis (command extraction, metrics)

### Evaluation Philosophy

Gates verify **outcomes**, not **process**. Because LLMs are non-deterministic,
a scenario should ask "did the task get done?" rather than "did the LLM follow
my expected steps without deviation?"

More importantly: gates are **fail-fast**, not **evaluation**. The framework's
primary output is the evaluation profile (scalar measurements across all three
layers), not a binary pass/fail stamp. Gates catch catastrophic failures; the
profile captures nuance.

- **Good gate**: `file_exists: report.pdf` — did the output get created?
- **Bad gate**: `no_transcript_errors` in a discovery scenario — errors during
  learning are expected and informative.

For guidance testing (`example_guidance_minimal` vs `example_guidance_rich`),
the primary signal is **interaction metrics** (error rate, retry rate, first-try
success, token usage, cost), not gate pass rate. Gates are a minimal sanity
check that the task was completed. See `specs/evaluation.md` for the full
design rationale.

### Fixture Scenario Regression Tests

Every example scenario in `fixtures/` has an automated regression test in
`src/fixture_tests.rs`. These run the mock-tool commands directly (no real LLM),
execute setup/post scripts, and verify all evaluation gates pass.

```bash
cargo test fixture_tests
```

These are the automated e2e tests -- they validate the framework and the scenario
definitions themselves run correctly.

### Running All Examples with a Real LLM

All built-in example scenarios share the `examples` tag. Run them together with
a real LLM tool via the CLI:

```bash
# List example scenarios
llm-tool-test scenarios --tags examples

# Run all examples with opencode
llm-tool-test run --all --tags examples --tool opencode

# Run all examples with a specific model
llm-tool-test run --all --tags examples --tool opencode --model opencode/kimi-k2.6
```

### Real-LLM End-to-End Tests

Tests in `tests/e2e.rs` exercise the framework with actual LLM adapters. They
require an installed and authenticated LLM tool (opencode, claude, claude-code, or codex)
and are gated behind `LLM_TOOL_TEST_E2E=1`:

```bash
LLM_TOOL_TEST_ENABLED=1 LLM_TOOL_TEST_E2E=1 cargo test --test e2e
```

These are for manual validation or CI environments with LLM credentials. They
verify artifact generation (transcript, metrics, evaluation report) with real
adapters.

## Dependencies

Key crates:
- `serde` / `serde_yaml` / `serde_json` - Serialization
- `regex` - Pattern matching
- `wait-timeout` - Script timeouts
- `chrono` - Timestamps
- `serde_json` / `serde_yaml` - Serialization (also used for results database)
- `tempfile` - Test fixtures

See `Cargo.toml` for complete list.

## Contact

For issues: https://github.com/anomalyco/llm-tool-test/issues
