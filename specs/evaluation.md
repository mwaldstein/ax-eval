# Evaluation

**Status: Draft**

## Purpose

Define how llm-tool-test evaluates the quality of LLM-tool interactions. The evaluation system must work for any CLI tool without domain-specific assumptions baked into the framework.

There are few universally applicable, objective measures of "did the LLM use this tool well." Rather than invent domain-specific metrics, the framework measures quality in three layers, ordered from cheapest to most expensive. Each layer answers a different question:

1. **Interaction quality** — Did the LLM use the tool efficiently? (transcript-derived, always available)
2. **Outcome assertions** — Did the task produce the right results? (scenario-author-defined gates)
3. **LLM-as-judge** — Was the output subjectively good? (optional, rubric-driven)

---

## Layer 1: Interaction Quality

These metrics are derived from the transcript and are always available, regardless of the tool being tested. They measure *how* the LLM interacted with the tool, not *what* it produced.

### Command Identification

The transcript analyzer must know which commands belong to the target tool. This requires a configurable command pattern, specified per scenario or globally:

```yaml
# In scenario YAML
target:
  command_pattern: "my-tool\\s+(\\S+)"

# Or globally in config
[target]
command_pattern = "my-tool\\s+(\\S+)"
```

The pattern identifies target tool invocations in the transcript. If the pattern includes a capture group, the captured text is used to extract the subcommand for per-subcommand analytics (e.g., distinguishing `my-tool create` from `my-tool list`). If no capture group is present, only aggregate counts (total commands, error rate, etc.) are available.

### Metrics

| Metric | Definition | Signal |
|--------|-----------|--------|
| **Error rate** | Proportion of target-tool commands that failed | Tool usability; unclear error messages |
| **Retry rate** | Commands repeated after failure (total - unique) / total | Error message quality; recovery difficulty |
| **Help-seeking** | Count of `--help` invocations | Documentation clarity |
| **First-try success rate** | Commands that succeeded on first attempt / total commands | Combined doc + UX quality |
| **Iteration ratio** | unique commands / total commands | Efficiency; high = less repetition |
| **Completion** | Did the agent complete the task vs give up or time out | Basic pass/fail signal |
| **Command count** | Total target-tool commands executed | Efficiency (fewer is better, given completion) |

### Data Sources

Interaction metrics are derived from one of two data sources:

- **Structured events (preferred)**: When `events.jsonl` is available, metrics are derived from `tool_call`/`tool_result` events which include explicit exit codes. This is the preferred source of truth.
- **Transcript fallback**: When structured events are not available, the framework falls back to regex-based transcript analysis using `target.command_pattern`. In this fallback mode, exit code detection is approximate and error rate metrics may be less reliable.

### Completion

The `completed` metric is determined as follows:

- **Completed**: The agent process exited normally (not killed by timeout) with exit code 0.
- **Not completed**: The agent was killed by timeout, crashed, or exited with a non-zero exit code.

### Interpreting Interaction Metrics

These metrics are most valuable to guidance/skills authors who want to know whether their documentation is working:

- **Low error rate + low help-seeking**: The docs are clear and the tool's CLI is intuitive.
- **High retry rate**: The tool's error messages aren't helping the LLM recover. The LLM is repeating the same command or trying slight variations.
- **High help-seeking**: The AGENTS.md or tool documentation doesn't provide enough information up front. The LLM is falling back to `--help` to figure out syntax.
- **Low first-try success rate**: Combined signal that something is off — either the docs are misleading or the CLI surface is confusing.
- **High command count with completion**: The LLM got there, but took a circuitous path. May indicate missing examples or unclear workflows.

### Rust Representation

The existing `EfficiencyMetrics` struct maps directly to this layer. Add `completion` and rename the struct:

```rust
pub struct InteractionMetrics {
    pub total_commands: usize,
    pub unique_commands: usize,
    pub error_count: usize,
    pub retry_count: usize,
    pub help_invocations: usize,
    pub first_try_success_rate: f64,
    pub iteration_ratio: f64,
    pub completed: bool,  // new: did the agent finish the task
}
```

---

## Layer 2: Outcome Assertions (Gates)

Gates are deterministic, scenario-author-defined checks. They answer: "did the task produce correct results?" Gates are domain-specific by nature, but expressed through generic primitives.

