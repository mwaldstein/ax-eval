#!/usr/bin/env bash
# Regenerate CLI help snapshots and docs/reference/cli-commands.md from the compiled binary.
#
# Usage: scripts/generate-cli-reference.sh
#
# This script is the single source of truth for CLI documentation.
# After changing src/cli.rs, run this script and commit the updated fixtures
# and reference doc in the same PR.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BINARY="${REPO_ROOT}/target/debug/ax-eval"
SNAPSHOT_DIR="${REPO_ROOT}/fixtures/cli-help-snapshots"
REFERENCE_DOC="${REPO_ROOT}/docs/reference/cli-commands.md"

if [ ! -x "$BINARY" ]; then
    echo "error: binary not found at $BINARY" >&2
    echo "run 'cargo build' first" >&2
    exit 1
fi

mkdir -p "$SNAPSHOT_DIR"
mkdir -p "$(dirname "$REFERENCE_DOC")"

SUBCOMMANDS=("run" "discover" "scenarios" "show" "clean" "validate" "guidance" "template")

echo "# CLI Command Reference" > "$REFERENCE_DOC"
echo "" >> "$REFERENCE_DOC"
echo "Generated from \`src/cli.rs\`. Do not edit by hand." >> "$REFERENCE_DOC"
echo "Run \`scripts/generate-cli-reference.sh\` to regenerate." >> "$REFERENCE_DOC"
echo "" >> "$REFERENCE_DOC"

# Root help
echo "## ax-eval" >> "$REFERENCE_DOC"
echo "" >> "$REFERENCE_DOC"
echo '```' >> "$REFERENCE_DOC"
"$BINARY" --help >> "$REFERENCE_DOC" 2>&1
echo '```' >> "$REFERENCE_DOC"
echo "" >> "$REFERENCE_DOC"

# Save root snapshot
"$BINARY" --help > "${SNAPSHOT_DIR}/ax-eval.txt" 2>&1

for cmd in "${SUBCOMMANDS[@]}"; do
    echo "## ax-eval ${cmd}" >> "$REFERENCE_DOC"
    echo "" >> "$REFERENCE_DOC"
    echo '```' >> "$REFERENCE_DOC"
    "$BINARY" "$cmd" --help >> "$REFERENCE_DOC" 2>&1
    echo '```' >> "$REFERENCE_DOC"
    echo "" >> "$REFERENCE_DOC"

    # Save snapshot
    "$BINARY" "$cmd" --help > "${SNAPSHOT_DIR}/${cmd}.txt" 2>&1
done

echo "Generated CLI reference: ${REFERENCE_DOC}"
echo "Updated snapshots in: ${SNAPSHOT_DIR}/"
