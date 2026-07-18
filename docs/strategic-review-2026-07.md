# ax-eval Strategic Review — 2026-07-18

Companion to [audit-2026-07.md](audit-2026-07.md). Where the audit examined
implementation correctness, this review examines the system's intent, its
evaluation methodology as a *measurement design*, and its role in the
development of AI-facing tools. A follow-up deep dive on one subsystem's role
lives in [gates-review-2026-07.md](gates-review-2026-07.md). Sources: README, CONTEXT.md, docs/evaluation.md,
docs/guidance-testing.md, docs/harness-materialization.md, docs/tradeoffs.md,
ADRs 0001–0005, TODO.md, plus a July 2026 scan of the surrounding ecosystem.

## What the system is, stated back

ax-eval inverts the usual evaluation question. The 2026 landscape of agent
evaluation — Promptfoo, DeepEval, LangSmith, MLflow's agent evaluators — asks
*"how good is my agent/LLM app?"* Benchmark suites (SWE-bench, terminal-bench
and kin) ask *"how good is this agent at tasks?"* ax-eval holds the agent
constant and asks *"how good is my tool at being used by agents?"* — measuring
the tool's CLI surface, error messages, and guidance docs by observing a real
agent's friction while using them.

That inversion is the product. It is also a genuinely underserved niche:
"Agent Experience (AX)" is now an established term with a growing community
(agentexperience.ax), AGENTS.md has Linux Foundation governance and 60k+
repository adoption, and design-principle projects like AXI publish
agent-ergonomics claims backed by exactly the kind of controlled runs ax-eval
automates. The demand side of this tool's thesis has materialized. Nothing
found in the current ecosystem occupies the same spot as a local, harness-level,
tool-side evaluation runner.

## Where the design thinking is sound

These are intent-level strengths, distinct from the code strengths the audit
listed:

- **Evaluation-not-testing framing.** The profile-over-pass/fail stance
  (docs/evaluation.md) is the correct epistemology for a noisy measured system,
  and the three-layer design (interaction metrics → gates → judge) maps cleanly
  onto a cost/validity gradient: cheap-and-objective first, expensive-and-
  interpretive last, with gates explicitly demoted to guardrails. Most eval
  tools get this backwards.
- **Comparison discipline in the docs.** The framework consistently frames
  metrics as *comparative within a controlled pair* (same scenario, same agent,
  vary one thing) rather than absolute scores. The diagnostic tables in
  evaluation.md ("high error rate + low help-seeking → docs may have incorrect
  examples") are the most valuable interpretive content in the project.
- **Self-consistency as a design principle.** ADR-0003 (the CLI must itself be
  agent-usable) and the README's agent-driven workflow make the tool an
  instance of its own thesis. This is both honest engineering and the correct
  distribution strategy — see "role" below.
- **Reasoned refusals.** ADR-0004's argument against default caching (a cache
  hit presents one past sample of a non-deterministic system as current
  capability) and the tradeoffs doc's explicit acceptance of partial fixture
  isolation show a project that knows *why* it declines features. That
  discipline is rare and worth protecting.
- **Direction tracks the ecosystem.** Harness materialization (guidance +
  skills provisioning per harness) and MCP targets (ADR-0005) are the right two
  bets: guidance files and skills are standardizing, and the "tool surface
  agents consume" is visibly splitting between CLI and MCP.

## Strategic tensions

These are not bugs; they are places where the system's stated intent and its
current shape pull against each other.

### 1. The n=1 problem contradicts the core promise

