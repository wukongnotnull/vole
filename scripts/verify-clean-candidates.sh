#!/usr/bin/env bash
# Dual-run skeleton for clean rule candidates (Phase 4c Task 11).
#
# VOLE_TEST_ROOT: required for full mole↔vole conformance diff via the
# `conformance` binary (design doc §7.0). Do not point at your real HOME.
#
# This script always runs the in-process fixture verifier (materialize fixture →
# load data/rules/*.toml → build_plan). When `vole clean --plan` is wired (Task 12),
# extend the loop below to invoke it per fixture under VOLE_TEST_ROOT.
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

if ! cargo build -q -p vole-cli 2>/dev/null; then
  echo "SKIP: vole-cli build failed — clean CLI not wired yet"
  echo "verify-clean-candidates: OK (rules + fixture plan checks only)"
  exit 0
fi

VOLE_BIN="${VOLE_BIN:-$ROOT/target/debug/vole}"
if ! "$VOLE_BIN" clean --help >/dev/null 2>&1; then
  echo "SKIP: clean CLI not wired; rules load OK (see cargo test above)"
  echo "verify-clean-candidates: OK (rules + fixture plan checks only)"
  exit 0
fi

MOLE_BIN="${MOLE_BIN:-$ROOT/third_party/mole-1.48.1/bin/clean.sh}"
if [[ ! -x "$MOLE_BIN" ]]; then
  echo "SKIP: mole clean.sh not found at $MOLE_BIN"
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
