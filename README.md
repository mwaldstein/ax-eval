# LLM Tool Test

Evaluate how coding agents use your CLI.

**Status:** Pre-1.0; interfaces may change. Supports `claude-code`, `opencode`, and `codex` on macOS, Linux, and Windows.

CLIs built for humans often frustrate AI agents — vague errors, complex setup, confusing subcommands — leading to retries and wasted tokens.

`llm-tool-test` runs coding agents against your CLI in reproducible scenarios and measures success, friction, and cost: pass/fail gates, scalar metrics, and optional rubric-based scores. Use it to drive a tight feedback loop on your CLI ergonomics, `--help` text, and `AGENTS.md` guidance.

Built primarily for CLI authors. Also useful for technical writers iterating on `AGENTS.md` and agent developers comparing models on a specific workflow.

## Why Not Just Tests?

Unit tests verify that your CLI works. `llm-tool-test` verifies that an agent can *discover and use* it.

Traditional tests are deterministic. Agents are not. `llm-tool-test` captures the qualitative friction of an agent operator — wrong turns, retries, and token burn — while providing repeatable setup and objective gates.

Repeatability here is about benchmarking changes to your environment: does the same scenario perform better after a model upgrade or a documentation rewrite? Scalar metrics give you trend data; **rubric-based Judge scoring** gives you a repeatable signal on the quality of the interaction, not just a binary pass/fail.

## How It Works

1. **Modify** your CLI's `--help`, `AGENTS.md`, or documentation.
2. **Execute** a scenario — `llm-tool-test` runs the configured agent CLI against your prompt and records every command, error, and token.
3. **Analyze** the metrics, transcript, and Judge score to see exactly where the agent hesitated.
4. **Refine** and repeat.

## Install

Install the latest release on macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/mwaldstein/llm-tool-test/master/scripts/install.sh | sh
```

Install the latest release on Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/mwaldstein/llm-tool-test/master/scripts/install.ps1 | iex
```

To build from source:

```bash
cargo build --release
# Binary will be at target/release/llm-tool-test
```

## Quickstart

You need an installed and authenticated agent CLI (`claude-code`, `opencode`, or `codex`).

1. **Enable the safety flag.** Required to launch agent runs — prevents accidental execution in CI or shared environments, where runs would incur token cost and execute arbitrary CLI commands.
   ```bash
   export LLM_TOOL_TEST_ENABLED=1
   ```

2. **List scenarios**: View built-in examples.
   ```bash
   llm-tool-test scenarios
   ```

3. **Run evaluation**:
   ```bash
   llm-tool-test run --scenario example_basic --tool claude-code
   ```

4. **Review results**:
   ```bash
   cat llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/evaluation.md
   ```

## What You Get

Every run measures two dimensions:
1. **Outcome**: Did the agent complete the task?
2. **Efficiency**: How much friction (retries, errors, tokens) did it encounter?

### Artifacts
Each run generates a directory under `llm-tool-test-results/` containing:

- `evaluation.md`: A human-readable evaluation profile and summary.
- `metrics.json`: Machine-readable scalar metrics (commands, errors, tokens, cost).
- `transcript.raw.txt`: The full agent transcript for debugging.
- `events.jsonl`: Structured event log of the entire interaction.

### Metrics Example
```json
{
  "scenario": "example_basic",
  "tool": "claude-code",
  "gates_passed": 3,
  "gates_total": 3,
  "command_count": 6,
  "error_rate": 0.0,
  "first_try_success_rate": 1.0,
  "token_usage": { "input": 14230, "output": 1840 },
  "judge_score": 0.92
}
```

`judge_score` is included only when a scenario configures a Judge rubric — see the [evaluation spec](specs/evaluation.md).

## Scenario Format

Scenarios are YAML files defining the agent task, the environment, and post-run assertions. The minimal form:

```yaml
name: example_basic
description: Create a summary with the notes CLI
prompt: |
  Use the notes CLI to create a project note and export a summary.
gates:
  - type: file_exists
    path: summary.md
  - type: file_contains
    path: summary.md
    contains: "Project"
```

See the [scenario spec](specs/scenarios.md) for setup hooks, custom evaluators, and the full field reference.

## Documentation

- [User guide](docs/user-guide.md): Commands, scenarios, and CI usage.
- [Scenario spec](specs/scenarios.md): Complete YAML format.
- [Evaluation spec](specs/evaluation.md): Scoring and metrics design.
- [Scripts spec](specs/scripts.md): Custom evaluators and post-scripts.
