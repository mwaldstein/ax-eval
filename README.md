# LLM Tool Test

Benchmark how well LLM agents can use your CLI.

`llm-tool-test` runs real coding agents against your command-line tool in isolated scenarios, captures full transcripts, and turns the run into an evaluation profile: quantitative interaction metrics, outcome gates, and optional rubric-based judge scoring.

Agents are users now. A CLI that feels obvious to a human can still confuse an agent: the agent may choose the wrong subcommand, miss required setup, retry after unclear errors, or spend far more tokens than expected. `llm-tool-test` helps tool authors see those failures directly and compare whether changes to docs, help text, prompts, models, or AGENTS.md guidance actually improved agent behavior.

## Who It Is For

`llm-tool-test` is primarily for CLI authors who want to know whether LLM agents can operate their tools successfully.

It is also useful for people writing agent-facing documentation, AGENTS.md files, coding-agent skills, or evaluation harnesses that need to compare model and guidance changes over time.

## Install

Install the latest GitHub release on macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/mwaldstein/llm-tool-test/master/scripts/install.sh | sh
```

Install the latest GitHub release on Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/mwaldstein/llm-tool-test/master/scripts/install.ps1 | iex
```

Installers ignore prereleases by default. Set `LLM_TOOL_TEST_INCLUDE_PRERELEASES=1` to install the newest beta release, or set `LLM_TOOL_TEST_VERSION` to install an exact version.

To build from source:

```bash
cargo build --release
# Binary will be at target/release/llm-tool-test
```

## Quickstart

You need an installed and authenticated agent CLI. Runtime adapters currently support `claude-code`, `opencode`, and `codex`; the examples below use `claude-code`.

`llm-tool-test` requires an explicit safety flag before it will launch agent runs:

```bash
export LLM_TOOL_TEST_ENABLED=1
```

List the built-in scenarios:

```bash
llm-tool-test scenarios
```

Run one example:

```bash
llm-tool-test run --scenario example_basic --tool claude-code
```

Open the generated evaluation report:

```bash
cat llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/evaluation.md
```

For detailed commands, scenario authoring, configuration, troubleshooting, and CI usage, see the [user guide](docs/user-guide.md).

## What You Get

Each run produces a directory under `llm-tool-test-results/` with artifacts such as:

- `evaluation.md`: human-readable evaluation profile
- `metrics.json`: machine-readable scalar metrics
- `transcript.raw.txt`: full agent transcript for debugging
- `events.jsonl`: structured event log

An evaluation profile combines:

- **Interaction metrics**: command count, error rate, first-try success rate, duration, token usage, and cost when available
- **Outcome gates**: fail-fast assertions such as files existing, commands succeeding, or JSON output matching expectations
- **Optional judge scoring**: rubric-based qualitative assessment of whether the agent used the tool as intended

Example metrics shape:

```json
{
  "scenario": "example_basic",
  "tool": "claude-code",
  "gates_passed": 3,
  "gates_total": 3,
  "command_count": 6,
  "error_rate": 0.0,
  "first_try_success_rate": 1.0,
  "token_usage": {
    "input": 14230,
    "output": 1840
  },
  "judge_score": 0.92
}
```

The transcript is a first-class artifact. When a run produces surprising results, it shows where the agent went wrong: unclear docs, missing setup guidance, confusing help output, fragile command behavior, or a model-specific failure.

## What A Scenario Looks Like

Scenarios are YAML files that describe the agent task, the fixture environment, and the checks to run after the agent exits.

```yaml
name: example_basic
description: Create a summary with the notes CLI
tags: [examples, smoke]

prompt: |
  Use the notes CLI to create a project note and export a summary.

gates:
  - type: file_exists
    path: summary.md
  - type: file_contains
    path: summary.md
    contains: "Project"
```

See [docs/user-guide.md](docs/user-guide.md) for the practical authoring guide and [specs/scenarios.md](specs/scenarios.md) for the full scenario format.

## Why Not Just Tests?

`llm-tool-test` does not replace unit tests, integration tests, or end-to-end tests. Those should still verify deterministic behavior in your CLI.

This framework evaluates a different question: how well does an LLM agent operate the CLI as a user?

Traditional tests ask, "Did the system pass?" `llm-tool-test` asks, "How well did the agent interaction go?" Gates catch catastrophic failures, but the primary output is the evaluation profile: scalar measurements and qualitative evidence you can compare across runs.

## Questions You Can Answer

- Did a new model reduce command retries or increase token usage?
- Did richer AGENTS.md guidance lower the error rate?
- Did better `--help` text improve first-try success?
- Does `claude-code`, `opencode`, or `codex` handle this workflow more reliably?
- Where exactly did the agent misunderstand the CLI?

## When To Use It

Use `llm-tool-test` when you want to evaluate agent interaction with a CLI: workflow discoverability, documentation quality, model behavior, prompt guidance, tool ergonomics, or comparative performance across repeated scenarios.

Do not use it as a substitute for deterministic test coverage. If you need to verify a parser, API, database migration, or command implementation, write normal tests first. Use `llm-tool-test` when the operator is the thing being evaluated.

## Documentation

- [User guide](docs/user-guide.md): commands, scenarios, gates, configuration, results, troubleshooting, and CI usage
- [Scenario spec](specs/scenarios.md): complete scenario YAML format
- [Evaluation spec](specs/evaluation.md): evaluation profile design and scoring rationale
- [Scripts spec](specs/scripts.md): post scripts, script gates, and custom evaluators
- [Distribution spec](specs/distribution.md): packaging and installer behavior
