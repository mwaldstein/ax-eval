# Evaluation

**Status: Stable**

## Purpose

Define how ax-eval evaluates the quality of LLM-tool interactions. The evaluation system must work for any CLI tool without domain-specific assumptions baked into the framework.

### Evaluation vs. Testing

This is an **evaluation framework**, not a testing tool. The distinction matters:

- **Testing** asks _"Did it pass?"_ — binary, terminable, final.
- **Evaluation** asks _"How well did it go?"_ — scalar, dimensional, comparative.

The primary output is not a Pass/Fail stamp. It is an **evaluation profile**: a set of quantitative and qualitative measurements positioned along continuous scales. These measurements enable comparisons:

- Does model B use fewer tokens than model A for the same task?
- Did the updated AGENTS.md reduce the error rate?
- Does claude-code achieve higher first-try success than opencode?
- Has a tool version change degraded the interaction profile?

Gates (binary pass/fail assertions) exist as **supporting guardrails**: they catch catastrophic failures, record whether the basic outcome was achieved, and can optionally prevent wasting an expensive judge call on a run that produced no usable output. Gates are necessary but they are not the point. The point is the profile.

That said, binary gates are a perfectly valid use of the framework in CI/CD contexts — for example, a token-limit gate that fails if a model or harness change causes token usage to exceed a budget, or a cost gate that catches regressions. Used this way, the framework acts as a guardrail: "this change broke an assumption, it needs attention." The evaluation profile then tells you *what* changed and *how much*, which a binary gate alone cannot.

### Quantitative vs. Qualitative

The framework measures quality along two axes:

**Quantitative** — directly measurable from the transcript and metadata:
- Token usage, command count, error rate, first-try success rate, cost, duration, retry rate, help-seeking frequency
- These answer questions like _"Does a new model increase token usage?"_ and _"Did the richer docs reduce retries?"_

**Qualitative** — requiring intrinsic knowledge of how the tool is intended to be used:
- Did the agent follow the documented workflow, or find a circuitous workaround?
- Did it use the tool as its author intended?
- Was the output well-organized and consistent with tool conventions?
- These require judgment — from an LLM-as-judge scoring against a rubric that encodes the tool author's intent, or from a human reviewing the transcript.

### Three Layers

There are few universally applicable, objective measures of "did the LLM use this tool well." Rather than invent domain-specific metrics, the framework measures quality in three layers, ordered from cheapest to most expensive. Each layer answers a different question:

1. **Interaction quality** — Did the LLM use the tool efficiently? **(quantitative, evidence-derived, always available)** — the primary comparative signal
2. **Outcome assertions (gates)** — Did the task produce the right results? **(binary, supporting, scenario-author-defined)** — necessary but not the point
3. **LLM-as-judge** — Was the tool used as intended? **(qualitative, rubric-driven, optional)** — encodes intrinsic knowledge of how the tool should be used

---

## Layer 1: Interaction Quality

These metrics are derived from interaction evidence and are always available, regardless of the tool being tested. They measure *how* the LLM interacted with the tool, not *what* it produced.

### Interaction Evidence

Structured tool calls are the canonical evidence source for interaction
profiles. If an adapter supports structured tool calls, it must return
structured command events for normal completed runs. Returning transcript regex
evidence is an evaluation failure. Returning no usable target-tool events is an
evaluation failure unless the scenario declares `interaction.target_commands:
optional`.

Scenarios can set `interaction.target_commands` to:

- `required` (default): completed runs must include at least one target-tool
  event.
- `optional`: completed runs may include zero target-tool events.
- `forbidden`: completed runs fail interaction profiling if target-tool events
  are present.

Transcript regex analysis is fallback evidence only for adapters that cannot
provide structured tool calls. See ADR-0002.

### Command Identification

The interaction profile must know which commands belong to the target tool.
For structured evidence, target commands are identified from adapter-provided
command events using `target.binary`. For regex fallback evidence, this requires
a configurable command pattern, specified per scenario or globally:

```yaml
# In scenario YAML
target:
  command_pattern: "my-tool\\s+(\\S+)"
```

The pattern is configured on each scenario. It identifies target tool
invocations in the transcript when regex fallback is used. If the pattern
includes a capture group, the captured text is used to extract the subcommand
for per-subcommand analytics (e.g., distinguishing `my-tool create` from
`my-tool list`). If no capture group is present, only aggregate counts (total
commands, error rate, etc.) are available.

