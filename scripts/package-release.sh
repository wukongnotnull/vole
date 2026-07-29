#!/usr/bin/env bash
# Package ad-hoc release tarballs for macOS (no Developer ID required).
# Produces: dist/vole-<version>-<arch>.tar.gz with bin/vole + share/vole/rules/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
RULES_SRC="$ROOT/data/rules"

mkdir -p "$OUT_DIR"

build_arch() {
  local arch="$1"
  rustup target add "$arch" >/dev/null 2>&1 || true
  cargo build -q -p vole-cli --release --target "$arch"
}

package_arch() {
  local arch="$1"
  local name="vole-${VERSION}-${arch}"
  local stage="$OUT_DIR/$name"
  rm -rf "$stage"
  mkdir -p "$stage/bin" "$stage/share/vole/rules"
  install -m 755 "target/$arch/release/vole" "$stage/bin/vole"
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
echo "Done. Upload dist/*.tar.gz to GitHub Release (tag v${VERSION})."
