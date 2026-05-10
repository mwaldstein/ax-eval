# LLM User Evaluation

**Status: Draft**

## Purpose

llm-tool-test is an **evaluation framework** for measuring how effectively LLM coding agents use CLI tools. This is not a traditional testing tool — it does not produce a binary pass/fail verdict and stop. It produces **dimensional measurements** that answer questions like "how much did the new model increase token usage?" and "did the richer AGENTS.md reduce the error rate?"

### Evaluation, Not Testing

Traditional testing tools answer: _"Did it pass?"_ — a binary question with a binary answer.

llm-tool-test answers: _"How well did it go?"_ — a scalar question measured across multiple dimensions:

- **Quantitative**: token usage, command count, error rate, first-try success rate, cost, duration, retry rate
- **Qualitative**: did the agent use the tool as intended? Did it follow documented conventions? Was its approach efficient or circuitous?

Gates (binary pass/fail assertions) exist as **supporting guardrails** — they catch catastrophic failures, record whether the basic outcome was achieved, and can optionally avoid wasting an expensive judge call on a run that didn't produce usable output. But gates are not the primary output of the framework. The primary output is the **evaluation profile**: a set of quantitative and qualitative measurements that can be compared across models, harnesses, documentation variants, and tool versions.

That said, binary gates are a valid use of the framework in CI/CD — for example, a token-limit gate that fails if a model or harness change exceeds a budget, or a cost gate that catches regressions. The framework can serve as both a guardrail (binary) and a diagnostic (scalar). The profile tells you what changed and how much; the gate tells you whether that change is acceptable.

### The Two Questions

The framework serves two audiences:

1. **CLI tool authors**: "Can an LLM use my tool effectively given my documentation?" — measures whether the tool's CLI design, help output, error messages, and documentation are sufficient for an LLM agent to accomplish real tasks, and **how much friction** the agent experienced along the way.

2. **Guidance authors**: "Does my AGENTS.md or documentation effectively guide LLMs?" — measures whether project-level instructions, workflow descriptions, and tool usage patterns produce good outcomes when followed by an LLM agent, and **how efficiently** the agent reached those outcomes.

Both audiences share the same core question: if you give an LLM agent your tool's documentation and a task, **how well** does it accomplish the task?

## Core Concept

An LLM-tool interaction reveals information along two axes:

**Quantitative** — directly measurable from the transcript and metadata:
- How many tokens did the agent consume?
- How many tool invocations did it make?
- How many of those failed?
- How long did the run take, and what did it cost?
- How often did the agent get a command right on the first try?

These answer questions like: _"Does a new model or harness increase token usage?"_ and _"Did switching to claude-sonnet reduce the error rate compared to gpt-4o?"_

**Qualitative** — requiring intrinsic knowledge of how the tool is intended to be used:
- Did the agent use the tool as its author intended, or did it find circuitous workarounds?
- Did it follow documented conventions, or ignore them?
- Was its approach efficient relative to the documented workflow, or did it discover the right path through trial and error?
- Did it recover gracefully from errors, or flail?

These require judgment — either from a human reviewing the transcript, or from an LLM-as-judge scoring against a rubric that encodes the tool author's intent.

llm-tool-test automates both dimensions. It launches a real LLM coding agent in an isolated workspace, gives it a task that requires using the target CLI tool, captures the full interaction transcript, and produces a structured evaluation profile — not just a pass/fail stamp.

The framework is tool-agnostic. It does not link against the target tool's code or assume anything about its internals. It treats the target tool as a black box — exactly as an LLM agent would.

---

## Architecture Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Scenarios     │────▶│  llm-tool-test  │────▶│  LLM Agent      │
│   (YAML)        │     │  (harness)      │     │  Adapters        │
└─────────────────┘     └────────┬────────┘     └────────┬────────┘
                                 │                       │
                        ┌────────┴────────┐              │ launches
                        │  Target Tool    │              ▼
                        │  Configuration  │     ┌─────────────────┐
                        └─────────────────┘     │  LLM Agent      │
                                                │  (opencode,     │
                                                │   claude-code,  │
                                                │   codex)        │
                                                └────────┬────────┘
                                                         │ uses
                                                         ▼
                                                ┌─────────────────┐
                                                │  Target CLI     │
                        ┌─────────────────┐     │  Tool           │
                        │  Transcript     │◀────└─────────────────┘
                        │  Capture        │
                        └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │  Evaluator      │
                        │  (3-layer)      │
                        └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │  Results &      │
                        │  Artifacts      │
                        └─────────────────┘
