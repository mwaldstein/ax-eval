# Agent Experience Eval (`ax-eval`)

**Build great tools for agents.**

Even capable agents benefit from tools designed with their needs in mind. Without intentional design, agents may encounter execution loops, hallucinated command structures, excessive token usage, or complete failure to achieve goals. 

Replace ad-hoc developer exploration with structured, measured, and judged evaluations. `ax-eval` runs coding agents against your tool in reproducible scenarios to generate quantitative metrics and qualitative scores. Stop guessing if your CLI help, error messages, MCP tool descriptions, or agent guidance are actually effective. Run apples-to-apples comparisons before and after changes to guarantee your updates reduce friction, eliminate loops, and save tokens.

Your tool can be a **CLI binary** or an **MCP server** (stdio or Streamable HTTP, including authenticated servers). Both are evaluated through the same profile, so you can even compare the two ways of exposing the same capability.

### See it in action

Run a reproducible scenario against your CLI or MCP server:

```bash
ax-eval run --scenario create_project --tool claude-code
```

Get an immediate, dimensional evaluation profile of the agent's execution:

```json
{
  "efficiency": {
    "total_commands": 6,
    "error_count": 0,
    "tool_reuse_count": 1,
    "first_try_success_rate": 1.0
  },
  "costs": {
    "total_tokens_used": 14250,
    "total_cost_usd": 0.04
  },
  "judge": {
    "score": 4,
    "rationale": "The agent correctly identified the right command but initially struggled with the syntax for the input file, eventually recovering after one retry."
  },
  "gate_status": "passed"
}
```

*(See `ax-eval-results/` for full transcripts, cost/token usage, and markdown summaries).*

---

## Supported Agents and Targets
`ax-eval` currently drives the `claude-code`, `opencode`, and `codex` agent CLIs on macOS, Linux, and Windows.

It evaluates two kinds of target:

- **CLI tools** — identified by a binary name; evidence comes from the agent's shell commands.
- **MCP servers** — `stdio` or Streamable HTTP (`http`); evidence comes from structured `tools/call` events. The harness provisions the server into the agent host's native MCP config; authenticated servers are supported via a static token from the environment or a pre-established host session. ax-eval does not run OAuth itself — the agent's host is the OAuth client. See [MCP targets](docs/mcp-targets.md) and [MCP authentication](docs/mcp-auth.md).

Built primarily for CLI and MCP-server authors, technical writers improving tool-facing documentation, and agent developers comparing models on a specific workflow.

## How It Works

1. **Execute** a baseline scenario — `ax-eval` runs the configured agent CLI against your prompt and records every command, error, and token.
2. **Analyze** the metrics, transcript, and Judge score to see exactly where the agent stalled or wasted effort.
3. **Modify** the friction points — a CLI's error messages, parameter handling, and `--help` text; an MCP server's tool names, descriptions, and input schemas; or fixture guidance when guidance quality is the variable under test.
4. **Repeat** to run an apples-to-apples comparison and verify your changes actually reduced friction.

If you do not have scenarios yet and your target is a CLI, start with `discover`:
it asks an agent to inspect your executable, write an understanding document,
author five goal-oriented scenarios, run them, and summarize what the results
reveal about your CLI's LLM usability. (Discovery is CLI-only today; author MCP
scenarios from `ax-eval template scenario`.)

## Agent-Driven Evaluation

ax-eval is self-documenting for agents. Point your AI coding tool at it instead of learning the CLI by hand, and let the agent run the evaluate-modify-repeat loop. Paste it a prompt like:

> Evaluate how well AI coding agents can use my CLI tool `mytool`. Learn `ax-eval` from itself: run `ax-eval guidance start`, then read `ax-eval --help`, `ax-eval discover --help`, and `ax-eval template --help`. Run `AX_EVAL_ENABLED=1 ax-eval discover mytool --tool opencode` to auto-generate and judge goal-oriented scenarios, or author scenarios from `ax-eval template scenario`. Read the profile under `ax-eval-results/` (`evaluation.md`, `metrics.json`) and propose changes to my CLI's help text, error messages, command structure, and structured output. Treat AGENTS.md changes as an explicit guidance experiment, not the default fix. Re-run to confirm the improvement.

See the [user guide](docs/user-guide.md#agent-driven-workflow) for self-discovery entry points and prompt variants.

## Install

Install via crates.io:

```bash
cargo install ax-eval
```

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
   consent. `validate` checks scenario YAML for schema errors before setup.
   ```bash
     ax-eval run --scenario example_basic --dry-run
    ```

   ```bash
     ax-eval validate --scenario ax-eval-fixtures/example_basic.yaml
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
    ax-eval template scenario > ax-eval-fixtures/my_scenario.yaml
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

1. **Interaction quality**: how many commands, tool reuses, errors, help invocations, and first-try successes occurred.
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
- `artifacts/command-events.json`: Normalized CLI command events when available.
- `artifacts/mcp-events.json`: Structured MCP tool-call events for MCP targets when available.

### Metrics Example
```json
{
  "gate_status": "passed",
  "details": [
    {
      "gate_type": "file_exists",
      "identifier": "file_exists(summary.md)",
      "passed": true,
      "message": "File exists: summary.md"
    }
  ],
  "efficiency": {
    "total_commands": 6,
    "unique_commands": 5,
    "error_count": 0,
    "tool_reuse_count": 1,
    "help_invocations": 1,
    "first_try_success_rate": 1.0,
    "iteration_ratio": 0.83,
    "completed": true
  },
  "interaction_evidence_source": "structured_tool_calls"
}
```

Judge, composite, and custom evaluator fields are included only when configured — see the [evaluation reference](docs/evaluation.md).

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

An MCP target swaps the `target` block for a server declaration; the rest of the scenario (task, gates, judge) is identical:

```yaml
target:
  kind: mcp
  name: notes
  transport:
    type: stdio
    command: "python3"
    args: ["${AX_EVAL_FIXTURE_DIR}/notes_mcp_server.py"]
  tools: [add_note, list_notes]
  # Authenticated HTTP server:
  # transport: { type: http, url: "https://mcp.example.com/mcp" }
  # auth: { type: bearer_env, env: MY_MCP_TOKEN }
```

See the [scenario reference](docs/scenarios.md) for setup hooks, custom evaluators, and the full field reference, and [MCP targets](docs/mcp-targets.md) for the transport and provisioning details. Use `ax-eval template scenario` for a copyable starting point.

## Documentation

- [Tutorial](docs/tutorial.md): Evaluate your CLI in 10 minutes.
- [CLI reference](docs/reference/cli-commands.md): All commands, flags, and examples.
- [User guide](docs/user-guide.md): Workflows, scenario authoring, results, and troubleshooting.
- [Scenario reference](docs/scenarios.md): Complete YAML format.
- [MCP targets](docs/mcp-targets.md): Evaluating MCP servers — transports, provisioning, evidence.
- [MCP authentication](docs/mcp-auth.md): Credentials for protected MCP servers.
- [Evaluation reference](docs/evaluation.md): Scoring and metrics design.
- [Scripts reference](docs/scripts.md): Custom evaluators and post-scripts.
