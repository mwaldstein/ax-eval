# ADR-0007: Separate Tool Reuse From Retry Recovery

**Status: Proposed**

## Context

Interaction metrics originally reported `retry_count` as:

```text
total target-tool calls - unique action names
```

For MCP targets, action identity is currently `(server, tool)`. Arguments are
not part of the metric identity, so retrieval-heavy workflows can look like
retry-heavy workflows. For example, three successful `search` calls with three
different queries are repeated use of one tool, not retries.

This is also true for successful exact repeats: reading the same resource twice
can be intentional verification, polling, or state refresh. The framework should
record objective interaction facts and leave redundancy judgments to a rubric or
human review unless stronger context is available.

## Decision

The immediate metric is named `tool_reuse_count`, not `retry_count`.

`tool_reuse_count` means:

```text
total target-tool calls - unique action names
```

It is a tool-diversity and repetition signal. It is not a recovery-retry signal.

Because this is a breaking metrics-schema correction, old `retry_count` output is
not retained.

## Recommended Future Design

Add a richer action model for structured evidence:

- `tool_key`: the coarse action identity. For MCP, `(server, tool)`. For CLI,
  the normalized target command or subcommand identity currently used by the
  interaction profile.
- `invocation_fingerprint`: the exact action identity. For MCP, `(server, tool,
  canonicalized_arguments)`. For CLI, use normalized argv when adapters can
  provide it; otherwise fall back to the current command identity.
- `outcome`: success, failure, or unknown.

Canonicalized MCP arguments should be produced by parsing the argument JSON as
structured data and serializing it with deterministic object-key ordering. Avoid
string manipulation of raw JSON. Arrays remain order-sensitive.

Once fingerprints exist, report these metrics:

| Metric | Definition |
|--------|------------|
| `tool_reuse_count` | Calls beyond the first call for each `tool_key`. |
| `distinct_action_count` | Count of unique `invocation_fingerprint` values. |
| `exact_repeat_count` | Calls whose full `invocation_fingerprint` has appeared before. |
| `recovery_retry_count` | Calls that immediately follow a failed call to the same `tool_key`. |
| `recovery_exact_retry_count` | Recovery retries with the same `invocation_fingerprint` as the failed call. |
| `recovery_adjusted_retry_count` | Recovery retries to the same `tool_key` with a different `invocation_fingerprint`. |
| `first_action_success_rate` | First invocation successes divided by `distinct_action_count`. |

Do not automatically label `exact_repeat_count` as redundant. Redundancy is a
quality judgment that depends on task context.

## Acceptance Cases

| Calls | Expected |
|-------|----------|
| Three `search` calls with different queries | `tool_reuse_count = 2`, `exact_repeat_count = 0`, recovery retries = 0 |
| Three `get` calls with different IDs | `tool_reuse_count = 2`, `exact_repeat_count = 0`, recovery retries = 0 |
| Failed `search(q1)`, then successful `search(q1)` | `tool_reuse_count = 1`, `exact_repeat_count = 1`, `recovery_retry_count = 1`, `recovery_exact_retry_count = 1` |
| Failed `search(q1)`, then successful `search(q2)` | `tool_reuse_count = 1`, `exact_repeat_count = 0`, `recovery_retry_count = 1`, `recovery_adjusted_retry_count = 1` |
| Successful `search(q1)` repeated identically | `tool_reuse_count = 1`, `exact_repeat_count = 1`; redundancy undetermined |

## Consequences

- `tool_reuse_count` remains comparable with historical `retry_count` values by
  formula, but not by name or interpretation.
- Recovery-oriented metrics require access to ordered structured events and
  argument fingerprints. They should be implemented only for evidence sources
  that can support them accurately.
- Composite scoring should move from `first_try_success_rate` to
  `first_action_success_rate` when the richer action model lands, because the
  current denominator is call-based while the numerator is first-action-based.
