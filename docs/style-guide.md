# Documentation Style Guide

This guide covers writing style for ax-eval documentation. System documentation (specs, ADRs) follows its own conventions; this guide applies to user-facing docs: README, user guide, tutorials, and CLI help text.

## Voice

Use second-person imperative. Address the reader directly with action-oriented language.

```
Good: Use --dry-run to validate scenario setup.
Good: Run ax-eval scenarios to list available examples.
Bad: The --dry-run flag can be used to validate scenario setup.
Bad: Users should run ax-eval scenarios.
```

Exceptions:
- **Reference/glossary**: third-person declarative (`CONTEXT.md` defines terms)
- **ADRs**: first-person plural is acceptable ("We chose this because...")

## Tone

Technical, direct, pragmatic. No marketing language, no filler. State what the tool does and how to use it.

```
Good: ax-eval measures how agents discover, learn, and use your CLI.
Bad: ax-eval is a revolutionary new tool that transforms your CLI workflow.
```

## Sentences

Keep sentences short. Prefer 15-30 words. Break long sentences into two.

```
Good: Gates catch catastrophic failures. The evaluation profile is the main signal.
Bad: Gates, which serve as guardrails for catastrophic outcome failures, should be used alongside the richer evaluation profile, which comes from the interaction metrics, transcript, custom evaluator results, and optional judge rubric, to give you a complete picture of how well the agent performed.
```

## Code References

Always use inline backticks for commands, flags, environment variables, file paths, types, and field names.

```
Good: Set AX_EVAL_ENABLED=1 before running.
Good: The --tool flag selects the adapter.
Good: Write the scenario to fixtures/my_scenario.yaml.
Bad: Set AX_EVAL_ENABLED=1 before running.
Bad: The --tool flag selects the adapter.
```

## Code Blocks

Fence code blocks with a language annotation. Never indent content inside a fence.

````
Good:
```bash
ax-eval run --scenario example_basic --tool claude-code
```

Bad:
```bash
    ax-eval run --scenario example_basic --tool claude-code
```
````

Use `bash` for shell commands, `yaml` for scenarios, `toml` for config, `json` for data, `text` for raw output.

## Headings

Use Title Case. Keep headings to 5 words or fewer.

```
Good: ## Scenario Authoring
Good: ## Interpreting Results
Bad: ## How to author a scenario
Bad: ## How to interpret the results and understand what they mean
```

## Cross-References

Use descriptive link text. The reader should know what they're clicking.

```
Good: See the [CLI reference](docs/reference/cli-commands.md) for all commands and flags.
Bad: See the [reference](docs/reference/cli-commands.md).
Bad: See [here](docs/reference/cli-commands.md).
```

## Lists

Numbered lists for ordered workflows (step-by-step procedures). Bulleted lists for unordered collections (options, artifacts, features).

Use bold lead-ins for list items that need a label:

```
- **evaluation.md**: Human-readable profile and summary.
- **metrics.json**: Machine-readable metrics for programmatic comparison.
```

## Design Philosophy

Use "Prefer X over Y" to signal guidance without prescribing. Use "Avoid X" for anti-patterns.

```
Good: Prefer outcome gates over process gates.
Good: Avoid using no_transcript_errors as a unit-test assertion.
Bad: You must use outcome gates.
Bad: Never use no_transcript_errors.
```

## Emoji

Do not use emoji in documentation.

## What to Avoid

- **Passive voice**: "scenarios are run" → "run scenarios"
- **Weasel words**: "very", "quite", "simply", "just"
- **Nested conditionals in prose**: split into steps or a table
- **Redundant sections**: each doc has one identity; link to others rather than repeat