### Metrics

| Metric | Definition | Signal |
|--------|-----------|--------|
| **Error rate** | Proportion of target-tool commands that failed | Tool usability; unclear error messages |
| **Retry rate** | Commands repeated after failure (total - unique) / total | Error message quality; recovery difficulty |
| **Help-seeking** | Count of `--help` invocations | Documentation clarity |
| **First-try success rate** | Commands that succeeded on first attempt / total commands | Combined doc + UX quality |
| **Iteration ratio** | unique commands / total commands | Efficiency; high = less repetition |
| **Completion** | Did the agent complete the task vs give up or time out | Basic run-status signal |
| **Command count** | Total target-tool commands executed | Efficiency (fewer is better, given completion) |

### Data Source

Interaction profiles record their evidence source in `metrics.json` as
`interaction_evidence_source`.

- `structured_tool_calls`: metrics came from adapter-provided command events.
- `transcript_regex_fallback`: metrics came from `artifacts/transcript.raw.txt` using
  `target.command_pattern`.

Exit code detection for regex fallback is only as reliable as the transcript
content. Structured-capable adapters must not use this fallback.

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

---

## Layer 2: Outcome Assertions (Gates)

Gates are deterministic, scenario-author-defined checks. They answer: "did the task produce correct results?" Gates are domain-specific by nature, but expressed through generic primitives.

**Gates are supporting guardrails, not the primary evaluation output.** Their role is to catch catastrophic failures — when the agent didn't produce any output, created the wrong files, or failed the core task entirely. Implementations may use them to skip expensive judge calls on fundamentally broken runs, but the gate result itself is still only one dimension of the profile.

The real evaluation value is in Layer 1 (quantitative metrics) and Layer 3 (qualitative assessment). Gates are the safety net and a quick filter.

### Gate Types

The current implementation provides generic gate primitives that any CLI tool author can combine with scenario-specific commands and scripts.

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
| `no_transcript_errors` | *(none)* | Quality guardrail: fail when target-tool interaction evidence includes non-zero exit codes. Prefer outcome gates for task correctness. |
| `script` | `command: String`, `description: String` | Run script. Pass if exit code 0. Optionally returns structured JSON. See [docs/scripts.md](scripts.md). |

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
```

The scenario author brings domain knowledge; the framework provides the assertion primitives.

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

**This layer encodes intrinsic knowledge of how the tool is intended to be used.** Where Layer 1 measures _what happened_ (quantitative) and Layer 2 checks _whether the bare minimum was met_ (binary), Layer 3 evaluates _whether the tool was used well_ — a qualitative judgment that requires understanding the tool author's intent.

### When to Use

Use the judge for qualitative quality that quantitative metrics and gates cannot capture:
- Was the output well-organized and consistent with tool conventions?
- Did the LLM make reasonable decisions about how to structure data?
- Did the LLM follow the documented workflow, or discover the right path through trial and error?
- Did the LLM seem confused by the tool's interface, or did it use the tool fluently?
- Did the agent use the tool as its author intended, or find circuitous workarounds?

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

### Judge Tool And Model

The judge model should be:
- **Cheap and fast** — the judge call should cost a small fraction of the run itself.
- **Not the same model being tested** — to avoid self-evaluation bias.
- Configurable via the `--judge-model` CLI flag.

Recommended defaults: `gpt-4o-mini`, `claude-haiku`.

The judge CLI tool is configurable independently of the scenario target tool and the agent tool. It resolves in this order:
- `--judge-tool` CLI flag.
- `evaluation.judge.tool` in the scenario.
- `opencode` default.

Supported judge tools are `opencode`, `codex`, `claude`, and `claude-code`.

### Judge Execution

The judge is executed via a supported local CLI. This avoids a separate API dependency while keeping judge execution observable through the same command runner.

Execution flow:
1. Build a judge prompt containing the **tool name** (from `target.binary`), task description, transcript file reference, and rubric criteria. The tool name is parameterized so the judge can evaluate how effectively the agent used *that specific tool*.
2. Invoke the configured CLI tool via `SessionRunner`.
3. Parse stdout as JSON into `JudgeResponse`.

Current judge invocations:
- `opencode`: `opencode run --format json [--model <model>] <prompt>`
- `codex`: `codex exec --json --full-auto --skip-git-repo-check [--model <model>] <prompt>`
- `claude` / `claude-code`: `claude -p --output-format stream-json --verbose --include-partial-messages [--model <model>] <prompt>`

### Structured Output

The judge must return JSON matching the `JudgeResponse` schema:

```json
{
  "scores": { "command_correctness": 0.85, "task_completion": 0.90, "efficiency": 0.70 },
  "weighted_score": 0.83,
  "confidence": 0.80,
  "issues": ["Retried 'create' command 3 times with same args"],
  "highlights": ["Good use of search to verify data was captured"],
  "rationale": "The agent completed the task successfully but took a circuitous path, retrying the create command multiple times before correcting its syntax. It used search appropriately to verify intermediate results."
}
```

The `rationale` field is required — a 2–4 sentence explanation of the overall assessment that gives the evaluation consumer context for interpreting the scores.

### Pass Threshold

The scenario configures a `pass_threshold` (0.0–1.0). The judge layer passes if `weighted_score >= pass_threshold`. Treat this as a guardrail threshold and interpretation aid: it can mark the run outcome as failed, but the score, rationale, confidence, issues, and highlights are the qualitative evaluation result.

### Execution Guard

By default, the judge only runs **if all gates pass**. This prevents wasting an expensive LLM call on a run that fundamentally failed to produce output. If any gate fails, the judge is skipped and `judge_score` is `None`. This is a cost guard, not a statement that failed-gate runs lack qualitative value; users should still inspect the transcript and interaction metrics, and future implementations may make judge-on-gate-failure configurable.

### Scenario Configuration

Rubrics are optional. If `rubric` and inline `criteria` are both omitted, the
judge uses the default criteria: goal-oriented task completion (0.50), tool
usage correctness (0.30), and efficiency (0.20). The default task completion
criterion assesses whether the agent achieved the user's requested goal and
produced the intended outcome, not whether it followed a specific sequence of
steps.

```yaml
evaluation:
  judge:
    enabled: true
    tool: opencode
    pass_threshold: 0.70
