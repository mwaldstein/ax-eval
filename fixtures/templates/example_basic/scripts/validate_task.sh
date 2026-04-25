#!/bin/bash
# validate_task.sh - Post script to validate task creation
# This script runs after the agent completes and validates the task file

set -e

TASK_FILE="${LLM_TOOL_TEST_FIXTURE_DIR}/tasks.txt"

if [ ! -f "$TASK_FILE" ]; then
    echo "Error: Task file not found at $TASK_FILE"
    exit 1
fi

echo "Post script: Validating task file..."

# Check that the file has the expected format
if grep -q "CREATED:" "$TASK_FILE"; then
    echo "Post script: Task file format is valid"
    echo "Post script: Contents:"
    cat "$TASK_FILE"
    exit 0
else
    echo "Error: Task file does not contain expected format"
    exit 1
fi
