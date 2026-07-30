#!/usr/bin/env bash
# Push signing + notarization secrets to GitHub Actions (wukongnotnull/vole).
#
# Run on your Mac in Terminal.app after exporting Developer ID .p12 and (optional) API key.
# Does NOT commit secrets to git.
#
# Usage:
#   bash scripts/setup-ci-secrets.sh
#   bash scripts/setup-ci-secrets.sh --repo wukongnotnull/vole
set -euo pipefail

REPO="${GITHUB_REPOSITORY:-wukongnotnull/vole}"
DRY_RUN=0

usage() {
  cat <<EOF
Usage: bash scripts/setup-ci-secrets.sh [--repo OWNER/NAME] [--dry-run]

Sets GitHub Actions secrets via \`gh secret set\`:
  VOLE_CODESIGN_IDENTITY
  APPLE_CERTIFICATE_BASE64
  APPLE_CERTIFICATE_PASSWORD
  APPLE_API_KEY_BASE64      (optional, for notarization)
  APPLE_API_KEY_ID          (optional)
  APPLE_API_ISSUER_ID       (optional)

Prerequisites:
  - gh auth login
  - Developer ID .p12 exported from Keychain Access
  - (optional) App Store Connect API key .p8 for notarization
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) REPO="$2"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

command -v gh >/dev/null || { echo "Install GitHub CLI: brew install gh" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "Run: gh auth login" >&2; exit 1; }

IDENTITY="${VOLE_CODESIGN_IDENTITY:-Developer ID Application: Kong Wu (WCYC8XY4V2)}"
echo "==> setup-ci-secrets for $REPO"
echo "    codesign identity: $IDENTITY"

set_secret() {
  local name="$1"
  local value="$2"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "DRY: would set $name (${#value} chars)"
    return 0
  fi
  printf '%s' "$value" | gh secret set "$name" --repo "$REPO"
  echo "OK: $name"
}

read -r -p "Path to Developer ID .p12: " P12
[[ -f "$P12" ]] || { echo "File not found: $P12" >&2; exit 1; }
read -r -s -p "P12 export password: " P12_PASS
echo
[[ -n "$P12_PASS" ]] || { echo "Password required." >&2; exit 1; }

B64="$(base64 < "$P12" | tr -d '\n')"
set_secret "VOLE_CODESIGN_IDENTITY" "$IDENTITY"
set_secret "APPLE_CERTIFICATE_BASE64" "$B64"
set_secret "APPLE_CERTIFICATE_PASSWORD" "$P12_PASS"

echo
read -r -p "Configure notarization secrets too? [y/N]: " DO_NOTARY
if [[ "$DO_NOTARY" =~ ^[Yy] ]]; then
  read -r -p "Path to AuthKey_*.p8: " P8
  [[ -f "$P8" ]] || { echo "File not found: $P8" >&2; exit 1; }
  KEY_ID="$(basename "$P8" .p8 | sed 's/^AuthKey_//')"
  if [[ ! "$KEY_ID" =~ ^[A-Z0-9]{10}$ ]]; then
    read -r -p "API Key ID (10 chars): " KEY_ID
  fi
  read -r -p "Issuer ID (UUID): " ISSUER_ID
  P8_B64="$(base64 < "$P8" | tr -d '\n')"
  set_secret "APPLE_API_KEY_BASE64" "$P8_B64"
  set_secret "APPLE_API_KEY_ID" "$KEY_ID"
  set_secret "APPLE_API_ISSUER_ID" "$ISSUER_ID"
fi

echo
echo "Done. Secrets are stored in GitHub → $REPO → Settings → Secrets."
echo "Next tag push (e.g. v0.0.9) will sign (+ notarize if API key set) in release.yml."