```

To define custom criteria in a reusable rubric file, run `ax-eval template rubric`
for a copyable starting point and reference it via `evaluation.judge.rubric`.

---

## Composite Scoring

### Recommendation: Report Layers Independently for Comparative Evaluation

A single composite number obscures the dimensions that matter for comparative evaluation. If model A has the same guardrail outcome as model B but uses 3x more tokens, collapsing that into one number loses the signal.

The default behavior should be:

1. **Report each layer independently.** The evaluation output shows interaction metrics, gate results, and judge score as separate dimensions — because they answer different questions and are compared independently.
2. **Guardrails have a status, not a score.** Gates are `not_configured`, `passed`, or `failed`; failures include the named guardrails that failed. Judge passes if `weighted_score >= pass_threshold`. Interaction metrics are informational (no automatic pass/fail). These statuses are filters and triage aids, not the primary evaluation output.
3. **The evaluation profile is the primary output.** The set of scalar measurements across all layers is what enables comparisons across models, harnesses, and documentation variants.
4. **Run status** is a triage label derived from execution completion, guardrails, and judge threshold state. It is not the evaluation conclusion.

### Optional Composite Scoring

If a scenario author wants a single number, they can define weights explicitly:

```yaml
evaluation:
  composite:
    judge_weight: 0.85
    interaction_weight: 0.15
```

When `composite` is present, a composite score is computed from judge and interaction signals only. The default active weights are judge 0.55 and interaction 0.10, renormalized to sum to 1.0. If the judge is absent, the composite is renormalized to interaction only. If any guardrail fails, the composite is voided and omitted. When `composite` is absent, no composite score is reported.

### Run Status

The human-facing aggregate output reports the judge column as
`score (target) +/-delta`, for example `0.74 (0.80) -0.06`. It also reports a
run status, not an absolute pass/fail outcome. Status labels explain
what needs attention:

- `completed; judge threshold met`
- `completed; judge not run`
- `guardrail failed: file_exists(summary.md), file_contains(summary.md)`
- `judge threshold attention`
- `agent did not complete`
- `dry run; not executed`

Interaction metrics do not make a run pass or fail. They are diagnostic and
comparative. Gate and judge threshold states are quick filters over the run,
not the evaluation conclusion.

### Rust Representation

```rust
pub struct EvaluationMetrics {
    pub gate_status: GateStatus,
    pub details: Vec<GateResult>,
    pub judge_score: Option<f64>,
    pub judge_response: Option<JudgeResponse>,
    pub judge_passed: Option<bool>,
    pub judge_threshold: Option<f64>,
    pub efficiency: EfficiencyMetrics,
    pub interaction_evidence_source: InteractionEvidenceSource,
    pub composite_score: Option<f64>,
    pub evaluator_results: Vec<EvaluatorResult>,
}

