# AGENTS.md - Full Config Fixture

This fixture demonstrates all optional scenario configuration fields.

## Mock Tool: config_tool

- `config_tool write <text>` - Write text to output.txt
- `config_tool read` - Read output.txt

## Config Fields Used

- `tier: 1`
- `tags: [smoke, config-demo, full-schema]`
- `tool_matrix` with mock and opencode tools
- `run.timeout_secs: 300`
