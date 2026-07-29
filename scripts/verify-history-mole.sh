#!/usr/bin/env bash
# Compare vole history --json against mole history --json on the same fixture.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOLE_ROOT="${MOLE_ROOT:-$ROOT/third_party/mole-1.48.1}"
HOME_DIR="$(mktemp -d "${TMPDIR:-/tmp}/vole-history-verify.XXXXXX")"
cleanup() { rm -rf "$HOME_DIR"; }
trap cleanup EXIT

export HOME="$HOME_DIR"
mkdir -p "$HOME/Library/Logs/mole"

cat > "$HOME/Library/Logs/mole/operations.log" <<'EOF'
# ========== clean session started at 2026-05-24 10:00:00 ==========
[2026-05-24 10:00:01] [clean] REMOVED /tmp/cache one (2KB)
[2026-05-24 10:00:02] [clean] TRASHED /tmp/Old App.app (4KB)
[2026-05-24 10:00:03] [clean] SKIPPED /tmp/protected (whitelist)
[2026-05-24 10:00:04] [clean] FAILED /tmp/fail (permission denied)
# ========== clean session ended at 2026-05-24 10:00:05, 2 items, 6KB ==========
# ========== purge session started at 2026-05-24 11:00:00 ==========
[2026-05-24 11:00:01] [purge] REMOVED /tmp/build (10KB)
# ========== purge session ended at 2026-05-24 11:00:02, 1 items, 10KB ==========
EOF

printf '2026-05-24T10:00:02+0000\ttrash\t4\tok\t/tmp/Old App.app\n' > "$HOME/Library/Logs/mole/deletions.log"
printf '2026-05-24T11:00:01+0000\tpermanent\t10\tdry-run\t/tmp/build\n' >> "$HOME/Library/Logs/mole/deletions.log"

VOLE_BIN="${VOLE_BIN:-$ROOT/target/debug/vole}"
if [[ ! -x "$VOLE_BIN" ]]; then
  cargo build -q -p vole-cli
  VOLE_BIN="$ROOT/target/debug/vole"
fi
VOLE_JSON="$("$VOLE_BIN" history --json --limit 20)"

MOLE_BIN="${MOLE_BIN:-$MOLE_ROOT/mo}"
if [[ ! -x "$MOLE_BIN" ]]; then
  MOLE_BIN="$MOLE_ROOT/mole"
fi
if [[ ! -x "$MOLE_BIN" ]]; then
  echo "skip: mole binary missing under $MOLE_ROOT" >&2
  exit 0
fi
MOLE_JSON="$(env HOME="$HOME" "$MOLE_BIN" history --json)"

python3 -c '
import json, sys
vole = json.loads(sys.argv[1])
mole = json.loads(sys.argv[2])
assert vole["limit"] == mole["limit"], (vole["limit"], mole["limit"])
assert vole["sessions"] == mole["sessions"], (vole["sessions"], mole["sessions"])
assert vole["deletions"] == mole["deletions"], (vole["deletions"], mole["deletions"])
print("verify-history-mole: sessions/deletions match")
' "$VOLE_JSON" "$MOLE_JSON"
