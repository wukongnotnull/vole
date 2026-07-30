#!/usr/bin/env bash
# TCC 矩阵（设计 4.1）：未签名 / ad-hoc / Developer ID；终端 + 最小 app bundle。
# 在受保护树下建小型探针目录，避免 analyze 扫整棵 Library。
# Raycast 启动需人工；脚本末尾打印步骤。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# shellcheck source=/dev/null
[[ -f "$ROOT/scripts/signing.env" ]] && source "$ROOT/scripts/signing.env"

IDENTITY="${VOLE_CODESIGN_IDENTITY:-}"
OUT_DIR="${TMPDIR:-/tmp}/vole-tcc-matrix-$$"
mkdir -p "$OUT_DIR/bins"
BIN_DIR="$OUT_DIR/bins"
MARKER="vole-tcc-probe-$$"

cdhash_of() {
  codesign -dv --verbose=4 "$1" 2>&1 | awk -F= '/^CDHash=/{print $2; exit}' || true
}

sign_info() {
  local bin="$1" out
  out=$(codesign -dv --verbose=4 "$bin" 2>&1 || true)
  if echo "$out" | grep -q "code object is not signed at all"; then
    echo "unsigned"
  elif echo "$out" | grep -q "Signature=adhoc"; then
    echo "adhoc"
  elif echo "$out" | grep -q "Developer ID Application"; then
    echo "developer-id"
  else
    echo "unknown"
  fi
}

setup_markers() {
  mkdir -p \
    "$HOME/Library/Caches/$MARKER" \
    "$HOME/Library/Containers/$MARKER" \
    "$HOME/Library/Application Support/$MARKER" \
    "$HOME/Library/Logs/$MARKER" \
    "$HOME/.cache/$MARKER"
  echo ok >"$HOME/Library/Caches/$MARKER/f.txt"
  echo ok >"$HOME/Library/Containers/$MARKER/f.txt"
  echo ok >"$HOME/Library/Application Support/$MARKER/f.txt"
  echo ok >"$HOME/Library/Logs/$MARKER/f.txt"
  echo ok >"$HOME/.cache/$MARKER/f.txt"
}

cleanup_markers() {
  rm -rf \
    "$HOME/Library/Caches/$MARKER" \
    "$HOME/Library/Application Support/$MARKER" \
    "$HOME/Library/Logs/$MARKER" \
    "$HOME/.cache/$MARKER" 2>/dev/null || true
  # Containers may gain containermanagerd metadata the shell cannot delete.
  rm -rf "$HOME/Library/Containers/$MARKER" 2>/dev/null || true
}

probe_paths() {
  local label="$1"
  local bin="$2"
  local path rc
  echo "=== probe [$label] ==="
  echo "    bin:      $bin"
  echo "    identity: $(sign_info "$bin")"
  echo "    cdhash:   $(cdhash_of "$bin")"
  for path in \
    "$HOME/Library/Caches/$MARKER" \
    "$HOME/Library/Containers/$MARKER" \
    "$HOME/Library/Application Support/$MARKER" \
    "$HOME/Library/Logs/$MARKER" \
    "$HOME/.cache/$MARKER"
  do
    set +e
    "$bin" analyze --json "$path" >"$OUT_DIR/${label}-$(basename "$(dirname "$path")").json" 2>"$OUT_DIR/${label}-$(basename "$(dirname "$path")").err"
    rc=$?
    set -e
    echo "    analyze …/$(basename "$(dirname "$path")")/$MARKER: exit=$rc"
  done
}

trap cleanup_markers EXIT
setup_markers

echo "==> build debug vole"
cargo build -p vole-cli -q
SRC="$ROOT/target/debug/vole"

cp "$SRC" "$BIN_DIR/vole-unsigned"
codesign --remove-signature "$BIN_DIR/vole-unsigned" 2>/dev/null || true

cp "$SRC" "$BIN_DIR/vole-adhoc"
codesign -s - -f --timestamp=none "$BIN_DIR/vole-adhoc" >/dev/null

if [[ -z "$IDENTITY" ]]; then
  echo "ERROR: VOLE_CODESIGN_IDENTITY unset (source scripts/signing.env)" >&2
  exit 3
fi
cp "$SRC" "$BIN_DIR/vole-devid"
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$BIN_DIR/vole-devid" >/dev/null
codesign --verify "$BIN_DIR/vole-devid" >/dev/null

probe_paths "unsigned-terminal" "$BIN_DIR/vole-unsigned"
probe_paths "adhoc-terminal" "$BIN_DIR/vole-adhoc"
probe_paths "devid-terminal" "$BIN_DIR/vole-devid"

echo "=== Developer ID rebuild CDHash ==="
HASH1=$(cdhash_of "$BIN_DIR/vole-devid")
echo "    before rebuild: $HASH1"
touch crates/vole-cli/src/main.rs
cargo build -p vole-cli -q
cp "$ROOT/target/debug/vole" "$BIN_DIR/vole-devid-rebuilt"
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$BIN_DIR/vole-devid-rebuilt" >/dev/null
HASH2=$(cdhash_of "$BIN_DIR/vole-devid-rebuilt")
echo "    after rebuild:  $HASH2"
if [[ -n "$HASH1" && -n "$HASH2" && "$HASH1" != "$HASH2" ]]; then
  echo "    result: CDHash CHANGED after rebuild+resign (TCC may treat as new program)"
else
  echo "    result: CDHash unchanged or missing (investigate)"
fi
cp "$BIN_DIR/vole-devid-rebuilt" "$BIN_DIR/vole-devid-resign"
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$BIN_DIR/vole-devid-resign" >/dev/null
HASH3=$(cdhash_of "$BIN_DIR/vole-devid-resign")
echo "    resign same bytes: $HASH3 ($([[ "$HASH2" == "$HASH3" ]] && echo SAME || echo DIFFERENT))"

echo "=== app-bundle spawn (minimal stub) ==="
APP="$OUT_DIR/VoleTccProbe.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cat >"$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>vole</string>
  <key>CFBundleIdentifier</key><string>com.wukongnotnull.vole.tccprobe</string>
  <key>CFBundleName</key><string>VoleTccProbe</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.0.1</string>
</dict>
</plist>
PLIST
cp "$BIN_DIR/vole-devid" "$APP/Contents/MacOS/vole"
codesign --force --options runtime --timestamp --sign "$IDENTITY" --deep "$APP" >/dev/null
probe_paths "devid-app-bundle" "$APP/Contents/MacOS/vole"

echo
echo "=== manual: Raycast ==="
echo "  Run: $BIN_DIR/vole-devid analyze --json \"\$HOME/Library/Caches/$MARKER\""
echo "  Note TCC prompt target app name; record in findings doc."
echo
echo "Artifacts: $OUT_DIR"
echo "DONE"
