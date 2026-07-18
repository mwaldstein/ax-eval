# Guidance Testing

The framework includes comparative examples for testing how AGENTS.md guidance
quality affects LLM tool usage.

## Built-In Examples

- `example_guidance_minimal`: bare-bones AGENTS.md with just command names. The
  LLM must discover init requirements, ID capture, linking rules, and search
  behavior through trial and error.
- `example_guidance_rich`: detailed AGENTS.md with examples, workflows, error
  handling tables, and constraints. It should produce better interaction
  metrics.

Both use the same `notes` mock tool, task prompt, and evaluation gates.

## Running The Comparison

Validate your scenarios first to catch schema errors before spending LLM credits:

```bash
ax-eval validate --all
```

Then run the comparison:

```bash
ax-eval run --all --tags guidance-test --tier 1
```

Compare `metrics.json` across result directories. Use `results.jsonl` in the
results directory for run-level token and cost fields when adapters report
them.

Primary signals:

- Lower error rates and help-seeking.
- Higher first-try success rate.
- Fewer total commands.
- Lower token usage and cost per task completion when adapters report those fields.

Gate outcome is a guardrail, not the main signal. It should be identical across
variants; if it differs, the task is miscalibrated. The purpose of this
comparison is to measure interaction quality.

## Running All Examples With A Real LLM

All built-in example scenarios share the `examples` tag.

```bash
ax-eval scenarios --tags examples
ax-eval run --all --tags examples --tool opencode
ax-eval run --all --tags examples --tier 1 --tool opencode --model opencode/kimi-k2.6
```
