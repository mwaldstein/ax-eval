# llm-tool-test Context

## Glossary

### Tool Event Normalization

Tool event normalization is the conversion from a tool adapter's raw execution
output into canonical run artifacts used by the evaluation framework:
transcript text, command events, token usage, cost, and exit status. Concrete
adapters own tool invocation and adapter-specific raw-output parsing; shared
normalization owns canonical artifact construction helpers and cross-adapter
behavior expectations.

### Interaction Evidence

Interaction evidence is the input used to build the interaction profile for a
run. When an adapter supports structured tool calls, those structured events are
the required interaction evidence. Transcript regex analysis is fallback
evidence only for adapters that do not support structured tool calls. If an
adapter supports structured tool calls but fails to provide usable structured
events, evaluation must fail instead of falling back to transcript regex.
Adapter interfaces expose whether they support structured tool calls, and the
interaction profile module validates returned interaction evidence against that
adapter capability. For normal completed runs, an adapter that supports
structured tool calls must return at least one usable structured event; an empty
structured event list is an evaluation failure unless the scenario explicitly
declares that no target-tool interaction is expected.
The interaction profile module returns metrics together with evidence quality,
including whether the profile was built from structured tool calls or transcript
regex fallback. `EfficiencyMetrics` may remain the serializable metrics shape,
but it is not the full interaction profile interface.

### Discovery Workflow

Discovery is a top-level workflow for evaluating how self-describing a target
tool is and for producing goal-oriented scenarios from that understanding. It is
not only scenario generation. A discovery workflow first asks an agent to build
an understanding of what the target tool is for, how it works, and where it is
useful, with emphasis on the tool's "why". That understanding is preserved as a
reviewable artifact so the tool author can judge whether the tool explains
itself clearly before considering generated fixtures or run results.

Discovery may create reusable scenario fixtures, but those fixtures are
discovery outputs rather than assumed permanent project fixtures. A user may
keep, edit, promote, or discard them after the discovery run.

Discovery outputs live under the discovery run's results directory by default,
not under the repository's canonical `fixtures/` tree. Generated fixtures may be
promoted into project fixtures manually, but promotion is outside the discovery
workflow.

A discovery agent is the configured LLM CLI adapter used by the discovery
workflow for inspection, fixture authoring, and result summarisation. Discovery
uses the same adapter interface as scenario runs rather than a separate direct
LLM integration path.

The discovery target is the executable binary or command the workflow asks the
agent to understand. Discovery intentionally avoids additional context inputs in
its initial design because the workflow should reveal how well the executable
surface describes the tool without side-channel documentation supplied by the
test author.

The discovery understanding artifact is a required Markdown document describing
what the agent believes the tool is for, its core concepts, primary workflows,
useful goal areas, consulted evidence, self-description quality, ambiguities,
and scenario ideas. Discovery does not require a parallel machine-readable
understanding artifact in the initial design; the Markdown document is the
source artifact for humans and for subsequent discovery stages. Discovery
continues even when the understanding is low-confidence so the generated
fixtures, runs, and final summary show how poor self-description affects
downstream tool use.

Discovery-authored scenarios are intentionally ungated in the initial design.
Unlike normal project scenarios, they are not meant to fail fast on correctness
assertions. Their primary value is subjective usage evaluation: observing how an
agent understands the target tool's role, whether it can use the tool
effectively toward a goal, how many tool calls fail, and how qualitative judging
explains the quality of the attempt. In discovery, command failures are
diagnostic evidence rather than pass/fail verdicts.

Discovery scenarios should use qualitative judge evaluation as a first-class
result, even when no deterministic gates exist. The judge is not a secondary
clean-up step after pass/fail testing; it captures the subjective assessment
that llm-tool-test exists to make visible.

Discovery may still use a judge `pass_threshold` as a general rubric reference,
but pass/fail is not the primary discovery result. The more important output is
the judge's numeric assessment of how well the agent used the tool, along with
the rationale, confidence, issues, highlights, failed-call counts, and other
interaction metrics that explain that assessment.

Discovery uses five generated scenarios by default rather than
letting the inspecting agent choose the count. A fixed count makes discovery
runs easier to compare across tool versions, documentation changes, agents, and
models. The agent still chooses the scenario content within that count, with the
authoring prompt requiring complex, goal-oriented coverage.

Discovery authors its generated scenarios as one coordinated fixture set in a
single authoring pass. The authoring agent uses the understanding document to
balance coverage across all five scenarios and avoid near-duplicate goals.

