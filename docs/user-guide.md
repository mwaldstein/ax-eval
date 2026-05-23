# User Guide

This guide covers day-to-day usage of `ax-eval`: running scenarios, authoring scenario files, reading results, configuring matrix runs, and troubleshooting.

For the high-level value proposition, start with the [README](../README.md). For the complete CLI flag reference, see [CLI commands](../docs/reference/cli-commands.md). For complete schema details, see the specs in [`specs/`](../specs/).

## Safety Flag

`AX_EVAL_ENABLED=1` is real-run consent. `ax-eval` will not launch
an agent adapter unless this variable is set, because real runs may spend LLM API
credits and execute agent-driven CLI commands.

```bash
export AX_EVAL_ENABLED=1
```

Use `--dry-run` when you want to validate scenario selection, fixture setup,
cache keys, and run planning without setting the safety flag or invoking an LLM:

```bash
ax-eval run --scenario example_basic --dry-run
```

## Runtime Agent Tools

Runtime adapters currently support:

- `claude-code`
- `opencode`
- `codex`

The selected tool must be installed and authenticated before you run a scenario.

The `mock` adapter is internal test support for adapter plumbing. It does not execute the target tool or mutate fixture state, so it is not intended for validating scenario outcomes.

## Typical Workflow

```bash
# 1. Validate selection and setup without a real LLM
ax-eval run --scenario example_basic --dry-run

# 2. Enable real-run consent
export AX_EVAL_ENABLED=1

# 3. List available scenarios
ax-eval scenarios

# 4. Run a scenario with an agent tool
ax-eval run --scenario example_basic --tool claude-code

# 5. Check the evaluation profile
cat ax-eval-results/<timestamp>-<tool>-<model>-<scenario>/evaluation.md

# 6. Review the transcript for debugging
cat ax-eval-results/<timestamp>-<tool>-<model>-<scenario>/artifacts/transcript.raw.txt
```

Filter scenarios by tag or tier before running:

```bash
ax-eval scenarios --tags examples
ax-eval scenarios --tier 0
ax-eval run --all --tags smoke --tool claude-code
ax-eval run --all --tags smoke --tags guidance-test --tier 1 --tool claude-code
```

See the [CLI reference](../docs/reference/cli-commands.md) for all commands and flags.

## Discover a Tool

Use `discover` when you do not yet have scenarios for a target CLI and want an
LLM-first evaluation of how self-describing the executable is:

```bash
export AX_EVAL_ENABLED=1
ax-eval discover qipu --tool opencode
```

Discovery runs one all-in-one workflow:

1. An LLM agent inspects the target executable and writes `understanding.md`.
2. The same discovery agent authors five complex, goal-oriented scenario YAML
   files, templates, and judge rubrics under the discovery result directory.
3. The harness validates the generated scenarios, runs the valid set with
   caching disabled, and keeps going across scenario-level failures.
4. An agent writes `discovery-summary.md`, and the harness writes
   `discovery.json` with stable paths, run IDs, judge scores, failure
   classifications, token totals, and cost when available.

Artifacts are written under a directory like:

```text
ax-eval-results/<timestamp>-discover-<target>-<tool>-<model>/
```

`discover` treats low judge scores, failed target-tool calls, timeouts, and poor
goal completion as findings rather than top-level command failures. The command
fails only when the discovery workflow itself cannot complete, such as when the
target executable is unavailable or zero generated scenarios are valid.

By default, the agent/model passed with `--tool` and `--model` is both the
evaluated scenario-run agent and the discovery authoring agent. Use
`--discover-tool` or `--discover-model` when inspect, fixture authoring, and
summary should use a different agent/model:

```bash
ax-eval discover qipu \
  --tool opencode \
  --model default \
  --discover-tool codex \
  --discover-model gpt-5
```

Judge selection remains separate through `--judge-tool` and `--judge-model`.
Discovery-generated scenarios intentionally use qualitative judge rubrics as a
first-class result; pass/fail is a coarse reference, while the numeric judge
score, rationale, confidence, issues, highlights, and interaction metrics are
the main evaluation signal.

## Scenario Authoring

A scenario describes the task given to the agent, the fixture environment it works in, and the evaluation that runs after the agent exits.

