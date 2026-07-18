# Example MCP Stdio Fixture

This fixture contains a tiny stdio MCP server implemented with only the Python
standard library. It exposes two tools:

- `add_note`
- `list_notes`

The scenario asks the agent to use those MCP tools and then verifies the
persisted notes with `scripts/probe_notes.py`.
