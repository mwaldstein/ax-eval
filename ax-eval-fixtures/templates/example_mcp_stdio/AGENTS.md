# Notes MCP Fixture

The target is an MCP server named `notes`. Use the MCP tools exposed by the
agent host; do not run `notes_mcp_server.py` directly for the task.

## Tools

- `add_note`: create a note. Arguments: `{ "text": "note text" }`.
- `list_notes`: list saved notes. Arguments: `{}`.

## Workflow

1. Call `add_note` once for each requested note.
2. Call `list_notes` to verify the saved notes.

The server persists notes in `notes.json` through the configured `NOTE_STORE`
environment variable. The probe script is for ax-eval gates, not for completing
the user task.
