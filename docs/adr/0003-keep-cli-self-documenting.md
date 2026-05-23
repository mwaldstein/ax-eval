# Keep CLI Self-Documenting

`ax-eval` should be usable from its CLI surface without requiring an agent
or human to inspect source code first. Top-level help, subcommand help, runtime
errors, and printable templates are part of the user-facing documentation
contract.

We chose this because the tool evaluates whether LLM coding agents can use CLI
tools effectively. If this tool itself requires hidden knowledge, stale external
docs, or source-code inspection for routine use, it undermines the evaluation
model it promotes. The CLI must explain what the tool is for, why someone would
use it, how to start, and where to get schema-valid examples.

The practical contract is:

- Running `ax-eval` with no arguments prints useful guidance, not just an
  error.
- Help text explains purpose and common workflows, not only flag names.
- Subcommands with non-obvious prerequisites document those prerequisites in
  help text and runtime errors.
- `ax-eval template <kind>` prints copyable, schema-valid starting points
  for scenarios, config, script gates, and evaluators.
- New user-facing schema or workflow changes update the relevant help text,
  templates, docs, and CLI tests together.

This does not replace the README, user guide, specs, or ADRs. Those documents
remain the durable reference. The CLI should carry enough guidance for an agent
to discover and execute the normal workflow from the terminal.