### Gate Types

The current implementation includes domain-specific gates (`MinNotes`, `MinLinks`, `SearchHit`, `NoteExists`, `LinkExists`, `TagExists`, `ContentContains`, `DoctorPasses`). These must be replaced with generic primitives that any CLI tool author can use.

#### Generic Gates

| Gate | Parameters | Behavior |
|------|-----------|----------|
| `command_succeeds` | `command: String` | Run shell command in work directory. Assert exit code 0. |
| `command_output_contains` | `command: String`, `substring: String` | Run command. Assert stdout contains substring. |
| `command_output_matches` | `command: String`, `pattern: String` | Run command. Assert stdout matches regex pattern. |
| `command_json_path` | `command: String`, `path: String`, `assertion: String` | Run command. Parse stdout as JSON. Apply assertion to value at JSONPath. |
| `file_exists` | `path: String` | Assert file exists relative to work directory. |
| `file_contains` | `path: String`, `substring: String` | Read file. Assert content contains substring. |
| `file_matches` | `path: String`, `pattern: String` | Read file. Assert content matches regex pattern. |
| `no_transcript_errors` | *(none)* | Assert no target-tool commands had non-zero exit codes. (Existing.) |
| `script` | `command: String`, `description: String` | Run script. Pass if exit code 0. Optionally returns structured JSON. See [specs/scripts.md](scripts.md). |

#### `command_json_path` Assertions

The `assertion` field supports these forms:

- `exists` — the path resolves to a value (not null/missing)
- `equals <value>` — exact equality (strings, numbers, booleans)
- `contains <substring>` — string value contains substring
- `len >= N`, `len == N`, `len > N` — array/object length comparisons

#### Scenario Example

For example, a scenario author testing a task manager might express "at least 3 tasks were created" as:

```yaml
evaluation:
  gates:
    - type: command_json_path
      command: "my-tool list --format json"
      path: "$"
      assertion: "len >= 3"

    - type: command_output_contains
      command: "my-tool search 'distributed systems'"
      substring: "distributed"

    - type: file_exists
      path: ".my-tool/store.db"

    - type: command_succeeds
      command: "my-tool doctor"

    - type: no_transcript_errors
```

This replaces the current domain-specific gates with equivalent generic ones. The scenario author brings domain knowledge; the framework provides the assertion primitives.

#### Migration from Current Gates

| Current Gate | Equivalent Generic Gate |
|-------------|------------------------|
| `MinNotes { count: 3 }` | `command_json_path { command: "my-tool list --format json", path: "$", assertion: "len >= 3" }` |
| `MinLinks { count: 1 }` | `command_json_path { command: "my-tool export --format json", path: "$.links", assertion: "len >= 1" }` |
| `SearchHit { query }` | `command_output_contains { command: "my-tool search '{query}'", substring: ... }` |
| `NoteExists { id }` | `command_succeeds { command: "my-tool show {id}" }` |
| `TagExists { tag }` | `command_output_contains { command: "my-tool list --format json", substring: "{tag}" }` |
| `ContentContains { id, substring }` | `command_output_contains { command: "my-tool show {id}", substring: "{substring}" }` |
| `DoctorPasses` | `command_succeeds { command: "my-tool doctor" }` |

