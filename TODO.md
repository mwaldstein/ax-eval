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

- Budget enforcement (`--max-usd` flag or equivalent) with actual enforcement logic
- Cost estimation completion for adapters
- HTML report generation option
- JUnit XML output for CI integration
- Parallel scenario execution
- Scenario validation command (dry-run but with schema checks)
- Interactive scenario debugger
- Global `[target]` section in config file (currently only per-scenario)
- Native skills support — formal plugin/skill system for tool-specific LLM guidance beyond AGENTS.md
- Expand e2e test scenarios beyond `example_e2e` (currently minimal smoke test)
- Add session size test gate — measures token count or text size at completion, allowing comparison of guidance quality (poor guidance = larger session = more trial-and-error)
- Add native metrics comparison command — `llm-tool-test compare --run1 <id> --run2 <id>` for with/without guidance evaluation

### Documentation

- Add CONTRIBUTING.md guide
- Add troubleshooting guide for common issues
- Add CI/CD integration examples
- Create video walkthrough of scenario creation
- Document install scripts and Homebrew formula in `distribution.md`

## Known Minor Issues

- `succeeded()` method on `ScriptResult` is only used in tests (marked `#[allow(dead_code)]`)
- `run_judge` async functions in `src/judge/eval.rs` are test-only; production uses CLI-based judge execution
