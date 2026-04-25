# Example Basic Scenario

This is a demonstration scenario showing the llm-tool-test framework's scripts system. It uses a simple shell script acting as a mock CLI tool called `taskmgr`.

## Purpose

This example demonstrates:
- Target tool configuration
- Generic gates (command_succeeds, file_exists, file_contains)
- Post-execution scripts
- Script gates
- Custom evaluators

## Structure

- `taskmgr` - A mock CLI tool (shell script)
- `scripts/` - Contains post scripts, evaluators, and script gates
- `README.md` - This file

## Running the Scenario

```bash
cargo run -- run --scenario example_basic
```

## How It Works

The scenario simulates a simple task management CLI. The LLM agent is asked to use the `taskmgr` tool to create a task. The evaluation verifies:

1. The task file was created (file_exists gate)
2. The task file contains the expected content (file_contains gate)
3. A post script runs to validate the task format
4. A script gate provides additional validation
5. A custom evaluator assesses task quality