This is the most important finding in this review. The product promise is
*"apples-to-apples comparisons … to guarantee your updates reduce friction"*
(README), against a system the project itself declares non-deterministic
(ADR-0004's entire argument). Yet:

- Every run is a single sample; statistical treatment is explicitly out of
  scope (evaluation.md "Not In Scope").
- The flagship workflow (guidance-testing.md) instructs users to compare
  single-run `metrics.json` files by eye.
- `ax-eval compare`, iterations, and significance testing all sit under
  "Future" in TODO.md.

The failure mode is not "missing feature" — it is *confidently wrong
conclusions*. A guidance author who sees error_rate drop from 0.35 to 0.10
across two single runs has learned very little, but the tool's framing tells
them they've verified an improvement. For a measurement tool, shipping the
comparison story without the variance story risks the credibility that is its
only real asset. The code audit filed multi-run variance under Priority 2;
strategically it is the first-priority investment, because it protects the
central claim rather than any single number.

Concretely: `--iterations N` with mean/spread in the profile, and a minimal
`compare` that refuses to declare a winner when the spread swamps the delta,
would convert the README's "guarantee" from aspiration to method. Until then,
the honest cheap fix is wording: the README should say "compare" where it
currently says "guarantee."

### 2. The instrument drifts under the measurement

The measured system is (agent CLI × model × environment × tool-under-test ×
guidance). ax-eval controls the last two and inherits the first three:

- Agent CLIs ship weekly; models are aliased and re-pointed upstream.
- The caller's environment leaks in (global skills, `~/.claude/CLAUDE.md` —
  acknowledged in tradeoffs.md).

For a *single* A/B session this is fine — both arms share the drift. But the
project's own ambitions (results.jsonl as run history, trend analysis,
regression detection) presume comparisons *across time*, where drift silently
confounds every delta. The run record does not currently pin what it measured
against: agent CLI version, resolved model, and an environment fingerprint
(were global skills present?) belong in every `results.jsonl` record, not just
in the cache key. Without them, the history database can describe the past but
cannot support the trend conclusions TODO.md plans to build on it. This
reframes the audit's "record agent version" item from housekeeping to a
validity requirement.

The same logic argues for elevating the opt-in strict-isolation runner
(tradeoffs.md's "direction") on the roadmap: reproducible comparisons are the
product, and today's default environment is not reproducible across machines —
which matters the moment scenario packs are shared.

### 3. The judge is absent exactly where it would earn its keep

The judge's concept — encoding the tool author's *intent* about how the tool
should be used — is the right idea, and it is the only layer that can answer
"why." But by design it runs only when all gates pass (cost guard). The runs a
tool author most needs interpreted are the *failures*: what confused the
agent, where the docs misled it, which error message sent it into a loop. Today
those runs get raw transcripts and metrics only, and the human does the
forensics.

A "diagnostic judge" mode for failed runs — different prompt ("diagnose why
the agent failed and what the tool could have said to prevent it"), same
machinery — would likely be the single highest-value output the tool can
produce for its primary audience. It converts the most expensive, most
frustrating runs (failed *and* paid for) into the most actionable artifact.
The evaluation.md note that judge-on-gate-failure "may become configurable"
undersells it: it is not a config toggle, it is a distinct product feature.

Secondary judge tensions worth resolving in the same pass: the judge is itself
n=1 and non-deterministic (its `confidence` field is parsed and dropped, per
the audit); and defaulting the judge tool to a specific harness (`opencode`)
quietly violates the docs' own "not the same model being tested" advice
whenever that harness is the subject.

### 4. Three audiences, one killer loop

The stated audiences are CLI authors, guidance/AGENTS.md authors, and
model/harness comparators. These pull in different directions:

- The **guidance-author loop** (edit AGENTS.md → run → compare friction) is the
  most complete story in the docs, the cheapest to run, the most frequent in
  practice, and — per the ecosystem scan — served by no one else. GitHub's own
  2,500-repo AGENTS.md analysis produced *correlational* advice; ax-eval is the
  only tool positioned to make guidance editing *experimental*.
- The **model/harness comparison** audience is the one most exposed to tensions
  1 and 2 (it needs variance and version pinning most) and is partially served
  by generic eval platforms.
- The **CLI-author** audience is the founding vision and remains sound, but
  its loop is slower (changing a CLI is costlier than changing a doc).

Recommendation: market and polish the guidance loop as the wedge — it is the
shortest path to a user experiencing the tool's value — while the CLI/MCP
ergonomics story remains the long-term identity. Concretely this means the
guidance workflow gets `compare` first, gets the diagnostic judge first, and
headlines the README.

### 5. The loop is open where the ecosystem wants it closed

ax-eval produces profiles; a human (or increasingly, an agent) does the
analyze-modify-rerun steps. The README's agent-driven paragraph — paste a
prompt, let the coding agent run the whole evaluate-modify-repeat loop — is
strategically the most important paragraph in the project, because the realistic
operator of this tool at scale is an agent improving a tool's AX autonomously.

That implies a priority ordering for the interface work: machine-stable output
schemas (`schema_version`, the audit's structured judge-skip reason), a
diff-friendly `compare` output, and JUnit/CI emitters matter *more* than
additional metrics, because they are what an orchestrating agent or CI system
consumes. ADR-0003 already commits to agent-usable help; the same standard
applied to *outputs* ("every artifact parseable without reading source") would
complete the contract. The audit's data-safety findings (redaction, path
sanitization) also land here: artifacts designed to be handed to agents and
teammates must be safe to hand over by construction.

### 6. MCP support is defense; CLI-vs-MCP comparison is offense

ADR-0005 treats MCP as a second target kind to keep up with where tool
surfaces are going — necessary, and the `(action, outcome)` projection is the
right abstraction. But it undersells the opportunity: the live debate in 2026
agent-tooling circles is precisely *"well-designed CLI vs. MCP server — which
gives agents better results per token?"* (AXI's headline claim is that
agent-native CLIs beat MCP on accuracy and token cost.) ax-eval, post-Stage 5,
will be nearly the only neutral instrument that can run the same task against
the same tool exposed both ways and publish the profile. That comparison is a
marquee use case — worth an example fixture pair and a doc — not merely a
schema generalization. It would also be the strongest possible demonstration
of the framework's comparison methodology (and, per tension 1, demands the
variance work to be credible).