```yaml
name: example_basic
description: Create a summary with the notes CLI
tags: [examples, smoke]
tier: 0
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

Good scenarios evaluate outcomes, not the exact process an agent used. Because LLMs are nondeterministic, prefer checks like "was the summary created?" over "did the agent run these exact commands in this exact order?"

Use gates as guardrails for catastrophic outcome failures. The richer evaluation signal comes from the interaction metrics, transcript, custom evaluator results, and optional judge rubric.

For the complete scenario schema, see the [scenario reference](scenarios.md).

`target.env` values are passed to setup commands, target health checks, agent
adapter runs, post scripts, script gates, and evaluators. Use
`${AX_EVAL_FIXTURE_DIR}` when a target tool needs an environment variable
pointing at the isolated test workspace:

```yaml
target:
  binary: mytool
  env:
    MYTOOL_ROOT_DIR: "${AX_EVAL_FIXTURE_DIR}"
```

### Target Tool Lookup During Development

`ax-eval` does not rewrite `PATH` for the target tool. The agent sees the
environment you give the harness, plus any variables declared in `target.env`.
When the target CLI is built outside the fixture, make that build directory
discoverable before running the scenario:

```bash
cargo build
PATH="$PWD/target/debug:$PATH" \
  AX_EVAL_ENABLED=1 \
  ax-eval run --scenario my_scenario --tool claude-code
```

You can also declare the development path in the scenario when every command in
that scenario should use it:

```yaml
target:
  binary: mytool
  health_check: "mytool --version"
  env:
    PATH: "/absolute/path/to/mytool/target/debug:/usr/local/bin:/usr/bin:/bin"
```

Prefer a shell-level `PATH` change for one-off local development. Prefer
`target.env` when the path is part of the scenario's reproducible setup. Values
in `target.env` are literal except for the documented
`${AX_EVAL_FIXTURE_DIR}` and `${AX_EVAL_RESULTS_DIR}` placeholders,
so `PATH: "...:${PATH}"` will not inherit the caller's path. Use a relative
command such as `./mytool` only when the binary is copied into the fixture
itself.

`interaction.target_commands` defaults to `required`, which is appropriate for
normal CLI workflows. Set it to `optional` for validation scenarios where the
agent may legitimately finish without calling the target tool, or `forbidden`
when calling the target tool should fail the scenario.

## Templates

The `template` command prints schema-valid starting points to stdout. Use it
when creating scenarios or scripts from an agent session:

```bash
ax-eval template scenario > fixtures/my_scenario.yaml
ax-eval template config > ax-eval-config.toml
```

Templates are examples, not generated project scaffolds. After printing one,
rename placeholders such as `mytool`, `example_cli_workflow`, and paths to match
your fixture. See the [CLI reference](../docs/reference/cli-commands.md) for all available template kinds.

## Guidance Topics

Use `guidance` when you want help authoring a CLI and fixture guidance that LLM
agents can use reliably:

```bash
ax-eval guidance list
ax-eval guidance start
ax-eval guidance test-usage
ax-eval guidance workflow-commands typed-errors
```

`guidance start` prints a short capsule index of the highest-priority
topics. Other topics cover LLM-first CLI design, help output, stdout/stderr
contracts, structured output, typed errors, recovery guidance, workflow
commands, state inspection, bounded autonomy, long-running work, agent
authentication, scenario authoring, usage-quality testing, and evaluation
signals. Each topic includes related-topic suggestions so agents can traverse
the guidance without loading every topic at once.

`guidance test-usage` describes the role `ax-eval` should play:
evaluate whether agents can discover and use a tool well from realistic goals,
not command recipes. Gates are guardrails for catastrophic correctness failures;
the evaluation profile is the main signal for usage quality, guidance quality,
and discoverability.

## Gate Types

Gates are binary assertions evaluated after the agent completes the task. They catch catastrophic failures and give you a quick guardrail status alongside the richer profile.

- `command_succeeds`: shell command exits successfully
- `command_output_contains`: command stdout contains an expected substring
- `command_output_matches`: command stdout matches a regex pattern
- `command_json_path`: JSON output contains data matching a path assertion
- `file_exists`: file exists in the fixture directory
- `file_contains`: file content contains an expected substring
- `file_matches`: file content matches a regex pattern
- `no_transcript_errors`: quality guardrail that fails when interaction evidence
  shows target-tool command errors
- `script`: custom script gate using exit code or JSON output

Avoid using `no_transcript_errors` as a unit-test-style assertion that the
agent made no mistakes. Prefer outcome gates such as `file_exists`,
`command_json_path`, or `script` to prove the task result. Treat command errors
as interaction-quality metrics unless the scenario is intentionally checking a
scaffolded, no-exploration workflow.

## Tool, Model, And Matrix Runs

Run a scenario with a specific agent tool and model, or configure a judge tool independently:

```bash
ax-eval run --scenario example_basic --tool claude-code --model claude-sonnet
ax-eval run --scenario example_judge \
  --tool codex \
  --judge-tool claude-code \
  --judge-model claude-sonnet
