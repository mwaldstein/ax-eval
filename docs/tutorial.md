# Tutorial: Evaluate Your CLI in 10 Minutes

This tutorial walks you through your first ax-eval evaluation from scratch. You will:

1. Run a built-in example to see what evaluation looks like.
2. Create a mock CLI tool.
3. Write a scenario that evaluates it.
4. Run the evaluation and read the results.

No LLM API calls are needed for this tutorial — we use `--dry-run` throughout.

## Prerequisites

```bash
cargo build
```

## Step 1: See What Evaluation Looks Like

Run the built-in `example_basic` scenario in dry-run mode:

```bash
ax-eval run --scenario example_basic --dry-run
```

This validates scenario loading, fixture setup, and run planning without launching an agent. You should see output like:

```
Loaded scenario: example_basic
Dry run: scenario validated, no agent launched
```

Now list all available scenarios:

```bash
ax-eval scenarios
```

You will see the built-in examples. These are your reference for authoring your own.

## Step 2: Create a Mock CLI

Create a simple CLI tool that we will evaluate. This tutorial uses a `todo` CLI — a minimal task manager.

```bash
mkdir -p ax-eval-fixtures/todo_tutorial
```

Create the mock CLI at `ax-eval-fixtures/todo_tutorial/todo`:

```bash
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  add)
    title="${2:?Usage: todo add <title>}"
    id=$(date +%s)
    echo "$id $title" >> .todo-store
    echo "Created task $id: $title"
    ;;
  list)
    if [ -f .todo-store ]; then
      cat .todo-store
    else
      echo "No tasks"
    fi
    ;;
  done)
    id="${2:?Usage: todo done <id>}"
    if [ -f .todo-store ]; then
      sed -i "s/^$id /DONE $id /" .todo-store
      echo "Marked task $id as done"
    fi
    ;;
  --help|help)
    echo "todo - a minimal task manager"
    echo ""
    echo "Usage:"
    echo "  todo add <title>    Create a new task"
    echo "  todo list           List all tasks"
    echo "  todo done <id>      Mark a task as done"
    echo "  todo help           Show this help"
    ;;
  *)
    echo "Unknown command: ${1:-}" >&2
    echo "Run 'todo help' for usage" >&2
    exit 1
    ;;
esac
```

```bash
chmod +x ax-eval-fixtures/todo_tutorial/todo
```

## Step 3: Write AGENTS.md

The AGENTS.md file is the primary documentation the agent receives about your tool. This is what you are testing.

Create `ax-eval-fixtures/todo_tutorial/AGENTS.md`:

```markdown
# Todo CLI (todo)

A minimal task manager. Tasks are stored in `.todo-store` in the current directory.

## Commands

```
todo add <title>    Create a new task. Prints the task ID.
todo list           List all tasks.
todo done <id>      Mark a task as done.
todo help           Show available commands.
```

## Workflow

1. Create tasks: `todo add "Buy groceries"`
2. List tasks: `todo list`
3. Complete a task: `todo done <id>` (use the ID printed when created)

## Notes

- Capture the ID when you create a task — you need it for `todo done`.
- Tasks are stored in `.todo-store` as plain text.
```

## Step 4: Write the Scenario

Create `ax-eval-fixtures/todo_tutorial.yaml`:

```yaml
name: todo_tutorial
description: Verify an agent can use the todo CLI to create and complete tasks
tags:
  - tutorial
tier: 0
template_folder: todo_tutorial

target:
  binary: ./todo

task:
  prompt: |
    Use the `todo` CLI to manage tasks for a project launch.

    1. Create a task: "Write launch announcement"
    2. Create a task: "Prepare demo slides"
    3. Create a task: "Send calendar invite"
    4. Mark "Write launch announcement" as done
    5. List all tasks to verify

    Read AGENTS.md first. Use `todo help` if you need to see available commands.

interaction:
  target_commands: required

evaluation:
  gates:
    - type: file_exists
      path: .todo-store
    - type: command_output_contains
      command: "./todo list"
      substring: "Write launch announcement"
    - type: command_output_contains
      command: "./todo list"
      substring: "Prepare demo slides"
    - type: command_output_contains
      command: "./todo list"
      substring: "Send calendar invite"
    - type: command_output_contains
      command: "./todo list"
      substring: "DONE"

  judge:
    enabled: false
    pass_threshold: 0.70

run:
  timeout_secs: 300
```

