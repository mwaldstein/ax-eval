# AGENTS.md - Notes Tool (Rich Guidance)

This is the detailed guidance version. It includes examples, workflows, and error handling so the LLM can succeed without trial and error.

## Commands

| Command | Description | Example |
|---------|-------------|---------|
| `notes init` | Initialize the notes database. **Must run this first.** | `notes init` |
| `notes add <title> [--tag <tag>]` | Create a note. Returns a numeric ID. Capture this ID. | `notes add "Design doc" --tag project` |
| `notes show <id>` | Show a note by its numeric ID. | `notes show 1` |
| `notes list [--tag <tag>]` | List all notes, optionally filtered by tag. | `notes list --tag project` |
| `notes search <query> [--content]` | Search note titles. Use `--content` to search body text too. | `notes search "design"` |
| `notes link <from-id> <to-id>` | Link two notes together. **Requires numeric IDs, not titles.** | `notes link 1 2` |
| `notes graph` | Display the note link graph. | `notes graph` |
| `notes export [--format json\|md]` | Export notes as JSON or Markdown. | `notes export --format json` |

## Workflow

1. **Initialize**: Always run `notes init` before any other command.
2. **Add notes**: Use `notes add "Title" --tag <tag>`. The output includes the note ID.
3. **Capture IDs**: Store the IDs returned by `add` — you'll need them for linking.
4. **Link notes**: Use `notes link <from-id> <to-id>` with the numeric IDs.
5. **Search**: Use `notes search "query"`. By default only searches titles. Add `--content` for full-text.
6. **Export**: Use `notes export --format json` for structured output.

## Important Constraints

- `link` requires **numeric IDs**, not titles. If you try to use a title, it will fail.
- `search` only searches **titles** unless you pass `--content`.
- `list` without `--tag` shows all notes.
- All commands except `init` and `help` require the database to be initialized first.

## Error Handling

| Error | Cause | Fix |
|-------|-------|-----|
| "database not initialized" | Forgot `notes init` | Run `notes init` first |
| "Note X not found" | Wrong ID or note deleted | Use `notes list` to find correct ID |
| "two note IDs required" | Missing args to `link` | Provide both IDs: `notes link 1 2` |
| "unknown flag" | Typo in flag name | Check command reference above |

## Tips

- After adding notes, run `notes list` to verify they exist and see their IDs.
- Use `notes graph` after linking to verify the graph structure.
- Export with `--format json` for machine-readable output in gates.