```

### Key Components

1. **Scenarios** — YAML files that define tasks and evaluation criteria. See [specs/scenarios.md](scenarios.md).
2. **Target Tool Configuration** — declares what CLI tool is being tested, including its commands and how to inspect its state.
3. **LLM Agent Adapters** — invoke LLM coding agents (opencode, claude-code, or codex) that then use the target tool.
4. **Transcript Capture** — records the full agent interaction via PTY.
5. **Evaluator** — three-layer evaluation producing a dimensional profile. Gates provide guardrails; interaction metrics and judge provide the main value. See [specs/evaluation.md](evaluation.md).
6. **Results & Artifacts** — structured output for analysis and review.

### Discovery Workflow

`llm-tool-test discover <target>` is a top-level workflow for bootstrapping an
evaluation profile from the target executable itself. Discovery is not a
scenario promotion mechanism and not a pass/fail test. It asks an LLM agent to
inspect the command surface, explain what the tool appears to be for, author a
fixed set of five complex goal-oriented scenarios, run those scenarios, and
summarize the results for the tool author.

Discovery artifacts live under the discovery result directory:

```text
llm-tool-test-results/<timestamp>-discover-<target>-<agent>-<model>/
├── understanding.md
├── scenarios/
├── runs/
├── discovery-summary.md
└── discovery.json
```

Generated scenarios are intentionally ungated and must include enabled
LLM-as-judge evaluation. In discovery, command errors, failed calls, low judge
scores, and run statuses are findings. The main signal is the subjective usage
evaluation: numeric judge scores, rationales, confidence, issues, highlights,
and interaction metrics. The `discover` command exits
non-zero only when the workflow itself cannot complete, such as when the target
is unavailable, the agent cannot run, no generated scenarios are valid, or final
artifacts cannot be written.

Discovery separates the evaluated scenario-run agent from the discovery
authoring agent. `--tool` and `--model` select the agent/model evaluated in the
generated scenarios. `--discover-tool` and `--discover-model` select the
agent/model used for inspect, fixture authoring, and final summary, defaulting
to the base tool/model when omitted. Judge tool/model selection remains
separate.

### Key Architectural Decisions

1. **Separate binary**: `llm-tool-test` is a standalone tool, not a library or test harness linked into the target tool.

2. **Black-box evaluation**: The harness treats the target as an external CLI tool. It doesn't link against the tool's library code.

3. **Adapter indirection**: The harness invokes LLM agents, not the target tool. The LLM agent invokes the target tool. This mirrors real-world usage.

4. **Tool-agnostic**: The harness can evaluate any CLI tool with appropriate scenario definitions and target tool configuration.

---

## LLM Agent Adapters

Adapters handle the specifics of launching and communicating with each LLM coding agent. An important distinction: adapters invoke the **LLM coding agent** (opencode, claude-code, or codex), not the target CLI tool. The agent then uses the target tool autonomously.

The `mock` adapter is internal test support for adapter plumbing. It intentionally has no target-tool behavior, does not mutate fixture state, and should not be used to validate scenario outcomes.

### How Adapters Work

1. The harness prepares an isolated workspace with the scenario's fixture files (AGENTS.md, README, seed data).
2. The adapter launches the LLM agent in that workspace with a task prompt.
3. The agent reads the documentation, uses the target CLI tool to accomplish the task, and exits.
4. The adapter captures the full interaction transcript and extracts cost/token metadata.

### Trait Definition

```rust
pub trait ToolAdapter: Send + Sync {
    /// Check if tool is installed and authenticated.
    fn is_available(&self) -> Result<ToolStatus, AdapterError>;

    /// Check if the tool is available and ready to use.
    fn check_availability(&self) -> anyhow::Result<()>;

    /// Run the tool with the given scenario in the specified working directory.
    /// Returns canonical run artifacts.
    fn run(
        &self,
        scenario: &Scenario,
        cwd: &Path,
        model: Option<&str>,
        timeout_secs: u64,
    ) -> anyhow::Result<ToolRunOutput>;
}

pub struct ToolStatus {
    pub available: bool,
    pub authenticated: bool,
}

