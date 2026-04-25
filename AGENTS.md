# AGENTS.md - llm-tool-test

This file provides guidance for LLM agents working on the llm-tool-test codebase.

## Project Overview

`llm-tool-test` is a testing framework for evaluating LLM coding agents against structured test scenarios. It runs scenarios that test how well LLM agents use CLI tools.

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

### Scenarios
A scenario is a YAML file defining:
- **Target tool**: The CLI tool being tested
- **Task prompt**: Instructions given to the LLM agent
- **Gates**: Pass/fail assertions evaluated after the run
- **Scripts**: Custom evaluation logic (post scripts, evaluators, script gates)

### Gates
Gates are evaluation assertions:
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
Adapters interface with LLM tools (OpenCode, Claude Code). Each adapter:
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
4. Add to config parsing in `src/config.rs`
5. Add integration test in `tests/cli.rs`

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

## Specs Reference

- `specs/scenarios.md` - Scenario YAML format
- `specs/evaluation.md` - Gate types and evaluation layers
- `specs/scripts.md` - Script hooks and contracts
- `specs/llm-user-validation.md` - Architecture overview

## Testing Approach

The framework is tested against itself using mock scenarios. Key test areas:
- Scenario parsing (YAML → Rust types)
- Gate evaluation (all gate types)
- Script execution (env vars, timeouts, exit codes)
- Adapter interfaces (mock adapter tests)
- CLI commands (integration tests)
- Transcript analysis (command extraction, metrics)

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
