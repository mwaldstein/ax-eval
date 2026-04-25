#!/bin/bash
# check_data.sh - Script gate checking data integrity via exit code

set -e

if [ ! -f "${LLM_TOOL_TEST_FIXTURE_DIR:-.}/data.txt" ]; then
    echo "Missing data.txt"
    exit 1
fi

if ! grep -q "item-" "${LLM_TOOL_TEST_FIXTURE_DIR:-.}/data.txt"; then
    echo "No items found in data.txt"
    exit 1
fi

echo "Data integrity check passed"
