# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
