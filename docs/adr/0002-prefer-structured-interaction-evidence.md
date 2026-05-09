# Prefer Structured Interaction Evidence

Interaction profiles are built from structured tool-call evidence when an
adapter can provide it. Transcript regex analysis remains only as fallback
evidence for adapters that cannot expose structured tool calls.

We chose this because the interaction profile is the framework's primary
comparative signal. Structured evidence preserves command text and exit status
as adapter-owned facts, while regex analysis infers those facts from human
transcript text. Inference is useful for compatibility, but it is too fragile to
be the canonical path for adapters that already receive structured CLI output.

Adapters expose whether they support structured tool calls through their
`ToolAdapter` interface. If a structured-capable adapter returns transcript
regex evidence, or returns no usable target-tool events for a normal completed
run, evaluation fails instead of silently falling back to regex. This keeps
adapter breakage visible and avoids misleading interaction profiles.

This decision preserves ADR-0001: adapter-specific raw-output parsing still
lives beside each adapter. The shared interaction profile module validates and
scores canonical evidence; it does not parse tool-specific raw event schemas.
