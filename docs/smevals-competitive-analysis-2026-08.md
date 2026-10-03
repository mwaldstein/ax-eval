# smevals Competitive Analysis — 2026-08-01

## Executive Summary

[smevals](https://primeradiant.com/blog/2026/smevals.html) is mostly
complementary to ax-eval's core product, but directly competitive with
ax-eval's experiment-running and reporting layer.

The cleanest product distinction is:

- **smevals** asks: "Which model, prompt, configuration, or harness performs
  best on these tasks?"
- **ax-eval** asks: "How well can agents discover and use this CLI or MCP
  product, and what should its author improve?"

smevals becomes a serious competitor if ax-eval presents itself as a general
model or harness evaluation framework. It is less threatening, and potentially
a useful integration partner, if ax-eval owns the narrower category of an
agent-experience laboratory for developer tools.

The most important competitive signal is not smevals' checker design. It is
the experiment lifecycle around the checkers: repeated sampling, immutable
runs, regrading without rerunning, aggregate uncertainty, and browsable or
static reports. Those capabilities expose ax-eval's most important current
gap: ax-eval promises comparative measurement but still relies primarily on
manual comparison of independent run artifacts.

## Product Comparison

| Dimension | smevals | ax-eval | Implication |
|-----------|---------|---------|-------------|
| Primary object | Model or configuration capability | Agent-facing CLI or MCP target | Strong differentiation for ax-eval |
| Experimental variables | Model, prompt, parameters, harness | Target, model, harness, guidance | Significant overlap |
| Evidence | Output plus arbitrary runner artifacts | Structured CLI/MCP calls, transcripts, outcomes, tokens, cost | ax-eval's strongest moat |
| Evaluation model | Ordered checks producing a grade | Interaction profile, guardrail gates, evaluators, and judge | ax-eval has the richer measurement model |
| Repetition | Native `-n N` with balanced top-up | Independent single runs | Major smevals advantage |
| Aggregation | Mean plus standard error, rates, and tags | Manual comparison of run artifacts | Major smevals advantage |
| Regrading | Immutable runs, multiple graders, no model rerun | Evaluation is largely coupled to execution | Major smevals advantage |
| Presentation | Terminal report, live UI, static site | Per-run Markdown/JSON and matrix summary | Major smevals advantage |
| Tool-specific insight | Whatever a custom runner or checker implements | Normalized target calls and AX-oriented metrics | Major ax-eval advantage |
| Target provisioning | Delegated to the runner | First-class CLI/MCP provisioning and authentication | Major ax-eval advantage |
| Scenario discovery | An agent can author an eval from bundled documentation | `discover` inspects a CLI and generates scenarios | Major ax-eval advantage |

The [smevals README](https://github.com/prime-radiant-inc/smevals/blob/main/README.md)
defines a general task/config/run/grade vocabulary and supports arbitrary
runner and checker executables. Its
[published haiku report](https://static.simonwillison.net/static/2026/smevals-haiku-build/)
shows the aggregation and reporting functionality as working product surface,
not merely roadmap.

## Complementary Relationship

A natural layered workflow is possible:

```text
smevals
  experiment matrix + repetitions + aggregation + publishing
        |
        `-- ax-eval-powered runner
              target provisioning
              real coding-agent execution
              structured CLI/MCP traces
              interaction profile
              AX-oriented diagnosis
```

In that arrangement, smevals determines which configuration performs best
across repeated trials, while ax-eval explains how the agent interacted with
the target and where friction occurred.

A small interoperability proof of concept would be worthwhile: a wrapper that
accepts smevals task and config environment variables, invokes ax-eval, and
retains the ax-eval run bundle as smevals artifacts. This should validate a
portable run/export contract, not make smevals a core dependency.

## Competitive Relationship

The competitive overlap is strongest for users who want to compare models or
agent harnesses. smevals currently provides several capabilities that ax-eval
does not:

- Repeated runs as a basic concept rather than an advanced future feature.
- Balanced sampling across task/model pairs.
- Aggregate uncertainty rather than an apparent single-run winner.
- Regrading after checker changes without paying to rerun the model.
- A usable leaderboard and individual-run browser.
- A simple Python/`uvx` installation and arbitrary executable runner contract.

This validates the n=1 concern already identified in
[the July strategic review](strategic-review-2026-07.md#1-the-n1-problem-contradicts-the-core-promise).
The comparison promise is ahead of the statistical methodology.

## Recommended Changes

### 1. Make Repeated, Matched Comparison the Immediate Priority

Implement repeated sampling and matched comparison before adding more target
types or interaction metrics. A possible surface is:

```text
ax-eval run ... --samples 5
ax-eval compare --baseline <selector> --candidate <selector>
```

The aggregate should report, for every evaluation dimension:

- Attempt count and completion rate.
- Guardrail pass rate.
- Mean, median, spread, and a confidence interval where meaningful.
- Judge and evaluator score distributions.
- Interaction-metric distributions.
- Cost and duration distributions.
- The paired delta when runs share the same scenarios.
- An inconclusive result when variance overwhelms the observed difference.

Unlike smevals, ax-eval should not top up until it obtains N successful agent
runs. Agent noncompletion is often evidence about the target experience. It
should count N attempts, classify their termination, and exclude only genuine
infrastructure failures.

This work is already described under
[Evaluation depth](../TODO.md#evaluation-depth). smevals raises its urgency
from an important roadmap item to a competitive requirement.

### 2. Separate Execution Evidence From Evaluation Versions

Adopt smevals' strongest architectural idea:

- An agent execution produces an immutable run bundle.
- Evaluating that bundle produces a versioned evaluation profile.
- Multiple evaluation profiles may coexist for one run.
- Each profile stores hashes or snapshots of its gates, evaluators, rubric,
  judge configuration, and framework version.
- A command such as `ax-eval evaluate <run-id> --profile <name>` can apply new
  evaluation logic without another paid agent run.

This should replace result caching as the primary answer to grader iteration.
[ADR-0004](adr/0004-result-caching-is-opt-in.md) already anticipates both
N-run aggregation and artifact re-evaluation.

### 3. Introduce an Explicit Experiment and Variant Layer

Keep `Scenario` as the unit of work, but add a higher-level experiment
definition. For example:

```yaml
name: richer_error_messages
scenarios:
  - create_project
  - recover_from_invalid_input

variants:
  baseline:
    target_build: ./target/baseline/mytool
    guidance: fixtures/minimal/AGENTS.md

  candidate:
    target_build: ./target/candidate/mytool
    guidance: fixtures/rich/AGENTS.md

samples: 5
```

A variant should be able to pin:

- Target build or version.
- Agent harness and version.
- Requested and resolved model identifier.
- Prompt or system-prompt variant.
- Guidance and skill materialization.
- Environment policy.
- Scenario and fixture hashes.

This makes "vary one thing" a machine-enforced experimental contract instead
of only a documentation recommendation.

### 4. Add Aggregate Reporting After the Statistics Are Trustworthy

Borrow smevals' presentation model:

- `ax-eval report` for terminal or Markdown aggregate output.
- `ax-eval serve` for local exploration.
- `ax-eval build` for a shareable static report.

The UI should prioritize dimensional AX diagnosis rather than a generic model
leaderboard:

- Ability, efficiency, and agent-facing structure shown separately.
- Baseline/candidate deltas and uncertainty.
- Cost-versus-quality Pareto views.
- Drill-down from an aggregate metric to exact trace events.
- Filters for failure categories and grounded judge findings.
- Side-by-side CLI-versus-MCP comparisons.

A single overall ranking should remain optional. Making it the default would
undermine ax-eval's three-layer model in [Evaluation](evaluation.md).

### 5. Complete Measurement-Context Fingerprinting

Every run should record:

- Target version or content hash.
- Agent CLI name and version.
- Adapter and ax-eval versions.
- Requested and resolved model identifiers.
- Fixture, guidance, skills, scenario, evaluator, and rubric hashes.
- Relevant isolation and environment facts.
- Token and cost provenance.

Without this fingerprint, a historical comparison may reflect model alias
drift or a harness update rather than a target improvement.

### 6. Add Typed Failure Classification

Preserve clear distinctions between:

- Preflight or configuration failure.
- Provider, network, or harness infrastructure failure.
- Agent timeout or noncompletion.
- Target interaction failure.
- Outcome guardrail failure.
- Evaluation or judge failure.

smevals excludes a non-zero runner exit because its contract defines that as
an infrastructure failure. ax-eval cannot adopt that rule wholesale: an agent
giving up, timing out, or crashing while using a confusing target can be
central evaluation evidence.

### 7. Tighten the Market Position

Lead with a narrow description:

> ax-eval measures and improves the agent experience of CLIs and MCP servers.

De-emphasize generic claims about evaluating models or harnesses. General eval
frameworks such as smevals will be broader and simpler for that job.

The strongest differentiated stories are:

- Improve CLI help and errors using observed agent friction.
- Improve MCP descriptions and schemas using structured calls.
- Diagnose why agents fail with a target.
- Compare guidance variants.
- Compare CLI and MCP versions of the same capability.
- Let an agent discover, evaluate, modify, and reevaluate its own tool surface.

## Capabilities Not to Copy

Some smevals choices fit its general-purpose scope but would weaken ax-eval:

- **Do not collapse everything into a grade.** ax-eval's dimensional profile
  is more informative.
- **Do not use the last checker's score as the overall score.** Explicit
  criterion or composite weights are more defensible.
- **Do not short-circuit all outcome evaluation after the first failed
  check.** ax-eval deliberately runs every gate to preserve the complete
  outcome picture.
- **Do not exclude behavioral failures from samples.** Only infrastructure
  faults should be statistically excluded.
- **Do not generalize the runner so far that structured target evidence
  becomes optional.** That evidence is the product moat.
- **Do not build a polished leaderboard before repeated-run semantics and
  context pinning are sound.**

## Suggested Sequence

1. Repeated samples and balanced matrices.
2. Immutable run bundles and re-evaluation.
3. Matched `compare` with uncertainty.
4. Measurement-context fingerprints.
5. Aggregate terminal report.
6. Static and live report UI.
7. Experiment variants and cost/quality views.
8. smevals interoperability proof of concept.
9. Diagnostic judge for failed runs.
10. A statistically credible CLI-versus-MCP case study.

## Conclusion

smevals does not invalidate ax-eval's direction. It clarifies which parts of
that direction are differentiated and which are becoming commodity.

General task execution, model matrices, executable graders, and leaderboards
are competitive infrastructure. Structured target interaction evidence,
CLI/MCP provisioning, agent-facing structure assessment, and actionable AX
diagnosis are ax-eval's differentiated product.

The correct response is therefore not to broaden ax-eval into another generic
eval framework. It is to adopt the experiment rigor that smevals demonstrates
while sharpening ax-eval around tool-side agent experience.