#### Rust Representation

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Gate {
    CommandSucceeds { command: String },
    CommandOutputContains { command: String, substring: String },
    CommandOutputMatches { command: String, pattern: String },
    CommandJsonPath { command: String, path: String, assertion: String },
    FileExists { path: String },
    FileContains { path: String, substring: String },
    FileMatches { path: String, pattern: String },
    NoTranscriptErrors,
    Script { command: String, description: String },
}
```

### Gate Design Philosophy

Gates verify **outcomes**, not **process**. A scenario should ask "did the task get done?" not "did the LLM follow my expected steps without deviation?"

#### Outcome vs Process

| Good Gate (outcome) | Bad Gate (process) |
|---------------------|-------------------|
| `file_exists: report.pdf` | Command used exact flag order |
| `command_json_path: len >= 4` | No errors in transcript |
| `file_contains: "migration"` | Search returned specific substring |

LLMs are non-deterministic. Two runs with identical prompts may use different note titles, file names, or command sequences. Gates that require exact outputs or zero errors will flake and do not measure effectiveness.

#### When to Use `no_transcript_errors`

The `no_transcript_errors` gate asserts that every target-tool command in the transcript succeeded. This is appropriate for:
- **Scaffolded scenarios** where the correct workflow is obvious (e.g., "run `./hello run`")
- **Regression tests** verifying a tool the LLM already knows

It is **inappropriate** for:
- **Discovery/guidance scenarios** where the LLM must learn the tool via trial and error
- **Complex multi-step tasks** where exploration is expected

In discovery scenarios, errors are diagnostic data (Layer 1 interaction metrics), not failures.

#### Minimal Gates for Guidance Testing

When comparing AGENTS.md versions, the primary signal is Layer 1 interaction metrics (error rate, retry rate, help-seeking, first-try success). Gates should be a minimal sanity check that the task was completed:

```yaml
evaluation:
  gates:
    - type: file_exists
      path: notes.db
    - type: command_json_path
      command: "./notes export --format json"
      path: "$.notes"
      assertion: "len >= 4"
    - type: file_exists
      path: links.txt
```

The comparison between guidance variants happens in `metrics.json` — lower error rates and higher first-try success rates tell you the richer documentation is working.

---

### Gate Evaluation

All gates are evaluated after the LLM agent finishes (or times out). Gate results are binary pass/fail with a message:

```rust
pub struct GateResult {
    pub gate_type: String,
    pub passed: bool,
    pub message: String,
}
```

Gates are evaluated in declaration order. All gates run regardless of earlier failures (no short-circuit) so the full picture is always available.

---

## Layer 3: LLM-as-Judge

Rubric-based qualitative assessment using a separate LLM call. This is the most expensive layer and is always optional.

### When to Use

Use the judge for subjective quality that gates can't capture:
- Was the output well-organized?
- Did the LLM make reasonable decisions about how to structure data?
- Did the LLM follow the conventions described in the documentation?
- Did the LLM seem confused by the tool's interface?

### Rubric Format

Rubrics define weighted criteria in YAML. The existing format is adequate:

```yaml
criteria:
  - id: command_correctness
    weight: 0.30
    description: "Uses valid CLI commands with correct syntax"

  - id: task_completion
    weight: 0.40
    description: "Completes all aspects of the assigned task"

  - id: efficiency
    weight: 0.30
    description: "Accomplishes the task without unnecessary commands or dead ends"

output:
  format: json
  require_fields: [scores, weighted_score, confidence, issues, highlights]
```

Rubric criteria are entirely scenario-specific. The framework imposes no default criteria — the scenario author defines what matters.

### Judge Model

The judge model should be:
- **Cheap and fast** — the judge call should cost a small fraction of the test run itself.
- **Not the same model being tested** — to avoid self-evaluation bias.
- Configurable via `LLM_TOOL_TEST_JUDGE` env var or scenario config.

Recommended defaults: `gpt-4o-mini`, `claude-haiku`.

### Structured Output

The judge must return JSON matching the `JudgeResponse` schema:

```json
{
  "scores": { "command_correctness": 0.85, "task_completion": 0.90, "efficiency": 0.70 },
  "weighted_score": 0.83,
  "confidence": 0.80,
  "issues": ["Retried 'create' command 3 times with same args"],
  "highlights": ["Good use of search to verify data was captured"]
}
```

### Pass Threshold

The scenario configures a `pass_threshold` (0.0–1.0). The judge layer passes if `weighted_score >= pass_threshold`.

### Scenario Configuration

```yaml
evaluation:
  judge:
    enabled: true
    rubric: rubrics/capture_v1.yaml
    pass_threshold: 0.70
```

---

## Composite Scoring

### Current Problem

The current implementation computes a single composite score with hardcoded weights:

```
judge: 50%, gates: 30%, efficiency: 10%, quality: 10%
```

The `quality` component (`QualityMetrics`) is entirely domain-specific — it measures title length, tags per note, orphan notes, links per note. This does not generalize.

### Recommendation: Report Layers Independently

A single composite number is misleading when the components measure fundamentally different things. The default behavior should be:

1. **Report each layer independently.** The evaluation output shows interaction metrics, gate results, and judge score as separate sections.
2. **Each layer has its own pass/fail.** Gates pass if all gates pass. Judge passes if `weighted_score >= pass_threshold`. Interaction metrics are informational (no automatic pass/fail).
3. **Overall outcome** is determined by gates and (if enabled) judge. Interaction metrics inform but don't gate.

### Optional Composite Scoring

If a scenario author wants a single number, they can define weights explicitly:

```yaml
evaluation:
  composite:
    gate_weight: 0.50
    judge_weight: 0.30
    interaction_weight: 0.20