Discovery fixture authoring produces complete runnable `llm-tool-test` scenario
YAML files directly, plus any required templates and rubrics. The authoring
stage may also create a README beside the generated scenarios when additional
annotation is worth preserving, but prose notes are not a substitute for
runnable scenario files.

Discovery runs generated scenarios immediately as part of the same top-level
command. The initial design does not include inspect-only or author-only modes;
the command's purpose is an all-in-one discovery run whose intermediate
artifacts remain available for review afterward.

Discovery should attempt every generated scenario even when one scenario has an
invalid fixture, harness failure, timeout, low judge score, or poor agent
outcome. Those failures are part of the discovery evidence and should be
classified in the final summary rather than stopping the workflow early.

Discovery runs the generated scenario set through one run invocation rather
than splitting the set into separate top-level runs. The resulting artifacts
should preserve the relationship between the five scenarios as one coordinated
discovery batch.

Discovery command success means the workflow completed and wrote its artifacts.
Low judge scores, failed target-tool calls, scenario-level pass/fail outcomes,
timeouts, and poor goal completion are discovery findings rather than top-level
command failures. The `discover` command should exit non-zero only when the
workflow itself cannot complete, such as when the target command is unavailable,
the discovery agent cannot run, generated fixtures cannot be parsed at all, or
the final artifacts cannot be written.

Discovery has the same explicit-consent requirement as normal real scenario
runs. Because it invokes LLM agents and allows agent-driven target command
execution, `discover` must require `LLM_TOOL_TEST_ENABLED=1` before performing
real work.

Discovery distinguishes the scenario-run agent from the discovery-authoring
agent. The base `--tool` and `--model` options identify the agent/model being
evaluated in the generated scenarios. Optional `--discover-tool` and
`--discover-model` options identify the agent/model used for the inspect,
fixture-authoring, and final-summary stages. When omitted, discovery authoring
defaults to the base tool/model. Judge tool/model options remain separate and
follow the existing judge defaulting behavior.

Discovery command syntax is target-first: `llm-tool-test discover <target>`.
The positional target is the executable binary or command being discovered.
Agent selection remains in flags so the target tool is not confused with the
LLM agent adapter that will inspect, author, run, or judge the scenarios.

Discovery does not use scenario-run caching in the initial design. Each
invocation should freshly inspect the target command surface, author a new
coordinated scenario set, run that generated batch, judge the results, and
summarise the findings. This keeps discovery sensitive to current tool
self-description, generated fixture quality, and agent behaviour.

Discovery validates every generated scenario before execution. Invalid
scenario files are discovery findings and should be reported with diagnostics
rather than silently ignored. If at least one generated scenario is valid,
discovery should run the valid subset and include invalid-fixture diagnostics
in the final summary. If zero generated scenarios are valid, discovery cannot
complete its run stage and should fail as a workflow error.

Discovery writes both an interpretive Markdown summary and a deterministic JSON
manifest. `discovery-summary.md` is authored by the discovery agent and explains
what the run reveals about the target tool's self-description and LLM usability.
`discovery.json` indexes the workflow artifacts and records stable metadata,
including generated scenario paths, scenario validity diagnostics, run IDs,
judge scores, failure classifications, aggregate token usage, aggregate cost
when available, and other resource totals captured by the harness.

Discovery usage accounting separates discovery overhead from evaluated
scenario-run usage. Inspect, fixture-authoring, and final-summary agent calls
are discovery overhead. Generated scenario executions are the evaluated usage.
`discovery.json` should report both categories separately and also provide
combined totals for budgeting.

Discovery reuses the existing scenario-run machinery internally rather than
shelling out to `llm-tool-test run`. The workflow loads generated scenarios
from the discovery artifact directory, runs them as one coordinated generated
batch with caching disabled, and records paths, result IDs, errors, and usage
totals under the discovery result.

Discovery result directories include the target command and evaluated
agent/model in the directory name so the main comparison axes are visible at a
glance, for example
`llm-tool-test-results/20260509-143012-discover-qipu-opencode-gpt-5/`.
`discovery.json` records the exact target, agent, and model values, including
any values that are sanitized or truncated for filesystem-friendly paths.

Discovery does not include a dry-run mode in the initial design. Its value
depends on real LLM inspection, fixture authoring, generated scenario
execution, judging, and summary. The explicit `LLM_TOOL_TEST_ENABLED=1`
requirement is the safety mechanism for avoiding accidental real runs.
