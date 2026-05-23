# Agent Experience Eval

Evaluate how coding agents use your CLI.

Supports `claude-code`, `opencode`, and `codex` on macOS, Linux, and Windows.

CLIs built for humans often frustrate AI agents — vague errors, complex setup, confusing subcommands — leading to retries and wasted tokens.

`ax-eval` runs coding agents against your CLI in reproducible scenarios and produces an evaluation profile: quantitative interaction metrics, cost and token data, optional qualitative rubric scores, and supporting guardrail checks. Use it to drive a tight feedback loop on your CLI ergonomics, `--help` text, and `AGENTS.md` guidance.

Built primarily for CLI authors. Also useful for technical writers iterating on `AGENTS.md` and agent developers comparing models on a specific workflow.

## Why Not Just Tests?

Unit tests verify that your CLI works. `ax-eval` verifies that an agent can *discover and use* it.

Traditional tests are deterministic. Agents are not. `ax-eval` captures the qualitative friction of an agent operator — wrong turns, retries, and token burn — while providing repeatable setup and objective guardrails.

Repeatability here is about benchmarking changes to your environment: does the same scenario perform better after a model upgrade or a documentation rewrite? Scalar metrics give you trend data; **rubric-based Judge scoring** gives you a repeatable qualitative signal on the quality of the interaction. Pass/fail gates are supporting checks for catastrophic failures, not the main result.

## How It Works

1. **Modify** your CLI's `--help`, `AGENTS.md`, or documentation.
2. **Execute** a scenario — `ax-eval` runs the configured agent CLI against your prompt and records every command, error, and token.
3. **Analyze** the metrics, transcript, and Judge score to see exactly where the agent hesitated.
4. **Refine** and repeat.

If you do not have scenarios yet, start with `discover`: it asks an agent to
inspect your executable, write an understanding document, author five
goal-oriented scenarios, run them, and summarize what the results reveal about
your CLI's LLM usability.

## Install

Install the latest release on macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/mwaldstein/ax-eval/master/scripts/install.sh | sh
```

Install the latest release on Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/mwaldstein/ax-eval/master/scripts/install.ps1 | iex
```

To build from source:

```bash
cargo build --release
# Binary will be at target/release/ax-eval
```

## Quickstart

You need an installed and authenticated agent CLI (`claude-code`, `opencode`, or `codex`).

1. **Validate without a real LLM**. `--dry-run` checks scenario selection,
   fixture setup, cache keys, and run planning without requiring real-run
   consent.
   ```bash
    ax-eval run --scenario example_basic --dry-run
   ```

2. **Enable real-run consent.** `AX_EVAL_ENABLED=1` is required only for
    real agent execution. It prevents accidental LLM API spend and arbitrary
    agent-driven CLI execution in CI or shared environments.
    ```bash
    export AX_EVAL_ENABLED=1
   ```

3. **List scenarios**: View built-in examples.
   ```bash
    ax-eval scenarios
   ```

4. **Print a scenario template**: Start from a schema-valid YAML example.
   ```bash
    ax-eval template scenario > fixtures/my_scenario.yaml
   ```

5. **Run evaluation**:
   ```bash
    ax-eval run --scenario example_basic --tool claude-code
   ```

6. **Or discover a target CLI before writing scenarios**:
   ```bash
    ax-eval discover mytool --tool claude-code
   ```
   Discovery writes `understanding.md`, generated scenarios, scenario run
   artifacts, `discovery-summary.md`, and `discovery.json` under
   `ax-eval-results/<timestamp>-discover-<target>-<tool>-<model>/`.

7. **Review results**:
   ```bash
    cat ax-eval-results/<timestamp>-<tool>-<model>-<scenario>/evaluation.md
   ```

## What You Get

Every run produces a dimensional profile:

1. **Interaction quality**: how many commands, retries, errors, help invocations, and first-try successes occurred.
2. **Qualitative quality**: optional judge and human review signals about whether the agent used the tool well.
3. **Guardrail outcome**: whether deterministic gates caught a catastrophic task failure.
4. **Cost and runtime**: duration, token usage, and cost when adapters report them.

### Artifacts
Each run appends a record to `ax-eval-results/results.jsonl` and generates a run directory containing:

- `evaluation.md`: A human-readable evaluation profile and summary.
- `report.md`: Execution details, guardrail results, and efficiency metrics.
- `metrics.json`: Machine-readable evaluation metrics (interaction evidence, efficiency, gate guardrails, composite score, and evaluator results).
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

Scenarios are YAML files defining the agent task, the environment, and post-run evaluation. The minimal form uses gates as outcome guardrails:

```yaml
name: example_basic
description: Create a summary with the notes CLI
template_folder: example_basic

target:
  binary: notes
  env:
    NOTES_ROOT_DIR: "${AX_EVAL_FIXTURE_DIR}"

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
ax-eval template scenario
ax-eval template config
ax-eval template script-gate
ax-eval template evaluator
```

See the [scenario spec](specs/scenarios.md) for setup hooks, custom evaluators, and the full field reference.

## Documentation

- [User guide](docs/user-guide.md): Commands, scenarios, and CI usage.
- [Scenario spec](specs/scenarios.md): Complete YAML format.
- [Evaluation spec](specs/evaluation.md): Scoring and metrics design.
- [Scripts spec](specs/scripts.md): Custom evaluators and post-scripts.
