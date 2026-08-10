#!/usr/bin/env bash
# Verify Developer ID identity is present in the login keychain.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=/dev/null
[[ -f "$ROOT/scripts/signing.env" ]] && source "$ROOT/scripts/signing.env"

IDENTITY="${VOLE_CODESIGN_IDENTITY:-Developer ID Application: Kong Wu (WCYC8XY4V2)}"
REAL_HOME="${VOLE_KEYCHAIN_HOME:-}"
if [[ -z "$REAL_HOME" ]]; then
  REAL_HOME="$(dscl . -read "/Users/$(whoami)" NFSHomeDirectory 2>/dev/null | awk '{print $2}')"
fi
REAL_HOME="${REAL_HOME:-$HOME}"
LOGIN_KEYCHAIN="${VOLE_LOGIN_KEYCHAIN:-$REAL_HOME/Library/Keychains/login.keychain-db}"

echo "==> check-signing"
echo "    identity: $IDENTITY"
echo "    shell HOME: $HOME"
echo "    keychain home: $REAL_HOME"

if [[ "$HOME" != "$REAL_HOME" && "$HOME" == /var/folders/* ]]; then
  echo "WARN: Cursor/CI 沙箱 HOME ($HOME) ≠ 用户目录 ($REAL_HOME)" >&2
  echo "      钥匙串访问 GUI 里可见，但 IDE 内置终端可能读不到私钥。" >&2
  echo "      请在「终端.app」或 iTerm 中运行本脚本。" >&2
fi

find_identities() {
  if [[ -f "$LOGIN_KEYCHAIN" ]]; then
    security find-identity -v -p codesigning "$LOGIN_KEYCHAIN" 2>/dev/null || true
    security find-identity -v -p codesigning 2>/dev/null || true
  else
    security find-identity -v -p codesigning 2>/dev/null || true
  fi
}

FOUND=0
while IFS= read -r line; do
  if [[ "$line" == *"$IDENTITY"* ]]; then
    FOUND=1
    MATCH_LINE="$line"
    break
  fi
done < <(find_identities)

if [[ "$FOUND" -eq 0 ]]; then
  echo "FAIL: codesign CLI 未找到 identity。" >&2
  echo >&2
  echo "若「钥匙串访问」里已显示该证书 + 私钥，常见原因：" >&2
  echo "  1. 在 Cursor 内置终端运行（沙箱 HOME，无法解锁私钥）→ 改用终端.app" >&2
  echo "  2. 私钥访问控制未允许 codesign → 钥匙串中双击私钥 → 访问控制" >&2
  echo "  3. 缺少中间证书 → 从 Apple PKI 安装 Developer ID CA" >&2
  echo >&2
  echo "当前 security find-identity -p codesigning 输出：" >&2
  find_identities | head -10 >&2 || true
  exit 1
fi

echo "OK: codesigning identity found"
echo "    $MATCH_LINE"

if [[ -n "${VOLE_NOTARY_PROFILE:-}" ]]; then
  NOTARY_OK=0
  # Do not pass --limit: older notarytool builds reject it and false-negative
  # a valid keychain profile as "missing credentials".
  if xcrun notarytool history --keychain-profile "$VOLE_NOTARY_PROFILE" >/dev/null 2>&1; then
    NOTARY_OK=1
  elif security find-generic-password -a "$VOLE_NOTARY_PROFILE" 2>/dev/null | grep -q 'class: "genp"'; then
    # API-key profiles may not respond to history; keychain entry is enough.
    NOTARY_OK=1
  fi
  if [[ "$NOTARY_OK" -eq 1 ]]; then
    echo "OK: notary profile '$VOLE_NOTARY_PROFILE'"
  else
    echo "WARN: could not verify notary profile '$VOLE_NOTARY_PROFILE' (submit may still work)" >&2
  fi
elif [[ -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER_ID:-}" && -n "${APPLE_API_KEY_BASE64:-}" ]]; then
  echo "OK: notary API key env (APPLE_API_KEY_ID + ISSUER + BASE64)"
else
  echo "SKIP: notarization not configured (sign-only OK)"
  echo "      Local: bash scripts/setup-notary-profile.sh"
fi