Key decisions in this scenario:

- **`target.binary: ./todo`** — the tool is part of the fixture, so we reference it by relative path. No PATH manipulation needed.
- **Gates verify outcomes** — the store file exists, all three tasks appear, and one is marked DONE. We do not assert exact command sequences.
- **Judge disabled** — for a first run, interaction metrics are the primary signal.

## Step 5: Validate the Scenario

Check the scenario YAML for schema errors before running:

```bash
ax-eval validate --scenario ax-eval-fixtures/todo_tutorial.yaml
```

You should see:

```
  ✓ todo_tutorial

Validated 1 scenario(s), 0 warning(s)
```

If there are YAML errors, missing fields, or misconfigured gates, they will
appear here with helpful messages (including typo suggestions for gate types).
Fix them before proceeding.

You can also use `--dry-run` to validate scenario loading, fixture setup, and
run planning without launching an agent:

```bash
ax-eval run --scenario ax-eval-fixtures/todo_tutorial.yaml --dry-run
```

You should see:

```
Loaded scenario: todo_tutorial
Dry run: scenario validated, no agent launched
```

If there are YAML errors, they will appear here. Fix them before proceeding.

## Step 6: Run the Evaluation

When you are ready to run with a real agent:

```bash
export AX_EVAL_ENABLED=1
ax-eval run --scenario ax-eval-fixtures/todo_tutorial.yaml --tool opencode
```

The agent will:
1. Read AGENTS.md
2. Learn the `todo` CLI
3. Create three tasks, mark one done, and list them
4. Exit

ax-eval will then evaluate the interaction and write results.

## Step 7: Read the Results

Results are written to `ax-eval-results/<timestamp>-<tool>-<model>-todo_tutorial/`.

### evaluation.md — the human-readable profile

```bash
cat ax-eval-results/*/todo_tutorial/evaluation.md
```

This shows:
- **Run status** — whether the agent completed and guardrails passed
- **Interaction metrics** — command count, errors, retries, help invocations, first-try success rate
- **Gate results** — which outcome assertions passed or failed
- **Judge score** — if enabled, a qualitative assessment with rationale

### metrics.json — the machine-readable profile

```bash
cat ax-eval-results/*/todo_tutorial/metrics.json
```

Use this for programmatic comparison between runs. Key fields:

```json
{
  "gate_status": "passed",
  "efficiency": {
    "total_commands": 6,
    "unique_commands": 5,
    "error_count": 0,
    "retry_count": 1,
    "help_invocations": 0,
    "first_try_success_rate": 0.83,
    "iteration_ratio": 0.83,
    "completed": true
  },
  "interaction_evidence_source": "structured_tool_calls"
}
```

### transcript.raw.txt — the full agent conversation

```bash
cat ax-eval-results/*/todo_tutorial/artifacts/transcript.raw.txt
```

Read this to understand exactly what the agent did, where it hesitated, and what commands it ran.

## Step 8: Improve and Iterate

Now you have a baseline. Make a change and run again:

**Example: improve AGENTS.md** — add a "Common workflows" section:

```markdown
## Common Workflows

### Create and complete a task
```
$ todo add "Review PR"
Created task 1716400000: Review PR
$ todo done 1716400000
Marked task 1716400000 as done
```
```

Run again:

```bash
export AX_EVAL_ENABLED=1
ax-eval run --scenario ax-eval-fixtures/todo_tutorial.yaml --tool opencode
```

Compare the two `metrics.json` files. Did the richer guidance reduce:
- Error count?
- Retry count?
- Help invocations?
- Total commands?

That is the core loop: **modify → run → compare → refine**.

## Next Steps

- **Add a judge rubric** — enable `judge.enabled: true` and define criteria to assess whether the agent followed the documented workflow.
- **Add custom evaluators** — write a script that scores task quality beyond pass/fail gates.
- **Run a matrix** — compare opencode vs claude-code on the same scenario using `tool_matrix` or a config profile.
- **Discover your CLI** — if you do not have scenarios yet, run `ax-eval discover <your-tool> --tool opencode` to have an agent author them for you.
- **Evaluate an MCP server** — the target can be an MCP server instead of a CLI binary; the task, gates, and judge work the same way. See [MCP targets](mcp-targets.md) and [MCP authentication](mcp-auth.md).
- **Read the full references** — [scenario reference](scenarios.md), [evaluation reference](evaluation.md), [scripts reference](scripts.md).
