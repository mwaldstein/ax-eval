# User Guide

This guide covers day-to-day usage of `llm-tool-test`: running scenarios, authoring scenario files, reading results, configuring matrix runs, and troubleshooting.

For the high-level value proposition, start with the [README](../README.md). For complete schema details, see the specs in [`specs/`](../specs/).

## Safety Flag

`llm-tool-test` will not launch agent runs unless explicitly enabled:

```bash
export LLM_TOOL_TEST_ENABLED=1
```

This prevents accidental expensive LLM API calls.

## Runtime Agent Tools

Runtime adapters currently support:

- `claude-code`
- `opencode`
- `codex`

The selected tool must be installed and authenticated before you run a scenario.

The `mock` adapter is internal test support for adapter plumbing. It does not execute the target tool or mutate fixture state, so it is not intended for validating scenario outcomes.

## Basic Commands

List available scenarios:

```bash
llm-tool-test scenarios
```

Filter by tags or tier:

```bash
llm-tool-test scenarios --tags examples
llm-tool-test scenarios --tier 0
```

Run a single scenario:

```bash
llm-tool-test run --scenario example_basic --tool claude-code
```

Run all scenarios:

```bash
llm-tool-test run --all --tool claude-code
```

Filter runs by tag or tier:

```bash
llm-tool-test run --all --tags smoke --tool claude-code
llm-tool-test run --all --tier 1 --tool claude-code
```

Dry run without LLM calls:

```bash
llm-tool-test run --scenario example_basic --dry-run
```

Show run details:

```bash
llm-tool-test show <run-id>
```

Clean artifacts:

```bash
# Clean cache and legacy transcript artifacts older than 7 days
llm-tool-test clean --older-than "7d"

# Clean cache and legacy transcript artifacts
llm-tool-test clean
```

## Typical Workflow

```bash
# 1. Enable the safety flag
export LLM_TOOL_TEST_ENABLED=1

# 2. List available scenarios
llm-tool-test scenarios

# 3. Run a scenario with an agent tool
llm-tool-test run --scenario example_basic --tool claude-code

# 4. Check the evaluation profile
cat llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/evaluation.md

# 5. Review the transcript for debugging
cat llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/transcript.raw.txt
```

## Scenario Authoring

A scenario describes the task given to the agent, the fixture environment it works in, and the evaluation that runs after the agent exits.

```yaml
name: example_basic
description: Create a summary with the notes CLI
tags: [examples, smoke]
tier: 0

prompt: |
  Use the notes CLI to create a project note and export a summary.

gates:
  - type: file_exists
    path: summary.md
  - type: file_contains
    path: summary.md
    contains: "Project"
```

Good scenarios evaluate outcomes, not the exact process an agent used. Because LLMs are nondeterministic, prefer checks like "was the summary created?" over "did the agent run these exact commands in this exact order?"

Use gates as fail-fast sanity checks. The richer evaluation signal comes from the metrics, transcript, and optional judge rubric.

For the complete scenario schema, see the [scenario spec](../specs/scenarios.md).

## Gate Types

Gates are binary assertions evaluated after the agent completes the task. They catch catastrophic failures before you spend time on deeper analysis.

- `command_succeeds`: shell command exits successfully
- `command_output_contains`: command stdout contains an expected substring
- `command_output_matches`: command stdout matches a regex pattern
- `command_json_path`: JSON output contains data matching a path assertion
- `file_exists`: file exists in the fixture directory
- `file_contains`: file content contains an expected substring
- `file_matches`: file content matches a regex pattern
- `no_transcript_errors`: no command errors detected in the transcript
- `script`: custom script gate using exit code or JSON output

For discovery-heavy scenarios, be cautious with `no_transcript_errors`. Errors during learning may be informative rather than a failed outcome.

## Tool, Model, And Matrix Runs

Run a scenario with a specific agent tool:

```bash
llm-tool-test run --scenario example_basic --tool claude-code
```

Pass a model when supported by the adapter:

```bash
llm-tool-test run --scenario example_basic --tool claude-code --model claude-sonnet
```

Use a specific judge tool and model for LLM-as-judge evaluation. Judge tool selection is independent of the agent tool:

```bash
llm-tool-test run --scenario example_judge \
  --tool codex \
  --judge-tool claude-code \
  --judge-model claude-sonnet
```

Run a configured matrix with a profile from `llm-tool-test-config.toml`:

```bash
llm-tool-test run --all --profile quick
```

Scenarios can also define a `tool_matrix` to run multiple tool/model combinations without CLI flags.

## Configuration

An optional `llm-tool-test-config.toml` can define tool/model validation, model costs, and matrix profiles.

```toml
[tools.claude-code]
name = "claude-code"
command = "claude-code"
models = ["claude-sonnet"]

[tools.opencode]
name = "opencode"
command = "opencode"
models = ["gpt-4o", "claude-sonnet"]

[profiles.quick]
name = "quick"
tools = ["claude-code"]
models = ["claude-sonnet"]

[models.claude-sonnet]
input_cost_per_1k_tokens = 3.0
output_cost_per_1k_tokens = 15.0
```

Copy `llm-tool-test-config.example.toml` as a starting point.

## Interpreting Results

Each run generates an `evaluation.md` with a full evaluation profile.

The profile includes:

- Summary: scenario name, tool, model, and outcome
- Quantitative metrics: gates passed, duration, cost, token usage, command count, error rate, and first-try success rate
- Qualitative scoring: judge score and judge issues/highlights when a rubric is configured
- Composite score: optional scenario-configured score from `0.0` to `1.0`
- Human review section: space for manual scoring
- Artifact links: transcript, metrics, and events

Use `metrics.json` when comparing repeated runs programmatically.

Common comparisons:

- Did the new model reduce token usage?
- Did richer AGENTS.md guidance lower the error rate?
- Did a help-text change improve first-try success?
- Does one adapter complete the same workflow with fewer retries?

## Results Location

All run artifacts are stored in:

```text
llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/
```

Typical artifacts include:

- `evaluation.md`
- `metrics.json`
- `transcript.raw.txt`
- `events.jsonl`

## CI And Regression Gates

CI/CD is an advanced but valid use case. Gates can fail builds when a model, harness, prompt, or documentation change breaks an assumption such as task completion, token budget, or generated artifact shape.

Use CI gates for catastrophic regressions. Use the evaluation profile to understand what changed and by how much.

## Troubleshooting

**"LLM testing is disabled"**: set `LLM_TOOL_TEST_ENABLED=1`.

**Scenario not found**: check that the scenario exists in `fixtures/`, then run `llm-tool-test scenarios`.

**Gate failures**: inspect `evaluation.md`, `metrics.json`, and `transcript.raw.txt`.

**Timeout errors**: increase timeout with `--timeout-secs 600`.

**Cache issues**: disable caching with `--no-cache` or run `llm-tool-test clean`.

**Composite score low**: review which gates failed and whether judge scoring found qualitative issues.

**Tool not supported**: use one of the runtime adapters: `claude-code`, `opencode`, or `codex`.

## Built-In Example Scenarios

All built-in example scenarios share the `examples` tag:

```bash
llm-tool-test scenarios --tags examples
```

Run all examples with Claude Code:

```bash
llm-tool-test run --all --tags examples --tool claude-code
```

The guidance examples compare minimal and rich AGENTS.md instructions:

- `example_guidance_minimal`: bare-bones guidance, requiring more discovery
- `example_guidance_rich`: detailed workflows, examples, and error handling

Compare their `metrics.json` files to see whether richer guidance reduced command count, retries, error rate, token usage, or cost.
