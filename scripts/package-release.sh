#!/usr/bin/env bash
# Package ad-hoc release tarballs for macOS (no Developer ID required).
# Produces: dist/vole-<version>-<arch>.tar.gz with bin/vole + share/vole/rules/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Optional local signing config (copy from scripts/signing.env.example).
# shellcheck source=/dev/null
[[ -f "$ROOT/scripts/signing.env" ]] && source "$ROOT/scripts/signing.env"

VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
RULES_SRC="$ROOT/data/rules"

mkdir -p "$OUT_DIR"

build_arch() {
  local arch="$1"
  rustup target add "$arch" >/dev/null 2>&1 || true
  cargo build -q -p vole-cli --release --target "$arch"
}

maybe_codesign() {
  local bin_path="$1"
  if [[ -z "${VOLE_CODESIGN_IDENTITY:-}" ]]; then
    return 0
  fi
  VOLE_BIN="$bin_path" bash "$ROOT/scripts/sign-and-notarize.sh"
}

package_arch() {
  local arch="$1"
  local name="vole-${VERSION}-${arch}"
  local stage="$OUT_DIR/$name"
  rm -rf "$stage"
  mkdir -p "$stage/bin" "$stage/share/vole/rules"
  install -m 755 "target/$arch/release/vole" "$stage/bin/vole"
  maybe_codesign "$stage/bin/vole"
  cp "$RULES_SRC"/*.toml "$stage/share/vole/rules/"
  tar -C "$OUT_DIR" -czf "$OUT_DIR/${name}.tar.gz" "$name"
  rm -rf "$stage"
  echo "  $OUT_DIR/${name}.tar.gz"
}

echo "==> package-release v${VERSION}"
build_arch aarch64-apple-darwin
build_arch x86_64-apple-darwin
echo "Artifacts:"
package_arch aarch64-apple-darwin
package_arch x86_64-apple-darwin
(
  cd "$OUT_DIR"
  shasum -a 256 \
    "vole-${VERSION}-aarch64-apple-darwin.tar.gz" \
    "vole-${VERSION}-x86_64-apple-darwin.tar.gz" > SHA256SUMS
)
echo "  $OUT_DIR/SHA256SUMS"
echo "Done. Upload dist/*.tar.gz and dist/SHA256SUMS to GitHub Release (tag v${VERSION})."