```

Run a configured matrix with a profile from `ax-eval-config.toml`:

```bash
ax-eval run --all --profile quick
```

Scenarios can also define a `tool_matrix` field to run multiple tool/model combinations without CLI flags — see the [scenario reference](scenarios.md).

See the [CLI reference](../docs/reference/cli-commands.md) for all `run` options.

## Configuration

An optional `ax-eval-config.toml` can define fixture/result paths, tool/model validation, and matrix profiles.

```toml
fixtures_path = "fixtures"
results_path = "ax-eval-results"

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
```

Copy `ax-eval-config.example.toml` as a starting point.

## Interpreting Results

Each run generates `evaluation.md`, `report.md`, and `metrics.json`.

`evaluation.md` is the human-readable profile. It includes:

- Summary: scenario name, tool, model, and run status
- Quality signals: judge score with target threshold and delta, composite score when present, and custom evaluator count
- Interaction metrics: command count, errors, retries, help invocations, first-try success, and evidence source
- Run metrics: duration and cost when adapters report it
- Qualitative scoring: judge score, rationale, issues, highlights, and criteria scores when a rubric is configured
- Custom evaluator summaries and errors
- Guardrails: gate counts and details
- Human review section: space for manual scoring

`report.md` includes execution details, scores, interaction metrics, and guardrail details.
Use `metrics.json` when comparing repeated runs programmatically; it is the
canonical machine-readable profile for interaction quality.
Use `results.jsonl` in the results directory for run-level metadata such as
tool, model, token usage, and cost when adapters report those fields.

The `interaction_evidence_source` field shows how command metrics were built:
`structured_tool_calls` means the adapter provided canonical command events;
`transcript_regex_fallback` means metrics came from transcript regex analysis
for an adapter that cannot expose structured tool calls.

Common comparisons:

- Did the new model reduce token usage?
- Did richer AGENTS.md guidance lower the error rate?
- Did a help-text change improve first-try success?
- Does one adapter complete the same workflow with fewer retries?

## Results Location

Run directories are stored in:

```text
ax-eval-results/<timestamp>-<tool>-<model>-<scenario>/
```

The results directory also contains `results.jsonl`, an append-only run record
database used by `ax-eval show`.

Typical artifacts include:

- `evaluation.md`
- `report.md`
- `metrics.json`
- `artifacts/transcript.raw.txt`
- `artifacts/events.jsonl`
- `artifacts/tool-output.raw.txt`
- `artifacts/command-events.json`

## CI And Regression Gates

CI/CD is an advanced but valid use case. Gates can fail builds when a model, harness, prompt, or documentation change breaks an assumption such as task completion, token budget, or generated artifact shape.

Use CI gates for catastrophic regressions. Use the evaluation profile to understand what changed and by how much.

## Troubleshooting

**"Real LLM tool runs require AX_EVAL_ENABLED=1"**: use `--dry-run` to
validate without an LLM, or set `AX_EVAL_ENABLED=1` to consent to real
agent execution.

**Scenario not found**: check that the scenario exists in `fixtures/`, then run `ax-eval scenarios`.

**Gate failures**: treat these as guardrail failures, then inspect `evaluation.md`, `metrics.json`, and `artifacts/transcript.raw.txt` to understand the full interaction profile.

**Interaction evidence failures**: a structured-capable adapter returned
fallback evidence or no usable target-tool events. Inspect `metrics.json`,
`artifacts/tool-output.raw.txt`, and `artifacts/transcript.raw.txt`; this usually means the
adapter's raw-output parser no longer matches the CLI output schema.

**Timeout errors**: increase timeout with `--timeout-secs 600`.

**Cache issues**: disable caching with `--no-cache` or run `ax-eval clean`.

**Composite score low**: review the interaction metrics, guardrail failures, and judge rationale to understand which dimension regressed.

**Tool not supported**: use one of the runtime adapters: `claude-code`, `opencode`, or `codex`.

## Built-In Example Scenarios

All built-in example scenarios share the `examples` tag:

```bash
ax-eval scenarios --tags examples
```

Run all examples with Claude Code:

```bash
ax-eval run --all --tags examples --tier 1 --tool claude-code
```

The guidance examples compare minimal and rich AGENTS.md instructions:

- `example_guidance_minimal`: bare-bones guidance, requiring more discovery
- `example_guidance_rich`: detailed workflows, examples, and error handling

Compare their `metrics.json` files to see whether richer guidance reduced command count, retries, error rate, token usage, or cost.
