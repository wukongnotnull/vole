#!/usr/bin/env bash
# Local macOS verification — no Developer ID required.
# Mirrors .github/workflows/ci.yml guardrails plus release build, install.sh smoke,
# and subsystem verify scripts. Safe to run on a dev machine (no apply/delete).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

step() {
  echo ""
  echo "==> $*"
}

step "license and attribution"
./scripts/check-license.sh

step "crate dependency direction"
./scripts/check-dep-direction.sh

step "protocol doc"
./scripts/check-protocol-doc.sh

step "cargo fmt"
cargo fmt --all -- --check

step "cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

step "cargo test (workspace)"
cargo test --workspace

if [[ "${VERIFY_LOCAL_SKIP_CROSS:-0}" != "1" ]]; then
  step "cross-compile aarch64-apple-darwin"
  rustup target add aarch64-apple-darwin >/dev/null 2>&1 || true
  cargo build --workspace --target aarch64-apple-darwin

  step "cross-compile x86_64-apple-darwin"
  rustup target add x86_64-apple-darwin >/dev/null 2>&1 || true
  cargo build --workspace --target x86_64-apple-darwin
else
  echo "SKIP: cross-compile (VERIFY_LOCAL_SKIP_CROSS=1)"
fi

step "release build (vole-cli)"
cargo build -p vole-cli --release

INSTALL_PREFIX="${VERIFY_LOCAL_INSTALL_PREFIX:-$(mktemp -d 2>/dev/null || mktemp -d -t vole-local-install)}"
step "install.sh smoke → $INSTALL_PREFIX"
PREFIX="$INSTALL_PREFIX" ./install.sh
"$INSTALL_PREFIX/bin/vole" --version

VOLE_BIN="$INSTALL_PREFIX/bin/vole"

step "clean plan JSON smoke (coverage_note)"
if ! "$VOLE_BIN" clean --json --plan 2>/dev/null | python3 -c "
import json, sys
p = json.load(sys.stdin)
assert p.get('coverage_note'), 'missing coverage_note'
print('  candidates=%d coverage_note=OK' % len(p.get('entries', [])))
"; then
  echo "FAIL: clean --json --plan smoke" >&2
  exit 1
fi

step "verify-clean-candidates.sh"
bash scripts/verify-clean-candidates.sh

step "verify-analyze-json.sh"
bash scripts/verify-analyze-json.sh

step "verify-status-json.sh"
bash scripts/verify-status-json.sh

step "verify-history-mole.sh"
bash scripts/verify-history-mole.sh

step "inventory-mole-rules.py"
python3 scripts/inventory-mole-rules.py >/dev/null

echo ""
echo "verify-local: OK"
echo "  release binary: $ROOT/target/release/vole"
echo "  smoke install:  $INSTALL_PREFIX/bin/vole"
if [[ -z "${VERIFY_LOCAL_INSTALL_PREFIX:-}" ]]; then
  echo "  (temp install dir; set VERIFY_LOCAL_INSTALL_PREFIX to keep)"
fi
