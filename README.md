# LLM Tool Test

Evaluate how well coding agents can use your CLI.

`llm-tool-test` is an evaluation framework for agent-facing command-line tools. It runs real coding agents against your CLI in isolated scenarios, captures the full transcript, and produces an evaluation profile with interaction metrics, outcome gates, and optional rubric-based judge scoring.

Agents are users now. Your CLI can be perfectly usable by humans and still expensive for agents: wrong subcommands, missed setup, retries after vague errors, extra token burn, or model-specific confusion. `llm-tool-test` turns those failures into transcripts and metrics you can compare across docs, help text, prompts, AGENTS.md guidance, models, and releases.

Today, `llm-tool-test` focuses on CLIs because command-line tools are the interface coding agents already use. The broader problem is agent-tool interaction: can an agent discover, operate, and recover from mistakes with the tools we give it?

## Who It Is For

`llm-tool-test` is primarily for CLI authors who want to know whether coding agents can operate their tools successfully and efficiently.

It is also useful for people writing agent-facing documentation, AGENTS.md files, coding-agent skills, or evaluation harnesses that need repeatable evidence about how model and guidance changes affect tool usage.

An agent-facing CLI is any command-line tool you expect coding agents to install, inspect, and operate through docs, `--help`, README files, AGENTS.md guidance, or task prompts.

## What It Helps You Decide

- Did a documentation, help text, or AGENTS.md change reduce retries and errors?
- Where exactly did the agent misunderstand the CLI?
- Which agent tool or model handles this workflow more reliably?
- Did a new model improve success while increasing token usage or cost?
- Did a CLI release make agent usage worse?

`llm-tool-test` is not a public leaderboard. It gives you repeatable, scenario-based evaluations for the CLI workflows your users and agents actually need.

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

Each run answers two questions at the same time:

1. Did the agent complete the task?
2. How much friction did the agent hit along the way?

The result is an evaluation profile you can compare across scenario runs. It combines:

- **Interaction metrics**: command count, error rate, first-try success rate, duration, token usage, and cost when available
- **Outcome gates**: fail-fast assertions such as files existing, commands succeeding, or JSON output matching expectations
- **Optional judge scoring**: rubric-based qualitative assessment of whether the agent used the tool as intended
- **Transcript evidence**: the full interaction showing where the agent succeeded, hesitated, retried, or went off course

Each run produces a directory under `llm-tool-test-results/` with artifacts such as:

- `evaluation.md`: human-readable evaluation profile
- `metrics.json`: machine-readable scalar metrics
- `transcript.raw.txt`: full agent transcript for debugging
- `events.jsonl`: structured event log

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

When a run produces surprising results, the transcript shows where the agent went wrong: unclear docs, missing setup guidance, confusing help output, fragile command behavior, or a model-specific failure.

## What a Scenario Looks Like

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

It evaluates a different question: how well does a coding agent operate the CLI as a user?

Unit and integration tests tell you whether the CLI works. Simple evals tell you whether an agent eventually produced the right output. `llm-tool-test` shows how the interaction went: retries, wrong turns, token and cost burn, transcript evidence, and qualitative rubric scoring.

That makes it a combined integration-test and evaluation harness. Scenarios give you repeatable setup, execution, gates, scripts, and artifacts. The evaluation profile gives you scalar measurements and qualitative evidence you can compare across documentation changes, model changes, prompt changes, and CLI releases.

## When To Use It

Use `llm-tool-test` when you want to evaluate agent interaction with a CLI: workflow discoverability, documentation quality, model behavior, prompt guidance, tool ergonomics, or comparative performance across repeated scenarios.

Do not use it as a substitute for deterministic test coverage. If you need to verify a parser, API, database migration, or command implementation, write normal tests first. Use `llm-tool-test` when the operator is the thing being evaluated.

## Documentation

- [User guide](docs/user-guide.md): commands, scenarios, gates, configuration, results, troubleshooting, and CI usage
- [Scenario spec](specs/scenarios.md): complete scenario YAML format
- [Evaluation spec](specs/evaluation.md): evaluation profile design and scoring rationale
- [Scripts spec](specs/scripts.md): post scripts, script gates, and custom evaluators
- [Distribution spec](specs/distribution.md): packaging and installer behavior