pub struct TokenUsage {
    pub input: usize,
    pub output: usize,
}
```

### Adapter Responsibilities

- **Execution**: Launch the agent process with appropriate flags and prompt.
- **Raw output capture**: Capture full output from the agent session.
- **Tool event normalization**: Keep adapter-specific raw-output parsing next to the adapter in `src/adapter/<name>/normalize.rs`. When an adapter receives structured JSON output, synthesize canonical transcript text and command events so interaction metrics can be derived consistently.
- **Canonical artifact construction**: Use shared helpers in `src/adapter/normalize.rs` for common `ToolRunOutput` construction. Keep tool-specific event schemas out of the shared module.
- **Cost/token tracking**: Parse actual cost and token usage from agent output when available. Do not estimate from character counts.
- **Timeout enforcement**: Kill the agent process if it exceeds the configured timeout.

### Adapter Module Layout

Runtime adapters use a directory module:

```text
src/adapter/<name>/
├── mod.rs        # Tool invocation and ToolAdapter implementation
└── normalize.rs  # Raw output -> ToolRunOutput
```

This keeps tool invocation and raw event schema knowledge local to the adapter,
while shared normalization helpers remain in `src/adapter/normalize.rs`. See
`src/adapter/README.md` for the maintainer checklist.

### Available Adapters

| Adapter | Agent Invocation | Status |
|---------|-----------------|--------|
| opencode | `opencode run --format json <prompt>` | Primary |
| claude-code | `claude -p --output-format stream-json --verbose --include-partial-messages <prompt>` | Primary |
| codex | `codex exec --json --full-auto --skip-git-repo-check <prompt>` | Primary |
| mock | internal mock adapter | Adapter tests only; no target-tool behavior |

---

## Transcript Capture

Transcripts are the primary artifact of a run. For CLI tool authors, they reveal how the LLM interprets documentation and error messages. For guidance authors, they show exactly how the LLM follows (or deviates from) instructions.

### PTY-Based Capture

The harness uses pseudo-terminal capture to get the complete interaction including:

- ANSI colors and formatting
- Interactive prompts and responses
- Real-time streaming output
- Tool invocations and results

Fallback to piped stdout/stderr if PTY is unavailable.

### Event Log Format

Structured events logged during the run:

```jsonl
{"ts": 1705500000.123, "event": "spawn", "command": "opencode", "args": ["--prompt", "..."]}
{"ts": 1705500001.456, "event": "output", "text": "I'll work on this task...\n"}
{"ts": 1705500005.789, "event": "tool_call", "tool": "bash", "command": "my-tool create \"Main Concept\""}
{"ts": 1705500006.012, "event": "tool_result", "output": "Created item abc123\n", "exit_code": 0}
{"ts": 1705500030.000, "event": "complete", "exit_code": 0, "duration_secs": 30.0}
```

### Artifact Structure

Each run produces:

```
llm-tool-test-results/<timestamp>-<tool>-<model>-<scenario>/
├── metrics.json            # Evaluation metrics
├── evaluation.md           # Human-readable summary
├── report.md               # Execution and efficiency report
├── artifacts/
│   ├── transcript.raw.txt  # Complete or synthesized transcript
│   ├── events.jsonl        # Structured event log
│   ├── tool-output.raw.txt # Raw adapter output when available
│   └── command-events.json # Normalized command events when available
└── fixture/                # Working directory, preserved after run
```

### Run Metadata

```json
{
  "scenario_id": "example_basic",
  "scenario_hash": "abc123def456",
  "tool": "opencode",
  "model": "claude-sonnet-4-20250514",
  "timestamp": "2025-01-17T12:00:00Z",
  "duration_secs": 45.3,
  "cost_usd": 0.023,
  "token_usage": {
    "input": 1500,
    "output": 800
  }
}
```

---

## Execution Flow

A single scenario run proceeds through these steps:

### 1. Load Scenario

Parse the YAML scenario file and its inline target tool configuration. Validate referenced fixtures, rubrics, and commands as they are used.

### 2. Prepare Isolated Workspace

Copy the scenario's fixture template into a `fixture/` directory inside the results directory. This includes AGENTS.md, README, seed data, and any pre-initialized tool state. The harness creates a fresh copy for each run. Because the fixture lives inside the results directory, any files the agent creates or modifies are automatically preserved as post-run artifacts.

### 3. Run Setup Commands

Execute any `setup` commands defined in the scenario (e.g., initializing the target tool, importing seed data). These run before the LLM agent is launched.

### 4. Launch LLM Agent via Adapter

The adapter launches the LLM coding agent (opencode, claude-code, or codex) in the prepared workspace with the scenario's task prompt. The agent autonomously reads documentation and uses the target CLI tool to accomplish the task.

### 5. Capture Transcript

The adapter captures agent output during the session. After the agent exits (or is killed by timeout), the raw or synthesized transcript is written to disk and run events are logged.

### 6. Post-Execution Scripts

Run any post-execution scripts defined in `scripts.post`. These handle tool-specific state capture, data export, or normalization — anything that needs to happen after the agent finishes but before evaluation. Failures are logged as warnings but do not block evaluation.

See [specs/scripts.md](scripts.md) for details.

### 7. Evaluate

The evaluator produces a dimensional evaluation profile:
- **Interaction quality metrics**: quantitative measures — command error rate, first-try success rate, retry rate, help-seeking, command count, token usage, cost, duration (always runs)
- **Gates**: binary guardrail assertions — catch catastrophic failures and record whether the basic outcome was achieved. All gates run regardless of earlier failures so the full picture is available.
- **Custom evaluators**: scripts that produce additional scalar metrics/scores (see [specs/scripts.md](scripts.md))
- **LLM-as-judge**: qualitative assessment — rubric-based scoring that encodes the tool author's intent about how the tool should be used (optional, if enabled and gates pass)

**The evaluation profile is the primary output.** Gate status is a filter and safety check; the quantitative and qualitative measurements across all layers are what enable comparisons across models, harnesses, and documentation variants.

See [specs/evaluation.md](evaluation.md) for details.

### 8. Generate Artifacts

Write the transcript, event log, metrics, and evaluation summary to the results directory. The fixture (working directory) is already in the results directory and is preserved as-is.

---

## Cost Management

LLM API calls are expensive, so the harness records available cost and token usage for trend analysis.

### Cost Tracking

The current implementation records actual cost when an adapter exposes it, and writes that cost to results for trend analysis. Budget enforcement is future work; there is no `--max-usd` flag today.

### Caching

Cache key components:
- Scenario YAML hash
- Prompt content hash
- Fixture/template content hash
- Agent tool + model identifier

If cache hit, reuse transcript and evaluation results. Disable with `--no-cache`.

### Dry Run Mode

`--dry-run` shows:
- Scenarios that would run
- Selected agent tool and model
- Generated run metadata placeholder

No LLM API calls are made, no agent adapter is launched, and
`LLM_TOOL_TEST_ENABLED=1` is not required.

### Environment Variables

```bash
LLM_TOOL_TEST_ENABLED=1          # Required only for real agent execution; not needed for --dry-run.
```

---

## Security

### Transcript Redaction

Before writing the human-readable evaluation summary, redact:
- API keys and tokens
- Passwords and secrets
- Email addresses (optional)
- File paths containing usernames

The raw transcript is preserved but marked as sensitive. It should not be committed to version control.

### Gitignore

```gitignore
# LLM test artifacts (volatile, potentially sensitive)
llm-tool-test-results/
```

---

## CLI Interface

### Commands

```bash
# Run scenarios
llm-tool-test run --scenario example_basic  # Run specific scenario
llm-tool-test run --all                     # Run all scenarios
llm-tool-test run --all --tags capture      # Run by tag
llm-tool-test run --all --tags smoke --tags capture  # Match any listed tag
llm-tool-test run --all --tier 1            # Run by tier
llm-tool-test run --scenario example_basic --tool opencode  # Run with specific agent
llm-tool-test run --dry-run                 # Validate selection without LLM calls

