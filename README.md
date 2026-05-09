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

1. **Validate without a real LLM**. `--dry-run` checks scenario selection,
   fixture setup, cache keys, and run planning without requiring real-run
   consent.
   ```bash
   llm-tool-test run --scenario example_basic --dry-run
   ```

2. **Enable real-run consent.** `LLM_TOOL_TEST_ENABLED=1` is required only for
   real agent execution. It prevents accidental LLM API spend and arbitrary
   agent-driven CLI execution in CI or shared environments.
   ```bash
   export LLM_TOOL_TEST_ENABLED=1
   ```

3. **List scenarios**: View built-in examples.
   ```bash
   llm-tool-test scenarios
   ```

4. **Print a scenario template**: Start from a schema-valid YAML example.
   ```bash
   llm-tool-test template scenario > fixtures/my_scenario.yaml
   ```

5. **Run evaluation**:
   ```bash
   llm-tool-test run --scenario example_basic --tool claude-code
   ```

6. **Review results**:
   ```bash
   cat llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/evaluation.md
   ```

## What You Get

Every run measures two dimensions:
1. **Outcome**: Did the agent complete the task?
2. **Efficiency**: How much friction (retries, errors, tokens) did it encounter?

### Artifacts
Each run appends a record to `llm-tool-test-results/results.jsonl` and generates a run directory containing:

- `evaluation.md`: A human-readable evaluation profile and summary.
- `report.md`: Execution details, gate results, and efficiency metrics.
- `metrics.json`: Machine-readable evaluation metrics (gates, interaction evidence, efficiency, composite score, and evaluator results).
- `artifacts/transcript.raw.txt`: The full agent transcript for debugging.
- `artifacts/events.jsonl`: Structured event log of the entire interaction.
- `artifacts/tool-output.raw.txt`: Raw adapter output when available.
- `artifacts/command-events.json`: Normalized command events when available.

### Metrics Example
```json
{
  "gates_passed": 3,
  "gates_total": 3,
  "details": [
    {
      "gate_type": "file_exists",
      "passed": true,
      "message": "File exists: summary.md"
    }
  ],
  "efficiency": {
    "total_commands": 6,
    "unique_commands": 5,
    "error_count": 0,
    "retry_count": 1,
    "help_invocations": 1,
    "first_try_success_rate": 1.0,
    "iteration_ratio": 0.83,
    "completed": true
  },
  "interaction_evidence_source": "structured_tool_calls"
}
```

Judge, composite, and custom evaluator fields are included only when configured — see the [evaluation spec](specs/evaluation.md).

## Scenario Format

Scenarios are YAML files defining the agent task, the environment, and post-run assertions. The minimal form:

```yaml
name: example_basic
description: Create a summary with the notes CLI
template_folder: example_basic

target:
  binary: notes
  env:
    NOTES_ROOT_DIR: "${LLM_TOOL_TEST_FIXTURE_DIR}"

task:
  prompt: |
    Use the notes CLI to create a project note and export a summary.

interaction:
  target_commands: required

evaluation:
  gates:
    - type: file_exists
      path: summary.md
    - type: file_contains
      path: summary.md
      substring: "Project"
```

For richer copyable examples, use:

```bash
llm-tool-test template scenario
llm-tool-test template config
llm-tool-test template script-gate
llm-tool-test template evaluator
```

See the [scenario spec](specs/scenarios.md) for setup hooks, custom evaluators, and the full field reference.

## Documentation

- [User guide](docs/user-guide.md): Commands, scenarios, and CI usage.
- [Scenario spec](specs/scenarios.md): Complete YAML format.
- [Evaluation spec](specs/evaluation.md): Scoring and metrics design.
- [Scripts spec](specs/scripts.md): Custom evaluators and post-scripts.
