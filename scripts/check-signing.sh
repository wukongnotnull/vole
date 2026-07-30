#!/usr/bin/env bash
# Verify Developer ID identity is present in the login keychain.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=/dev/null
[[ -f "$ROOT/scripts/signing.env" ]] && source "$ROOT/scripts/signing.env"

IDENTITY="${VOLE_CODESIGN_IDENTITY:-Developer ID Application: Kong Wu (WCYC8XY4V2)}"

echo "==> check-signing"
echo "    identity: $IDENTITY"

if ! security find-identity -v -p codesigning | grep -Fq "$IDENTITY"; then
  echo "FAIL: identity not in keychain." >&2
  echo "  Install the Developer ID Application cert (Keychain Access → My Certificates)." >&2
  echo "  Or export from Apple Developer → Certificates, then double-click the .cer." >&2
  security find-identity -v -p codesigning 2>&1 || true
  exit 1
fi

echo "OK: codesigning identity found"

if [[ -n "${VOLE_NOTARY_PROFILE:-}" ]]; then
  if xcrun notarytool history --keychain-profile "$VOLE_NOTARY_PROFILE" --limit 1 >/dev/null 2>&1; then
    echo "OK: notary profile '$VOLE_NOTARY_PROFILE'"
  else
    echo "WARN: VOLE_NOTARY_PROFILE=$VOLE_NOTARY_PROFILE not usable" >&2
    exit 2
  fi
else
  echo "SKIP: VOLE_NOTARY_PROFILE unset (sign-only OK)"
fi
