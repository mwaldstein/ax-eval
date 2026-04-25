#!/bin/bash
# assess_completion.sh - Custom evaluator to assess task completion quality
# This script provides a quality score for the task completion

set -e

TASK_FILE="${LLM_TOOL_TEST_FIXTURE_DIR}/tasks.txt"

# Check if task file exists
if [ ! -f "$TASK_FILE" ]; then
    echo '{"score": 0.0, "summary": "No task file found", "metrics": {"task_count": 0}}'
    exit 0
fi

# Count tasks
TASK_COUNT=$(grep -c "CREATED:" "$TASK_FILE" || echo "0")

# Calculate score based on task count
# For this demo, having at least 1 task gives a perfect score
if [ "$TASK_COUNT" -ge 1 ]; then
    SCORE="1.0"
    SUMMARY="Task created successfully"
else
    SCORE="0.0"
    SUMMARY="No tasks found"
fi

echo "{\"score\": $SCORE, \"summary\": \"$SUMMARY\", \"metrics\": {\"task_count\": $TASK_COUNT}}"
