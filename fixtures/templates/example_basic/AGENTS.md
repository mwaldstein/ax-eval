# AGENTS.md - Example Basic Fixture

This directory contains a demonstration scenario for the llm-tool-test framework.

## Files

- `taskmgr` - Mock CLI tool that simulates a task manager
- `scripts/` - Contains helper scripts for the scenario

## Mock Tool: taskmgr

The `taskmgr` script is a simple shell-based mock CLI that:
- Creates task files in the current directory
- Supports `taskmgr create <task-name>` command
- Writes tasks in a simple text format

## Scripts

### Post Scripts

- `scripts/validate_task.sh` - Runs after agent execution to validate the created task

### Script Gates

- `scripts/check_task_quality.sh` - Evaluates task quality as a gate

### Evaluators

- `scripts/assess_completion.sh` - Provides a quality score for task completion

## Testing

This fixture is used by `fixtures/example_basic.yaml` to demonstrate the complete scripts system.
