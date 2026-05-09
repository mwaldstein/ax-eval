# Adapter Organization

Adapters launch LLM coding-agent CLIs such as opencode, Codex, and Claude Code.
They do not invoke the target CLI directly; the agent does that inside the
scenario workspace.

## Layout

Runtime adapters use a directory module:

```text
src/adapter/<name>/
├── mod.rs        # Tool invocation and ToolAdapter implementation
└── normalize.rs  # Raw output -> ToolRunOutput
```

Keep adapter-specific raw-output parsing next to the adapter invocation code.
The raw event schema is part of that adapter's interface with its CLI.

Shared canonical construction helpers live in `src/adapter/normalize.rs`.
Keep tool-specific event schemas out of the shared module.

The `mock` adapter is internal test support for adapter plumbing. It remains a
single file because it does not wrap a real LLM CLI or parse real tool events.

## Responsibilities

Each runtime adapter owns:

- Availability and authentication checks for its CLI.
- Process invocation, arguments, model selection, timeout, and environment.
- Raw output capture.
- Tool event normalization from raw output into `ToolRunOutput`.
- Cost and token usage parsing when the CLI exposes those values.

The normalizer in `src/adapter/<name>/normalize.rs` should return canonical run
artifacts:

- Transcript text for human review and fallback analysis.
- Command events for interaction metrics.
- Token usage, if available.
- Cost, if available.
- Exit status.

Adapters that can expose structured tool calls must return structured command
events and set `supports_structured_tool_calls()` to `true` on their
`ToolAdapter` implementation. For normal completed runs, those events must
include at least one usable target-tool command. Evaluation fails when a
structured-capable adapter falls back to transcript regex evidence or returns no
usable target-tool events.

Transcript regex analysis is only fallback evidence for adapters that cannot
provide structured tool calls. See `docs/adr/0002-prefer-structured-interaction-evidence.md`.

## Adding An Adapter

1. Create `src/adapter/<name>/mod.rs` and implement `ToolAdapter`.
2. Create `src/adapter/<name>/normalize.rs` for raw output -> `ToolRunOutput`.
3. Set `supports_structured_tool_calls()` to `true` if the CLI exposes tool
   calls in structured output.
4. Use shared helpers from `src/adapter/normalize.rs` for canonical construction.
5. Register the module in `src/adapter/mod.rs`.
6. Add the adapter to dispatch in `src/run/execution.rs`.
7. Add normalization behavior tests in `src/adapter/normalize.rs` or adapter-local tests.
8. Add integration test coverage in `tests/cli.rs` where relevant.
9. Add e2e coverage in `tests/e2e.rs` if the tool supports real-LLM execution.

## Testing

Prefer behavior tests through the adapter normalizer interface:

```rust
let output = codex::normalize::normalize(raw_output, exit_code);
let command_events = output.command_events().expect("structured command events");
```

Tests should assert canonical behavior, not private parser function names.
