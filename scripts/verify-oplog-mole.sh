#!/usr/bin/env bash
set -euo pipefail
REPO=$(cd "$(dirname "$0")/.." && pwd)
TMP=$(mktemp -d)
export HOME="$TMP/home"
mkdir -p "$HOME/Library/Logs/vole"

cd "$REPO"
cargo test -p vole-core oplog::tests::mole_verify_fixture -- --ignored --exact

MOLE="$REPO/third_party/mole-1.48.1/mo"
if [[ ! -x "$MOLE" ]]; then
    echo "FAIL: mole mo wrapper missing at $MOLE" >&2
    exit 1
fi

# Vole writes ~/Library/Logs/vole; point Mole at that file to check format compatibility.
json=$(env HOME="$HOME" MOLE_OPERATIONS_LOG="$HOME/Library/Logs/vole/operations.log" MOLE_TEST_NO_AUTH=1 "$MOLE" history --json 2>/dev/null)
if [[ -z "$json" ]]; then
    echo "FAIL: mo history --json returned empty output" >&2
    exit 1
fi

if ! echo "$json" | python3 -c '
import json, sys
data = json.load(sys.stdin)
assert data["sessions"], "sessions empty"
assert data["sessions"][0]["actions"]["removed"] >= 1
'; then
    echo "FAIL: mo history did not parse vole oplog session" >&2
    echo "$json" | head -20
    exit 1
fi

echo "OK: mo history parsed vole oplog (removed >= 1)"
rm -rf "$TMP"