# Matrix runs
llm-tool-test run --all --profile quick

# Discover a target tool and run generated scenarios
llm-tool-test discover qipu --tool opencode
llm-tool-test discover qipu --tool opencode --discover-tool codex

# Print copyable schema templates
llm-tool-test template scenario
llm-tool-test template config
llm-tool-test template script-gate
llm-tool-test template evaluator

# List and inspect scenarios
llm-tool-test scenarios                     # List all scenarios
llm-tool-test scenarios --tags capture      # Filter by tags
llm-tool-test show <run-id>                 # Show saved run details

# Maintenance
llm-tool-test clean --older-than 7d         # Clean cache and legacy transcript artifacts
llm-tool-test clean                         # Clean cache and legacy transcript artifacts
```

---

## Results Tracking

### Purpose

Results are not primarily for stamping Pass/Fail. They are for **comparative evaluation** — answering questions like:

- Did the new model reduce token usage?
- Did the richer AGENTS.md lower the error rate?
- Does claude-code achieve higher first-try success than opencode on this tool?
- Has a tool version change degraded the interaction profile?

A single run produces a profile. Two runs produce a comparison. Many runs produce a trend.

### Storage

Results are stored in an append-only format for trend analysis:

```
llm-tool-test-results/
├── results.jsonl           # Append-only run results
└── results.db              # Optional SQLite for queries
```

### Regression Detection

Compare against baseline runs:
- Score degradation > 15% triggers warning
- Gate failures that previously passed trigger alert
- Cost increases > 50% trigger warning

---

## Not In Scope

- CI integration (too expensive for automated runs)
- Multi-model statistical benchmarking (future)
- Real-time cost tracking via provider APIs
- Interactive test authoring UI

---

## Cross-References

- Scenario format and target tool configuration: see [specs/scenarios.md](scenarios.md)
- Quality measurement and evaluation: see [specs/evaluation.md](evaluation.md)
- Scripts and extension hooks: see [specs/scripts.md](scripts.md)
- Distribution and installation: see [specs/distribution.md](distribution.md)
