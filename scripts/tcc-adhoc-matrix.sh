#!/usr/bin/env bash
# ad-hoc TCC 探测：无 Developer ID 时可跑的最小子集。
set -euo pipefail
REPO=$(cd "$(dirname "$0")/.." && pwd)
cd "$REPO"

echo "=== build vole-cli ==="
cargo build -p vole-cli -q
codesign -s - -f target/debug/vole 2>/dev/null || true

echo "=== probe: read Containers ==="
ls "$HOME/Library/Containers" >/dev/null 2>&1
echo "containers: $?"

echo "=== probe: read Caches ==="
ls "$HOME/Library/Caches" >/dev/null 2>&1
echo "caches: $?"

echo "=== probe: recompile cdhash ==="
touch crates/vole-cli/src/main.rs
cargo build -q -p vole-cli
codesign -s - -f target/debug/vole 2>/dev/null || true
codesign -dv target/debug/vole 2>&1 | grep -i CDHash | head -1 || echo "no-cdhash-line"
