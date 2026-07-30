#!/usr/bin/env bash
# Create a local notarytool keychain profile for vole (Terminal.app only).
#
# Two supported auth methods (pick one):
#   A) App Store Connect API key (.p8) — recommended for CI + local
#   B) Apple ID + app-specific password in Keychain
#
# Usage:
#   bash scripts/setup-notary-profile.sh
#   bash scripts/setup-notary-profile.sh --api-key ~/Downloads/AuthKey_XXXX.p8
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="${VOLE_NOTARY_PROFILE:-vole-notary}"
TEAM_ID="${APPLE_TEAM_ID:-WCYC8XY4V2}"
SIGNING_ENV="$ROOT/scripts/signing.env"

usage() {
  cat <<EOF
Usage: bash scripts/setup-notary-profile.sh [--api-key PATH] [--profile NAME]

Creates notarytool keychain profile "$PROFILE" and appends to scripts/signing.env.

Options:
  --api-key PATH   App Store Connect API key (.p8)
  --profile NAME   Keychain profile name (default: vole-notary)
  --team-id ID     Team ID (default: WCYC8XY4V2)

Apple ID mode (no --api-key): prompts for Apple ID; uses app-specific password
  stored in Keychain as AC_PASSWORD (create at appleid.apple.com).

Get API key: App Store Connect → Users and Access → Integrations → Keys
EOF
}

API_KEY=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --api-key) API_KEY="$2"; shift 2 ;;
    --profile) PROFILE="$2"; shift 2 ;;
    --team-id) TEAM_ID="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ "$HOME" == /var/folders/* ]] && [[ -z "${VOLE_KEYCHAIN_HOME:-}" ]]; then
  echo "WARN: Cursor sandbox HOME — run in Terminal.app for keychain access." >&2
fi

if [[ -n "$API_KEY" ]]; then
  [[ -f "$API_KEY" ]] || { echo "API key not found: $API_KEY" >&2; exit 1; }
  KEY_ID="$(basename "$API_KEY" .p8 | sed 's/^AuthKey_//')"
  if [[ "$KEY_ID" == AuthKey_* ]] || [[ ! "$KEY_ID" =~ ^[A-Z0-9]+$ ]]; then
    read -r -p "App Store Connect Key ID (10 chars, e.g. AB12CD34EF): " KEY_ID
  fi
  read -r -p "Issuer ID (UUID from App Store Connect): " ISSUER_ID
  [[ -n "$KEY_ID" && -n "$ISSUER_ID" ]] || { echo "Key ID and Issuer ID required." >&2; exit 1; }
  echo "==> store-credentials $PROFILE (API key)"
  xcrun notarytool store-credentials "$PROFILE" \
    --key "$API_KEY" \
    --key-id "$KEY_ID" \
    --issuer "$ISSUER_ID"
else
  read -r -p "Apple ID email: " APPLE_ID
  [[ -n "$APPLE_ID" ]] || { echo "Apple ID required." >&2; exit 1; }
  cat <<EOF

Store app-specific password in Keychain first (one-time):
  security add-generic-password -a "$APPLE_ID" -s "AC_PASSWORD" -w

Or use an existing Keychain item named AC_PASSWORD.

EOF
  read -r -p "Keychain password item name [AC_PASSWORD]: " KC_ITEM
  KC_ITEM="${KC_ITEM:-AC_PASSWORD}"
  echo "==> store-credentials $PROFILE (Apple ID + @keychain:$KC_ITEM)"
  xcrun notarytool store-credentials "$PROFILE" \
    --apple-id "$APPLE_ID" \
    --team-id "$TEAM_ID" \
    --password "@keychain:$KC_ITEM"
fi

touch "$SIGNING_ENV"
grep -v 'VOLE_NOTARY_PROFILE=' "$SIGNING_ENV" > "${SIGNING_ENV}.tmp" || true
mv "${SIGNING_ENV}.tmp" "$SIGNING_ENV"
echo "export VOLE_NOTARY_PROFILE=\"$PROFILE\"" >> "$SIGNING_ENV"
echo "OK: wrote VOLE_NOTARY_PROFILE to $SIGNING_ENV"
echo "Verify: bash scripts/check-signing.sh"
