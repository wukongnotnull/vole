#!/usr/bin/env bash
# 校验 vole analyze --json 结构与 total_size/total_files（有 mo 时与 mole 对比）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../" && pwd)"
cd "$ROOT"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/sub"
dd if=/dev/zero of="$TMP/sub/big.bin" bs=1024 count=10 status=none
echo hi >"$TMP/small.txt"

VOLE_JSON="$(cargo run -q -p vole-cli -- analyze "$TMP" --json)"
echo "$VOLE_JSON" | jq -e '.path and .entries and .total_size and .total_files' >/dev/null

TOTAL_SIZE="$(echo "$VOLE_JSON" | jq '.total_size')"
TOTAL_FILES="$(echo "$VOLE_JSON" | jq '.total_files')"
if [[ "$TOTAL_SIZE" -lt 10240 ]]; then
  echo "total_size too small: $TOTAL_SIZE"
  exit 1
fi
if [[ "$TOTAL_FILES" -lt 2 ]]; then
  echo "total_files too small: $TOTAL_FILES"
  exit 1
fi

if command -v mo >/dev/null 2>&1; then
  MO_JSON="$(mo analyze "$TMP" --json 2>/dev/null || true)"
  if [[ -n "$MO_JSON" ]]; then
    MO_SIZE="$(echo "$MO_JSON" | jq '.total_size')"
    MO_FILES="$(echo "$MO_JSON" | jq '.total_files')"
    if [[ "$TOTAL_SIZE" != "$MO_SIZE" ]]; then
      echo "total_size mismatch vole=$TOTAL_SIZE mole=$MO_SIZE"
      exit 1
    fi
    if [[ "$TOTAL_FILES" != "$MO_FILES" ]]; then
      echo "total_files mismatch vole=$TOTAL_FILES mole=$MO_FILES"
      exit 1
    fi
    echo "mole compare: OK (total_size=$TOTAL_SIZE total_files=$TOTAL_FILES)"
  else
    echo "SKIP mole compare: mo analyze failed"
  fi
else
  echo "SKIP mole compare: mo not in PATH"
fi

echo "verify-analyze-json: OK"
