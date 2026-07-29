#!/usr/bin/env bash
# Dual-run / fixture verifier for clean rule candidates (Phase 4c+).
#
# Always runs in-process fixture checks (`verify_clean_fixtures`).
# Optional mole↔vole dual-run requires disposable VOLE_TEST_ROOT (design §7.0).
# Do not point VOLE_TEST_ROOT at your real HOME.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../" && pwd)"
cd "$ROOT"

echo "==> verify clean fixtures against loaded rules (vole-core)"
cargo test -p vole-core verify_clean_fixtures -- --nocapture

FIXTURE_DIR="$ROOT/tests/fixtures/clean"
ALLOWLIST="${VOLE_DUAL_RUN_ALLOWLIST:-$FIXTURE_DIR/dual_run_allowlist.txt}"
if [[ "$ALLOWLIST" != /* ]]; then
  ALLOWLIST="$ROOT/$ALLOWLIST"
fi
if [[ ! -d "$FIXTURE_DIR" ]]; then
  echo "missing fixture dir: $FIXTURE_DIR" >&2
  exit 1
fi

if [[ -z "${VOLE_TEST_ROOT:-}" ]]; then
  echo "SKIP: VOLE_TEST_ROOT unset — conformance dual-run not executed"
  echo "      export VOLE_TEST_ROOT=\$(mktemp -d) for mole↔vole diff on allowlist"
  echo "verify-clean-candidates: OK (rules + fixture plan checks only)"
  exit 0
fi

REAL_HOME="${HOME:-}"
if [[ -z "$REAL_HOME" ]]; then
  echo "HOME is unset; cannot validate VOLE_TEST_ROOT safety" >&2
  exit 1
fi

VOLE_TEST_ROOT="$(cd "$VOLE_TEST_ROOT" && pwd)"
REAL_HOME="$(cd "$REAL_HOME" && pwd)"
if [[ "$VOLE_TEST_ROOT" == "$REAL_HOME" ]]; then
  echo "VOLE_TEST_ROOT must not equal HOME ($REAL_HOME)" >&2
  exit 1
fi
export VOLE_TEST_ROOT

if [[ ! -f "$ALLOWLIST" ]]; then
  echo "missing dual-run allowlist: $ALLOWLIST" >&2
  exit 1
fi

cargo build -q -p vole-cli
VOLE_BIN="${VOLE_BIN:-$ROOT/target/debug/vole}"
export VOLE_RULES_DIR="${VOLE_RULES_DIR:-$ROOT/data/rules}"

MOLE_BIN="${MOLE_BIN:-$ROOT/third_party/mole-1.48.1/bin/clean.sh}"
if [[ ! -x "$MOLE_BIN" ]]; then
  echo "SKIP: mole clean.sh not found at $MOLE_BIN"
  echo "verify-clean-candidates: OK (rules + fixture plan checks only)"
  exit 0
fi

echo "==> conformance dual-run under VOLE_TEST_ROOT=$VOLE_TEST_ROOT"
echo "    rules: $VOLE_RULES_DIR"
cargo build -q -p conformance
CONF_BIN="$ROOT/target/debug/conformance"

DUAL_COUNT=0
while IFS= read -r line || [[ -n "$line" ]]; do
  line="${line%%#*}"
  line="$(echo "$line" | tr -d '[:space:]')"
  [[ -z "$line" ]] && continue
  fixture="$FIXTURE_DIR/$line"
  if [[ ! -f "$fixture" ]]; then
    echo "missing allowlisted fixture: $fixture" >&2
    exit 1
  fi
  DUAL_COUNT=$((DUAL_COUNT + 1))
  echo "--- $fixture"
  "$CONF_BIN" \
    --fixture "$fixture" \
    --mole "$MOLE_BIN" \
    --vole "$VOLE_BIN"
done < "$ALLOWLIST"

if [[ "$DUAL_COUNT" -eq 0 ]]; then
  echo "dual-run allowlist is empty: $ALLOWLIST" >&2
  exit 1
fi

echo "verify-clean-candidates: OK ($DUAL_COUNT dual-run fixtures)"
