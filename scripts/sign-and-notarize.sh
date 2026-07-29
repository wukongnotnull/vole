#!/usr/bin/env bash
# Developer ID codesign + notarize placeholder for vole.
# Without Apple credentials this script exits non-zero with a clear message.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${VOLE_BIN:-$ROOT/target/release/vole}"
IDENTITY="${VOLE_CODESIGN_IDENTITY:-}"
PROFILE="${VOLE_NOTARY_PROFILE:-}"

if [[ ! -x "$BIN" ]]; then
  echo "sign-and-notarize: binary not found at $BIN" >&2
  echo "  Build first: cargo build -p vole-cli --release" >&2
  exit 2
fi

if [[ -z "$IDENTITY" ]]; then
  echo "sign-and-notarize: VOLE_CODESIGN_IDENTITY unset — no Developer ID available." >&2
  echo "  See docs/findings/2026-07-phase5-signing.md" >&2
  echo "  Ad-hoc only (dev): codesign -s - --force --timestamp=none \"$BIN\"" >&2
  exit 3
fi

echo "sign-and-notarize: codesign with identity: $IDENTITY"
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$BIN"
codesign --verify --verbose=2 "$BIN"

if [[ -z "$PROFILE" ]]; then
  echo "sign-and-notarize: signed OK; VOLE_NOTARY_PROFILE unset — skipping notarization." >&2
  echo "  Configure notarytool keychain profile, then re-run." >&2
  exit 0
fi

ZIP="$(mktemp -t vole-notarize).zip"
ditto -c -k --keepParent "$BIN" "$ZIP"
echo "sign-and-notarize: submitting $ZIP via profile $PROFILE"
xcrun notarytool submit "$ZIP" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$BIN"
echo "sign-and-notarize: notarized and stapled"
rm -f "$ZIP"
