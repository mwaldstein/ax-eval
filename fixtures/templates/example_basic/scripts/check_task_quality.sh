#!/bin/bash
# check_task_quality.sh - Script gate to check task quality
# This script acts as a gate that passes or fails based on task quality

set -e

TASK_FILE="${LLM_TOOL_TEST_FIXTURE_DIR}/tasks.txt"

# Check if task file exists
if [ ! -f "$TASK_FILE" ]; then
    echo '{"passed": false, "message": "Task file does not exist"}'
    exit 1
fi

# Count tasks
TASK_COUNT=$(grep -c "CREATED:" "$TASK_FILE" || echo "0")

if [ "$TASK_COUNT" -ge 1 ]; then
    echo "{\"passed\": true, \"message\": \"Found $TASK_COUNT task(s)\"}"
    exit 0
else
    echo '{"passed": false, "message": "No tasks found in task file"}'
    exit 1
fi
