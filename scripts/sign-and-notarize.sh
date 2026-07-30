#!/usr/bin/env bash
# Developer ID codesign + notarize placeholder for vole.
# Without Apple credentials this script exits non-zero with a clear message.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Optional local signing config (copy from scripts/signing.env.example).
# shellcheck source=/dev/null
[[ -f "$ROOT/scripts/signing.env" ]] && source "$ROOT/scripts/signing.env"

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

run_notary_submit() {
  local zip="$1"
  if [[ -n "$PROFILE" ]]; then
    xcrun notarytool submit "$zip" --keychain-profile "$PROFILE" --wait
    return 0
  fi
  if [[ -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER_ID:-}" ]]; then
    local key_file="${APPLE_API_KEY_PATH:-${RUNNER_TEMP:-/tmp}/vole-AuthKey.p8}"
    if [[ -n "${APPLE_API_KEY_BASE64:-}" ]]; then
      echo "$APPLE_API_KEY_BASE64" | base64 --decode > "$key_file"
    elif [[ ! -f "$key_file" ]]; then
      echo "sign-and-notarize: APPLE_API_KEY_BASE64 or APPLE_API_KEY_PATH required" >&2
      return 1
    fi
    xcrun notarytool submit "$zip" \
      --key "$key_file" \
      --key-id "$APPLE_API_KEY_ID" \
      --issuer "$APPLE_API_ISSUER_ID" \
      --wait
    return 0
  fi
  return 1
}

CAN_NOTARY=0
if [[ -n "$PROFILE" ]] || [[ -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER_ID:-}" ]]; then
  CAN_NOTARY=1
fi

if [[ "$CAN_NOTARY" -eq 0 ]]; then
  echo "sign-and-notarize: signed OK; notarization skipped (no profile / API key)." >&2
  echo "  Local: bash scripts/setup-notary-profile.sh" >&2
  echo "  CI: bash scripts/setup-ci-secrets.sh" >&2
  exit 0
fi

ZIP="$(mktemp -t vole-notarize).zip"
ditto -c -k --keepParent "$BIN" "$ZIP"
echo "sign-and-notarize: submitting $ZIP"
SUBMIT_OUT="$(mktemp -t vole-notary-out.XXXXXX)"
run_notary_submit "$ZIP" | tee "$SUBMIT_OUT"
rm -f "$ZIP"

SUBMISSION_ID="$(sed -n 's/^[[:space:]]*id:[[:space:]]*//p' "$SUBMIT_OUT" | tail -1)"
rm -f "$SUBMIT_OUT"

# stapler supports .app / .dmg / .pkg only — bare CLI binaries return Error 73.
if xcrun stapler staple "$BIN" 2>/dev/null; then
  echo "sign-and-notarize: notarized and stapled"
else
  echo "sign-and-notarize: notarized (Accepted); staple skipped for bare CLI binary." >&2
  echo "  Gatekeeper checks Apple's servers on first run (ticket id: ${SUBMISSION_ID:-unknown})." >&2
  echo "  Verify: spctl -a -vv -t install \"$BIN\"  → expect 'source=Notarized Developer ID'" >&2
fi