```

When `composite` is present, a composite score is computed. When absent, no composite score is reported.

### Outcome Determination

```
Outcome = Pass    if all gates pass AND (judge disabled OR judge passes)
Outcome = Fail    if any gate fails OR (judge enabled AND judge fails)
```

Interaction metrics do not affect the outcome. They are diagnostic.

### Rust Representation

```rust
pub struct EvaluationResult {
    // Layer 1
    pub interaction: InteractionMetrics,

    // Layer 2
    pub gates: Vec<GateResult>,
    pub gates_passed: usize,
    pub gates_total: usize,

    // Layer 3 (optional)
    pub judge_score: Option<f64>,
    pub judge_response: Option<JudgeResponse>,

    // Composite (optional)
    pub composite_score: Option<f64>,

    // Final
    pub outcome: Outcome,
}

pub enum Outcome {
    Pass,
    Fail { reason: String },
}
```

Drop `ReviewRequired` — a run either passes or fails. Human review is a workflow concern, not an evaluation outcome.

---

## Evaluation for Guidance Authors

The secondary audience for llm-tool-test is guidance/skills authors who are testing whether their AGENTS.md or skill definitions help LLMs use a tool effectively.

### Primary Signal: Interaction Metrics

For guidance authors, Layer 1 metrics are the most important signal. Gates tell you whether the task was completed; interaction metrics tell you *why it was or wasn't* and *how much friction the LLM experienced*.

**Do not** use `no_transcript_errors` as a gate in guidance scenarios. Errors during discovery are expected and informative. The gate should only verify the final outcome (e.g., the expected files exist, the exported data has the right shape).

### Comparing Guidance Versions

The key workflow: run the same scenario with different AGENTS.md files and compare interaction metrics.

```
Scenario: create_and_link
AGENTS.md v1: error_rate=0.35, help_seeking=4, first_try_success=0.55
AGENTS.md v2: error_rate=0.10, help_seeking=1, first_try_success=0.82
```

This tells the author that v2 of their documentation is substantially better at helping the LLM use the tool. The gate pass rate should be similar or identical across variants — the difference is in *how* the LLM got there.

### Diagnostic Patterns

| Pattern | Likely Cause |
|---------|-------------|
| High error rate, low help-seeking | LLM thinks it knows the syntax but doesn't. Docs may have incorrect examples. |
| High error rate, high help-seeking | `--help` output isn't sufficient. Missing examples or unclear argument descriptions. |
| Low error rate, high help-seeking | Docs don't include enough up front, but `--help` is good. Add more examples to AGENTS.md. |
| High retry rate on specific commands | That command's error messages don't help the LLM correct its approach. |
| High command count, task completed | Docs describe the commands but not the workflow. Add a "common workflows" section. |

### Judge Prompting for Guidance Evaluation

When evaluating documentation quality specifically, the judge rubric can include criteria like:

```yaml
criteria:
  - id: documentation_sufficiency
    weight: 0.50
    description: "Did the LLM appear to have sufficient information from the provided documentation to complete the task without confusion?"

  - id: workflow_clarity
    weight: 0.50
    description: "Did the LLM follow a logical workflow, or did it seem to discover the correct approach through trial and error?"
```

---

## Not In Scope

- **Domain-specific quality metrics** (title length, tags per note, links per note, orphan detection) — these belong in scenario-specific gates and rubrics, not in the framework. The existing `QualityMetrics` / `StoreAnalyzer` module should be removed or moved to an example scenario.
- **Real-time cost tracking via provider APIs** — cost estimation from token counts and configured rates is sufficient.
- **Statistical significance testing across runs** — future work. Currently, each run is evaluated independently.
- **Automatic regression detection** — future work. The results database supports trend analysis, but automated alerting is not part of this spec.
