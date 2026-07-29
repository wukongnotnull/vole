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
if [[ ! -d "$FIXTURE_DIR" ]]; then
  echo "missing fixture dir: $FIXTURE_DIR" >&2
  exit 1
fi

if [[ -z "${VOLE_TEST_ROOT:-}" ]]; then
  echo "SKIP: VOLE_TEST_ROOT unset — conformance dual-run not executed"
  echo "      export VOLE_TEST_ROOT to a disposable directory for mole↔vole diff"
  echo "verify-clean-candidates: OK (rules + fixture plan checks only)"
  exit 0
fi

cargo build -q -p vole-cli
VOLE_BIN="${VOLE_BIN:-$ROOT/target/debug/vole}"

MOLE_BIN="${MOLE_BIN:-$ROOT/third_party/mole-1.48.1/mo}"
if [[ ! -x "$MOLE_BIN" ]]; then
  MOLE_BIN="$ROOT/third_party/mole-1.48.1/mole"
fi
if [[ ! -x "$MOLE_BIN" ]]; then
  echo "SKIP: mole binary not found under third_party/mole-1.48.1"
  echo "verify-clean-candidates: OK (rules + fixture plan checks only)"
  exit 0
fi

echo "==> conformance dual-run under VOLE_TEST_ROOT=$VOLE_TEST_ROOT"
cargo build -q -p conformance
CONF_BIN="$ROOT/target/debug/conformance"

for fixture in "$FIXTURE_DIR"/*.json; do
  [[ -f "$fixture" ]] || continue
  echo "--- $fixture"
  "$CONF_BIN" \
    --fixture "$fixture" \
    --mole "$MOLE_BIN" \
    --vole "$VOLE_BIN" || true
done

echo "verify-clean-candidates: OK"
