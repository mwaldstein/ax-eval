# LLM Tool Test - TODO List

> **Note**: This file tracks remaining work and future enhancements.

## Completed

- ✅ Remove snapshot mechanism and qipu-specific code
- ✅ Add target tool configuration to scenarios
- ✅ Implement generic gate system (9 gate types)
- ✅ Implement scripts system (post scripts, script gates, evaluators)
- ✅ Update README with generic gates
- ✅ Update config example
- ✅ Create AGENTS.md for the project
- ✅ Create example scenario with scripts (`fixtures/example_basic/`)
- ✅ Review specs against implementation (no deviations found)
- ✅ Remove dead code and address clippy warnings
- ✅ Make composite score conditional in reports and summaries
- ✅ Add LICENSE and CHANGELOG

## Release Blockers

None. All planned work is complete.

## Future Enhancements

These are not part of the current release plan.

### Features

- Budget enforcement (`--max-usd` flag) - CLI accepts it but no enforcement logic
- Cost estimation completion for adapters
- HTML report generation option
- JUnit XML output for CI integration
- Parallel scenario execution
- Scenario validation command (dry-run but with schema checks)
- Interactive scenario debugger
- Global `[target]` section in config file (currently only per-scenario)

### Documentation

- Add CONTRIBUTING.md guide
- Add troubleshooting guide for common issues
- Add CI/CD integration examples
- Create video walkthrough of scenario creation
- Document install scripts and Homebrew formula in `distribution.md`

## Known Minor Issues

- `succeeded()` method on `ScriptResult` is only used in tests (marked `#[allow(dead_code)]`)
- `run_judge` async functions in `src/judge/eval.rs` are test-only; production uses CLI-based judge execution
