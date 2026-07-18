# The Role of Gates — Deep Evaluation, 2026-07-18

Companion to [audit-2026-07.md](audit-2026-07.md) (code correctness) and
[strategic-review-2026-07.md](strategic-review-2026-07.md) (intent and
positioning). This document examines one subsystem — gates — against its own
stated role, because that role is where the framework's evaluation philosophy
is most easily confused, by scenario authors and by the implementation itself.

References are current as of commit `4d2e223`.

## The stated role

The documentation is unusually explicit about what gates are supposed to be:

- "Gates (binary pass/fail assertions) exist as **supporting guardrails**:
  they catch catastrophic failures … Gates are necessary but they are not the
  point. The point is the profile." (docs/evaluation.md)
- "They are guardrails, not the primary evaluation signal." (CONTEXT.md)
- "Gate pass rate is a sanity check, not the main signal."
  (docs/guidance-testing.md)
- "Keep gates minimal … verify the outcome, not the process."
  (docs/scenarios.md)

So the doctrine is: gates are a **binary safety net over outcomes**, subordinate
to the evaluation profile. The doctrine is coherent and correct for what this
tool is. The problem is that the implementation quietly assigns gates six
additional jobs, several of which contradict the doctrine.

## The actual roles: an inventory

Tracing `gates` through the pipeline, a gate result is consumed in six places
with six different semantics:

| # | Role | Where | Semantics |
|---|------|-------|-----------|
| 1 | **Outcome assertion** (the intended job) | `evaluate_gates` (`src/evaluation.rs:111`) | Binary checks against post-run workspace state |
| 2 | **Judge admission control** | `maybe_run_judge` (`src/evaluation/judge.rs:68`) | *Any* single gate failure suppresses the entire qualitative layer |
| 3 | **Composite score component** | `compute_composite_score` (`src/eval_helpers.rs:16-20`) | `gates_passed / gates_total` as a scalar, weight 0.35 by default |
| 4 | **Run verdict** | `RunStatus::outcome` (`src/run/status.rs:23-34`) | The literal string `"Fail: N/M gates passed"` vs `"Pass"` |
| 5 | **DB roll-up flag** | `ResultRecordInput::build` (`src/run/records.rs:25`) | `results.jsonl`'s `gates_passed: bool` is actually `legacy_gates_passed()` = gates **AND** judge threshold |
| 6 | **Process/interaction assertion** | `no_transcript_errors` (`src/evaluation/gates/interaction.rs`) | A Layer-2 gate that reads Layer-1 evidence |

Each role individually has a rationale. Together they mean the word "gate"
does not have one meaning in this system, and the confusions below all trace
back to that.

## Confusion 1: guardrail vs. score input

The doctrine says gates are binary guardrails. The composite score treats them
as a *graded quantity*: `gates_passed / gates_total` at a default weight of
0.35 — more than triple the interaction layer's 0.10 (`src/eval_helpers.rs:11`).
Two consequences:

- **Gate cardinality becomes a hidden scoring parameter.** A scenario with ten
  fine-grained gates and one with two coarse gates produce incomparable
  composites for identical agent behavior. Fraction-of-gates is only a
  meaningful scalar if gates are equally-sized units of task value, which
  nothing encourages or checks. An author who follows the docs' advice ("keep
  gates minimal") *changes their scores* relative to an author who doesn't.
- **A scenario with zero gates scores its gate component as 0.0**
  (`src/eval_helpers.rs:16-20`), while `RunStatus` treats zero gates as passed
  (`0 >= 0`, `src/run/status.rs:13`). A judged, deliberately ungated scenario —
  exactly the shape `discover` mandates — loses the full 0.35 gate weight from
  its composite if composite weights are configured. This is the same
  weight-renormalization family as the audit's `judge_score.unwrap_or(0.0)`
  finding, and should be fixed in the same pass.

The root cause is that a gate *count* exists at all. Counting is what turns a
guardrail into a metric — see "Resolution: remove gate statistics" below.

## Confusion 2: guardrail vs. admission control — all gates are "catastrophic"

The judge runs only when **all** gates pass (`src/evaluation/judge.rs:68`).
The docs justify this as a cost guard against judging runs that "produced no
usable output." That justification is valid for one *kind* of gate — the
catastrophic kind the doctrine describes (workspace empty, tool never ran).

But the schema has no way to say which kind a gate is. In practice, authors
write fine-grained outcome detail as gates — the shipped examples do:
`file_contains: substring "Project"` (`example_basic`), a specific search-term
match, an exact JSON length. When such a detail gate fails on an otherwise
substantive run:

1. The judge is silently skipped — the qualitative layer vanishes precisely on
   the run most worth interpreting (see the strategic review's "diagnostic
   judge" argument).
2. The composite (if configured) zeroes the 0.55 judge component on top of the
   gate deduction (audit P0 finding).
3. The run is stamped `"Fail: 2/3 gates passed"`.

One substring case-mismatch thus converts a rich, mostly-successful, *paid*
run into a near-zero record with no qualitative explanation. That is not a
guardrail; that is a single point of failure with veto power over two of the
three layers. The doctrine says gates catch catastrophes; the mechanism makes
**every** gate a catastrophe.

The missing concept is **severity**. Two honest tiers would resolve it:

- `required` (default, backward-compatible): failure marks the outcome and
  skips the judge — for "nothing to evaluate" conditions.
- `informational` / `check`: failure is recorded in the profile and reports
  but neither vetoes the judge nor dominates the composite — for outcome
  detail.

(Equivalently: keep `gates:` for the catastrophic tier and add a separate
`checks:` list. Naming matters less than the distinction existing.)

## Confusion 3: the verdict strings contradict the philosophy

evaluation.md is emphatic that run status is "a triage label … not an absolute
pass/fail verdict," and the human-facing labels (`guardrail attention: N/M
gates`) honor that. But `RunStatus::outcome` — the string persisted in every
`results.jsonl` record — is literally `"Pass"` / `"Fail: N/M gates passed"`
(`src/run/status.rs:23-34`). Whatever the docs say, the artifact users query
and script against renders gates as the verdict. Anyone building dashboards on
the DB will treat `outcome` and `gates_passed` as the result of the run,
reconstructing exactly the testing-tool mental model the project defines
itself against.

Compounding it, the DB field `gates_passed` does not mean gates passed: it is
`gates && judge_threshold` (`src/run/records.rs:25`, `src/run/status.rs:19-21`).
The method is honestly named `legacy_gates_passed`, but the serialized field
keeps the misleading name forever. A judge-threshold miss shows up in the
database as a gate failure. This is a data-contract bug in the same sense as
the audit's metric findings: the number is not what its name says. Dry runs
additionally stamp `gates_passed: true` (`src/run/records.rs:69`; audit P2).

Fix direction: persist the layers separately (`gate_status`, `judge_status`,
`run_state`) and keep any combined verdict out of the record — or name it
`legacy_passed` and deprecate it. This belongs with the audit's "structured
judge skip reason" item; together they make `results.jsonl` layer-faithful.

## Confusion 4: a Layer-1 assertion wearing a Layer-2 costume

`no_transcript_errors` is defined as a gate but reads the interaction profile
(`src/evaluation/gates/interaction.rs`) — it asserts about *process*, not
*outcome*. The docs then spend paragraphs warning people not to use it for its
apparent purpose (evaluation.md: "**Do not** use `no_transcript_errors` … in
guidance scenarios"; scenarios.md repeats the warning), while the
comprehensive example ships it anyway.

A primitive whose documentation is mostly warnings is in the wrong place. The
scenario schema already has the right home for interaction-level policy: the
`interaction:` block, which is where `target_commands: required|optional|
forbidden` — the *other* hard interaction assertion — already lives. A
`interaction.max_error_count: 0` (or `errors: forbidden`) field would express
the same constraint in the layer that owns the evidence, and the gate could be
deprecated. That would also restore a clean invariant worth writing down:
**gates assert only on post-run workspace state; the interaction block asserts
on interaction evidence; the judge asserts on intent.** One sentence, three
layers, no overlap.

## Confusion 5: what gates measure is end-state, under the agent's influence

Two properties of gate execution are undocumented and worth stating:

- **Gates measure final state, and can mutate it.** Command gates run
  arbitrary shell in the workspace, in declaration order, with no
  short-circuit (`src/evaluation/gates.rs:74-91`). A `command_succeeds` with
  side effects changes the workspace that later gates, evaluators — and any
  human inspecting `fixture/` — observe. Nothing warns authors that gate order
  can matter or that gates should be read-only. A one-line authoring rule
  ("gates must not mutate the workspace") plus a doc note would close this.
- **Gates trust a workspace the agent just controlled.** `command_succeeds:
  "./demo_tool status"` executes a binary that the evaluated agent had full
  write access to moments earlier. For today's threat model (local author
  evaluating their own tool) this is a documented-tradeoff-shaped issue, not a
  vulnerability — but it composes badly with the roadmap: shared scenario
  packs and discovery-generated scenarios mean gate commands authored by one
  party executing in state produced by another party's agent, with the
  caller's privileges. The audit flagged the path-traversal half of this
  (file gates joining `../`); the execution half belongs in
  `docs/tradeoffs.md` now and in the sandboxed-runner design later. Preferring
  fixture-relative invocations and treating gate commands as untrusted input
  in shared packs are the mitigations.

## Confusion 6: gates can only say "did," never "shouldn't have"

The gate vocabulary is entirely positive: exists, contains, matches, succeeds.
There is no `file_absent`, no `command_fails`, no "path untouched outside X."
For an *agent-experience* tool this is a real expressiveness gap, because many
of the guardrails a tool author cares about are negative-space properties:

- the agent did **not** scatter files outside the intended directory;
- the agent did **not** use the destructive flag the docs warn about;
- the tool's failure mode was clean — the command **should** fail, and the
  agent should recognize it (evaluating error-message AX requires asserting
  failure as the expected outcome).

`interaction.target_commands: forbidden` shows the designers have already
needed negation once. A small negative-gate family (`file_absent`,
`command_fails`, optionally `file_untouched`) would let guardrails express
"catastrophe" in both directions and would materially strengthen the
CLI-error-message evaluation story, which is core to the product's thesis.

## Resolution: remove gate statistics entirely

The project's position (adopted during this review): gate statistics should be
**removed**, not reweighted. The principle:

> **A guardrail is a fact, not a quantity.** The moment gates are counted —
> `3/4 passed`, a pass rate, a weighted fraction — they become a score, and
> every consumer downstream (composite, dashboards, humans scanning output)
> will treat them as the evaluation. Recording *which* gate failed and *why*
> is evidence; counting them is a metric. Gates should produce the former and
> never the latter.

This is stronger than the "veto or multiplier" fix considered above, and
better: a multiplier still admits gates into arithmetic. Under removal, the
only aggregate that survives is a **boolean** — "all required gates passed" —
which is the one question a guardrail can legitimately answer, plus the named
list of failures. Numbers disappear; names replace them. `guardrail
attention: 2/3 gates` becomes `guardrail failed: file_exists(summary.md),
file_contains(summary.md)` — which is simultaneously less metric-like and
*more* actionable.

### Every surface where gate statistics currently leak

| Surface | Today | After removal |
|---|---|---|
| `metrics.json` (`EvaluationMetrics`, `src/evaluation/profile.rs:41-42`) | `gates_passed: usize`, `gates_total: usize` | `gate_status: not_configured \| passed \| failed` + existing `details` list |
| `results.jsonl` (`EvaluationMetricsRecord`, `src/results/types/mod.rs:63-65`) | same counts | same status enum; failed gate names queryable from `details` |
| `results.jsonl` roll-up (`ResultRecord.gates_passed: bool`, `src/results/types/mod.rs:42`) | gates AND judge conflated | delete; superseded by per-layer statuses (Confusion 3) |
| Composite (`src/eval_helpers.rs:16-20`) | `gates_passed/gates_total` × 0.35 | no gate term. Composite = judge + interaction, renormalized; computed only when required gates pass. A failed guardrail voids the composite (absent), never scores it |
| `CompositeConfig.gate_weight` (`src/scenario/types.rs:169-181`) | schema field, default 0.35 | remove field; reject with a migration hint in validation |
| Outcome string (`src/run/status.rs:23-25`) | `"Fail: N/M gates passed"` | `"guardrail failed: <gate names>"` |
| Status label (`src/transcript/writer.rs:18-21`, `src/output.rs:64-67`) | `guardrail attention: N/M gates` | `guardrail failed: <gate names>` |
| `report.md` / `evaluation.md` (`src/transcript/writer.rs:266,440`) | `**Gates Passed**: N/M` | `**Guardrails**: passed` or the failure list |
| CLI summary (`src/output.rs:29-30`) | `N/M gates` column | pass/fail marker; names on failure |
| README metrics examples; scenarios.md "Gate pass rate" row; guidance-testing.md "gate pass rate is a sanity check" | rates and counts as vocabulary | strike the phrase "gate pass rate" from the project's vocabulary entirely; replace with "gate outcome should be *identical* across variants — if it differs, the task is miscalibrated, not one variant better" |

The per-gate `details` list (`GateResult { gate_type, passed, message }`) is
**kept unchanged** — it is diagnostic evidence, exactly what the doctrine
wants gates to produce.

### What this costs, honestly

- **Partial credit disappears.** "The agent achieved 3 of 4 outcomes" can no
  longer be read off a number. That is the point: graded outcome quality is
  the judge's job (or informational checks under the severity split), and a
  *required* guardrail set that is partially satisfiable was mis-designed —
  the doctrine already says required gates mark catastrophes, and there is no
  such thing as 75% non-catastrophic.
- **It is a breaking schema change** to `metrics.json`, `results.jsonl`, and
  the scenario's `composite` block — strictly bigger than the audit's metric
  fixes. It should land in the same CHANGELOG-flagged release as the
  metric-integrity cluster, and it makes the `schema_version` field (audit
  P2, strategic review rec 5) a prerequisite rather than a nicety.
- **One count had a legitimate job**: `gates_total` distinguished "no gates
  defined" from "gates defined and passed." The three-valued status enum
  (`not_configured | passed | failed`) preserves that distinction without a
  number.

## What the design already gets right

Credit where due — several gate decisions are better than the surrounding
confusion suggests, and should be preserved through any rework:

- **No short-circuit between gates** (`src/evaluation/gates.rs:78-88`): the
  full gate picture is always available; partial information isn't discarded.
- **Generic primitives + author-supplied domain knowledge** (evaluation.md's
  `command_json_path` example): the framework refuses to bake in
  domain-specific quality metrics, which is the correct boundary.
- **The script gate's structured-JSON contract** gives an escape hatch without
  schema sprawl.
- **Discovery's gate prohibition is the buried gem.** Discovery-generated
  scenarios *must* have `gates: []` and use the judge as primary evaluation
  (`src/scenario/discovery.rs:16-17`). The project has already discovered the
  correct principle: **a gate encodes ground truth the author knows in
  advance; where no ground truth exists, gates are forbidden.** Today that
  principle is enforced in one code path and stated nowhere. Promote it to the
  first sentence of the gate documentation — it is the cleanest available
  definition of the gate's role, and it resolves most authoring confusion
  ("should this be a gate or a rubric criterion?" — answer: is it ground
  truth you can state before the run?).

## Recommendations

Ordered by leverage; items 1–3 change semantics and belong together in one
CHANGELOG-flagged release, alongside the audit's metric-integrity cluster
(they touch the same functions).

1. **Remove gate statistics** (see Resolution section): delete the
   `gates_passed`/`gates_total` counts, the composite gate term and
   `gate_weight` field, and every `N/M` rendering. Replace with a three-valued
   `gate_status`, the named-failure list, and a judge+interaction composite
   renormalized and computed only when required gates pass. This resolves
   Confusion 1 at the root and most of Confusion 3's surface.
2. **Introduce gate severity** (`required` vs. informational `check`), with
   judge-skip and run-outcome tied to the required tier only. Resolves
   Confusion 2 and unblocks the diagnostic-judge feature from the strategic
   review. Informational checks are likewise reported as named facts, never
   counted.
3. **Make persisted records layer-faithful**: delete the conflated DB
   `gates_passed` boolean in favor of per-layer statuses; stop persisting
   `"Pass"/"Fail"` strings that the docs disavow; give dry runs a distinct
   status (audit P2). Ship 1–3 together behind a `schema_version` bump.
4. **Relocate `no_transcript_errors`** to the `interaction:` block as a policy
   field; deprecate the gate. Document the resulting invariant: gates =
   workspace state, interaction block = process evidence, judge = intent.
5. **Add negative gates** (`file_absent`, `command_fails`) to let guardrails
   express expected failure and forbidden side effects — high value for
   error-message AX evaluation.
6. **Document gate execution semantics**: gates must be read-only; they run
   in declaration order against agent-influenced state with caller
   privileges. One paragraph each in scenarios.md and tradeoffs.md; enforce
   path containment per the audit.
7. **Canonize discovery's principle** ("gates encode pre-known ground truth")
   as the opening definition in evaluation.md's Layer 2 section and the
   scenario template comments.

## Bottom line

The doctrine — gates as binary outcome guardrails subordinate to the profile —
is right, and the docs state it repeatedly and well. But the implementation
gives gates six jobs, and four of them (judge veto, composite fraction, DB
roll-up, verdict string) quietly re-promote gates to the primary signal the
docs insist they are not. The result is that the system's *strongest stated
principle is contradicted by its own defaults*: a tool author following the
examples will experience gates as the pass/fail verdict, cardinality-weighted
score, and judge kill-switch all at once. The adopted resolution — remove gate
statistics entirely, leaving only a boolean status, named failures, and the
per-gate evidence list — is the decision that makes the doctrine
self-enforcing: once gates cannot be counted, nothing downstream can quietly
turn them back into a metric. Combined with severity tiers and layer-faithful
records, the evaluation profile becomes what every layer of documentation
already promises it is.
