# LLM Tool Test

An evaluation framework for measuring how effectively LLM coding agents use CLI tools. It launches real agents against your tool in isolated scenarios, captures full transcripts, and produces dimensional evaluation profiles — not just pass/fail stamps, but scalar measurements you can compare across models, harnesses, and documentation variants.

The transcript is a first-class artifact. When a scenario produces unexpected results, the transcript shows you exactly where the agent went wrong — whether due to unclear docs, missing guidance, or tool behavior the LLM couldn't navigate.

## Evaluation, Not Testing

Traditional testing tools answer _"Did it pass?"_ — binary, terminable. This framework answers _"How well did it go?"_ — scalar, dimensional, comparative.

**Quantitative measures** (from the transcript and metadata):
- Token usage — does a new model increase token usage for the same task?
- Command count, error rate, first-try success rate — how much friction did the agent experience?
- Cost and duration — how expensive was the interaction?

**Qualitative measures** (requiring intrinsic knowledge of how the tool is intended to be used):
- Did the agent follow the documented workflow, or find circuitous workarounds?
- Was the tool used as its author intended, or was it misused?
- These require judgment — from an LLM-as-judge scoring against a rubric, or from a human reviewing the transcript.

Gates (binary pass/fail assertions) exist as a **fail-fast mechanism** — catching catastrophic failures early — but they are not the primary output. The primary output is the evaluation profile.

In CI/CD contexts, binary gates are a valid use of the framework — for example, a token-limit gate that fails if a model or harness change causes usage to exceed a budget. The gate catches the regression; the evaluation profile tells you what changed and how much.

## Who is this for?

**CLI tool authors** who want confidence that their tool works well when an LLM agent is the operator. Does the agent invoke the right subcommands? Does it recover from errors? Do your `--help` strings actually help?

**Skills/guidance authors** (people writing AGENTS.md files, tool documentation, or system prompts) who want to test whether their documentation effectively guides LLMs through real workflows.

## How it works

1. You define **scenarios** — structured tasks with a prompt, evaluation gates, and optional judge rubrics.
2. The framework launches a real LLM agent (opencode, claude-code) in an isolated environment with your tool available.
3. The agent works through the prompt. The full interaction is captured as a **transcript**.
4. Results are evaluated on three layers:
   - **Interaction quality** — quantitative metrics from the transcript (error rate, first-try success, token usage, command count, cost)
   - **Outcome assertions (gates)** — binary fail-fast checks that catch catastrophic failures early
   - **LLM-as-judge** — qualitative rubric-based evaluation of whether the tool was used as intended

Each run produces an `evaluation.md` with the full evaluation profile, and a `metrics.json` with machine-readable scalar measurements for comparative analysis.

## Safety

**Required**: Set `LLM_TOOL_TEST_ENABLED=1` before running tests.

```bash
export LLM_TOOL_TEST_ENABLED=1
```

This prevents accidental expensive LLM API calls.

## Basic Commands

### Run Scenarios

```bash
# Run single scenario
llm-tool-test run --scenario example_basic

# Run all scenarios
llm-tool-test run --all

# Filter by tags or tier
llm-tool-test run --all --tags smoke
llm-tool-test run --all --tier 1

# Dry run (no LLM calls)
llm-tool-test run --scenario example_basic --dry-run
```

### List Scenarios

```bash
# List all
llm-tool-test scenarios

# Filter
llm-tool-test scenarios --tags capture
llm-tool-test scenarios --tier 0
```

### Show Run Details

```bash
llm-tool-test show <run-id>
```

### Clean Artifacts

```bash
# Clean old results (older than 7 days)
llm-tool-test clean --older-than "7d"

# Clean all
llm-tool-test clean
```

## Tool, Model, and Matrix Runs

Run a scenario with a specific agent tool and optional model:

```bash
llm-tool-test run --scenario example_basic --tool opencode
llm-tool-test run --scenario example_basic --tool opencode --model gpt-4o
```

Run a configured matrix with a profile from `llm-tool-test-config.toml`:

```bash
llm-tool-test run --all --profile quick
```

Scenarios can also define a `tool_matrix` to run multiple tool/model combinations without CLI flags.

## Interpreting Results

Each run generates an `evaluation.md` with:

**Summary**: Scenario name, tool, model, outcome (Pass/Fail — fail-fast only)

**Evaluation Profile** (the primary output):
- **Quantitative**: Gates Passed (X/N), Duration, Cost, Token Usage, Command Count, Error Rate, First-try Success Rate
- **Qualitative**: Judge Score (0.0-1.0, when rubric configured), Judge Issues/Highlights
- **Composite Score**: Available when configured per scenario (0.0-1.0)

**Human Review**: Manual scoring section (you fill in)

**Links**: Transcript, metrics, events

**Compare across runs** to answer questions like:
- Did the new model reduce token usage?
- Did the richer AGENTS.md lower the error rate?
- Does claude-code achieve higher first-try success than opencode?

### Gate Types

Gates are binary fail-fast assertions that verify outcomes after the LLM tool completes the task. They catch catastrophic failures early — the real evaluation happens in the scalar metrics and judge layers.

- `command_succeeds`: Shell command exits successfully (exit code 0)
- `command_output_contains`: Command stdout contains expected substring
- `command_output_matches`: Command stdout matches regex pattern
- `command_json_path`: JSON output contains data matching a path assertion (e.g., `$.items[0].status exists`, `$.count > 5`)
- `file_exists`: File present at expected path in fixture directory
- `file_contains`: File content contains expected substring
- `file_matches`: File content matches regex pattern
- `no_transcript_errors`: No command errors detected in transcript
- `script`: Custom script gate that can return pass/fail via exit code or JSON output (`{"passed": true, "message": "..."}`)

## Typical Workflow

```bash
# 1. Enable safety flag
export LLM_TOOL_TEST_ENABLED=1

# 2. List available scenarios
llm-tool-test scenarios

# 3. Run specific scenario
llm-tool-test run --scenario example_basic --tool opencode

# 4. Check results
cat llm-tool-test-results/<timestamp>*/evaluation.md

# 5. Review transcript for debugging
cat llm-tool-test-results/<timestamp>*/transcript.raw.txt
```

## Configuration

Optional `llm-tool-test-config.toml` for tool/model configuration and cost tracking:

```toml
[tools.opencode]
name = "opencode"
command = "opencode"
models = ["gpt-4o", "claude-sonnet"]

[profiles.quick]
name = "quick"
tools = ["opencode"]
models = ["gpt-4o"]

[models.gpt-4o]
input_cost_per_1k_tokens = 2.5
output_cost_per_1k_tokens = 10.0
```

Copy `llm-tool-test-config.example.toml` as a starting point.

## Troubleshooting

**"LLM testing is disabled"**: Set `LLM_TOOL_TEST_ENABLED=1`

**Scenario not found**: Check it's in `fixtures/` directory, use `llm-tool-test scenarios` to list

**Gate failures**: Check metrics.json and transcript.raw.txt for details

**Timeout errors**: Increase timeout with `--timeout-secs 600`

**Cache issues**: Disable caching with `--no-cache` or clean old results

**Composite score low**: Review which gates failed in evaluation.md

**Tool not supported**: Available tools: opencode, claude-code, codex, mock

## Results Location

All test artifacts stored in `llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/`

## Installation

```bash
cargo build --release
# Binary will be at target/release/llm-tool-test
```
