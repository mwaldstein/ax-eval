#!/bin/bash
# check_json_gate.sh - Script gate that returns structured JSON output

DATA_FILE="${LLM_TOOL_TEST_FIXTURE_DIR:-.}/data.txt"

if [ ! -f "$DATA_FILE" ]; then
    echo '{"passed": false, "message": "data.txt not found"}'
    exit 1
fi

COUNT=$(grep -c "item-" "$DATA_FILE" || echo "0")

if [ "$COUNT" -ge 1 ]; then
    echo "{\"passed\": true, \"message\": \"Found $COUNT item(s) in data.txt\"}"
    exit 0
else
    echo '{"passed": false, "message": "No items found in data.txt"}'
    exit 1
fi