pub struct RunStatus {
    pub gate_status: GateStatus,
    pub failed_guardrails: Vec<String>,
    pub judge_passed: Option<bool>,
    pub judge_score: Option<f64>,
}
```

`RunStatus` is derived from `EvaluationMetrics` and projects a triage outcome:
`"completed"`, `"guardrail failed: <names>"`, or
`"judge score X.XX below threshold"`. Human-facing status labels are produced
separately by `format_run_status` in the output module.

Human review is a workflow concern layered on top of the profile, not a
replacement for the dimensional evaluation data.

---

## Evaluation for Guidance Authors

The secondary audience for ax-eval is guidance/skills authors who are testing whether their AGENTS.md or skill definitions help LLMs use a tool effectively.

### Primary Signal: Quantitative Interaction Metrics

For guidance authors, Layer 1 metrics are the most important signal. Gates tell you whether the basic task outcome was achieved; interaction metrics tell you *how efficiently the LLM got there* and *how much friction it experienced*. This is the quantitative backbone of guidance evaluation.

**Do not** use `no_transcript_errors` as a gate in guidance scenarios, or as a
unit-test-style substitute for outcome assertions. Errors during discovery are
expected and informative. Gates should verify the final outcome (e.g., the
expected files exist, the exported data has the right shape); interaction
metrics should report how much friction the agent experienced while getting
there.

### Comparing Guidance Versions — The Core Workflow

The key workflow: run the same scenario with different AGENTS.md files and compare quantitative metrics.

```
Scenario: create_and_link
AGENTS.md v1: error_rate=0.35, first_try_success=0.55, tokens_out=4200, cost=$0.08
AGENTS.md v2: error_rate=0.10, first_try_success=0.82, tokens_out=1800, cost=$0.03
```

This tells the author that v2 of their documentation is substantially better — the LLM used fewer tokens, made fewer errors, and cost less. The gate outcome should be identical across variants; if it differs, the task is miscalibrated, not one variant automatically better.

### Qualitative Assessment of Guidance

The judge (Layer 3) adds qualitative depth: did the LLM follow the documented workflow, or stumble into the right answer? A rubric for guidance evaluation encodes the intrinsic knowledge that the tool *should* be used in a specific way, not just that the task *was* completed:

```yaml
criteria:
  - id: documentation_sufficiency
    weight: 0.50
    description: "Did the LLM appear to have sufficient information from the provided documentation to complete the task without confusion?"

  - id: workflow_clarity
    weight: 0.50
    description: "Did the LLM follow the documented workflow, or discover the correct approach through trial and error?"
```

### Diagnostic Patterns

| Pattern | Likely Cause |
|---------|-------------|
| High error rate, low help-seeking | LLM thinks it knows the syntax but doesn't. Docs may have incorrect examples. |
| High error rate, high help-seeking | `--help` output isn't sufficient. Missing examples or unclear argument descriptions. |
| Low error rate, high help-seeking | Docs don't include enough up front, but `--help` is good. Add more examples to AGENTS.md. |
| High retry rate on specific commands | That command's error messages don't help the LLM correct its approach. |
| High command count, task completed | Docs describe the commands but not the workflow. Add a "common workflows" section. |

---

## Not In Scope

- **Domain-specific quality metrics** (title length, tags per note, links per note, orphan detection) — these belong in scenario-specific gates and rubrics, not in the framework. The existing `QualityMetrics` / `StoreAnalyzer` module should be removed or moved to an example scenario.
- **Real-time cost tracking via provider APIs** — cost estimation from token counts and configured rates is sufficient.
- **Statistical significance testing across runs** — each run is evaluated independently.
- **Automatic regression detection** — the results database supports trend analysis, but automated alerting is not part of this spec.