## Role in the AI-tool development lifecycle

Where the system fits today and plausibly next:

1. **Now — inner-loop instrument.** A developer (or their agent) iterating on
   a CLI's help text, error messages, or AGENTS.md, running tens of cheap
   scenarios locally. The consent gate, dry-run, validate, and template
   commands serve this well. This is the loop to perfect.
2. **Near — CI guardrail.** Token-budget and cost gates on a schedule or per
   release, catching "the new model/harness version tripled our token spend."
   evaluation.md already endorses this; JUnit output (TODO) and record-level
   version pinning (tension 2) are the prerequisites.
3. **Later — shared benchmark packs.** Scenario packs shipped alongside CLIs
   the way test suites are, letting a tool advertise (and prove) its AX across
   harnesses. This is where discovery-generated scenarios, the sandboxed
   runner, data-safety hardening, and schema versioning converge — and where
   the trust bar is highest.

The maturity gradient is real: each stage inherits the previous stage's
validity requirements. The project should not promote stage-3 workflows
(shared packs) before stage-1 validity (variance, drift pinning) is solid.

## Recommendations, ranked

Strategic priorities — deliberately different from the audit's code-level
sequencing, though they share items:

1. **Protect the core claim: variance before new surface area.** `--iterations
   N` with mean/spread, and a `compare` that is honest about noise. Elevate
   from TODO "Future" to next-minor scope. Interim: soften "guarantee" in the
   README.
2. **Pin the measurement context.** Agent CLI version, resolved model, and an
   environment fingerprint in every `results.jsonl` record. Cheap, and a
   precondition for every trend/regression feature planned on top of the
   history.
3. **Ship the diagnostic judge for failed runs.** Highest-leverage new output
   for the primary audience; reuses existing machinery.
4. **Lead with the guidance loop.** Make AGENTS.md iteration the headline
   workflow in README and tutorial; it is the unserved niche with the fastest
   time-to-value.
5. **Treat outputs as an agent-facing API.** schema_version, structured skip
   reasons, diff-friendly compare, JUnit — sequenced with, not after, the MCP
   work. Pair with the audit's redaction/path-safety fixes before encouraging
   artifact sharing.
6. **Build the CLI-vs-MCP head-to-head as a first-class story** once MCP
   Stage 5 lands: example fixture pair, doc, and a published sample comparison.

## Bottom line

The intent is right, early, and increasingly well-timed: the ecosystem has
converged on the vocabulary (AX, AGENTS.md, skills, MCP) this tool was built
to measure, and no incumbent occupies the tool-side evaluation niche. The
architecture and documented reasoning are unusually disciplined for the
project's size. The strategic risk is not direction but *epistemics*: the tool
currently invites conclusions its single-run methodology cannot support. Close
that gap — variance, context pinning, honest comparison — and ax-eval is
positioned to become the reference instrument for a question the whole
ecosystem is starting to ask.
