# Result Caching Is Opt-In

`ax-eval` runs fresh by default. The result cache — which skips agent
execution when a prior run used the same scenario, prompt, fixtures, tool,
and model — is opt-in via `--cache`. Every completed run is appended to the
results database (`results.jsonl`) regardless of cache settings.

We chose this because `ax-eval` evaluates a non-deterministic system. An
LLM coding agent given the same prompt and fixtures will produce different
behavior on each run. A cache hit presents a single past sample as if it
represents current agent capability, which is misleading. No comparable
tool caches evaluation results to skip re-runs: benchmark tools (Criterion,
pytest-benchmark) always re-run and store history for comparison; LLM eval
harnesses (lm-eval-harness, Promptfoo) cache at the API-call layer, never
at the evaluation layer; e2e tools (Cypress, Playwright) never cache
results. The closest precedent (DeepEval) writes results by default but
only reads cache on explicit opt-in — the asymmetric pattern we adopt here.

The practical contract is:

- Default: every run executes the agent, appends to the results database,
  and writes per-run artifacts. The cache is not read or written.
- `--cache`: the cache is read before execution (short-circuit on hit) and
  written after completion. Use this when iterating on gates, evaluators,
  or rubrics and you want to avoid re-spending LLM credits on unchanged
  agent tasks.
- The results database (`results.jsonl`) is the source of truth for run
  history. It is append-only and captures every executed run. The cache is
  a deduplication layer for development speed, not a persistence mechanism.
- `ax-eval clean` removes cache entries. It does not touch the results
  database.

The expensive-runs problem (LLM API cost, minutes per run) is real but
orthogonal to caching. It is addressed by the tier system, selective runs
(tags, specific scenarios), dry-run for setup and gate validation, and
future support for N-run aggregation and artifact re-evaluation against
updated gates without re-invoking the agent.
