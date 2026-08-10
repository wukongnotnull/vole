#!/usr/bin/env bash
# Pin Homebrew Formula/vole.rb stable url/sha256 from local dist/ or GitHub Release.
# Usage: bash scripts/update-homebrew-formula.sh [VERSION]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FORMULA="$ROOT/Formula/vole.rb"
VERSION="${1:-$(grep '^version' "$ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')}"
TAG="v${VERSION}"
DIST="$ROOT/dist"
AARCH64="vole-${VERSION}-aarch64-apple-darwin.tar.gz"
X86_64="vole-${VERSION}-x86_64-apple-darwin.tar.gz"

sha_for() {
  local file="$1"
  if [[ -f "$file" ]]; then
    shasum -a 256 "$file" | awk '{print $1}'
    return 0
  fi
  local url="https://github.com/wukongnotnull/vole/releases/download/${TAG}/$(basename "$file")"
  curl -fsSL "$url" | shasum -a 256 | awk '{print $1}'
}

SHA_ARM="$(sha_for "$DIST/$AARCH64")"
SHA_X86="$(sha_for "$DIST/$X86_64")"

export FORMULA VERSION TAG AARCH64 X86_64 SHA_ARM SHA_X86
python3 <<'PY'
import os, re
from pathlib import Path

p = Path(os.environ["FORMULA"])
version = os.environ["VERSION"]
tag = os.environ["TAG"]
aarch64 = os.environ["AARCH64"]
x86_64 = os.environ["X86_64"]
sha_arm = os.environ["SHA_ARM"]
sha_x86 = os.environ["SHA_X86"]

text = p.read_text()
text = re.sub(r'^\s*version\s+"[^"]*"', f'  version "{version}"', text, count=1, flags=re.M)
text = re.sub(r'(on_arm do\n\s*url )"[^"]*"', f'\\1"https://github.com/wukongnotnull/vole/releases/download/{tag}/{aarch64}"', text, count=1)
text = re.sub(r'(on_intel do\n\s*url )"[^"]*"', f'\\1"https://github.com/wukongnotnull/vole/releases/download/{tag}/{x86_64}"', text, count=1)
# Allow optional comment lines between url and sha256 (placeholder formula style).
text = re.sub(
    r'(on_arm do\n\s*url[^\n]*\n)(?:\s*#[^\n]*\n)*(\s*sha256 )"[^"]*"',
    f'\\1\\2"{sha_arm}"',
    text,
    count=1,
)
text = re.sub(
    r'(on_intel do\n\s*url[^\n]*\n)(?:\s*#[^\n]*\n)*(\s*sha256 )"[^"]*"',
    f'\\1\\2"{sha_x86}"',
    text,
    count=1,
)
p.write_text(text)
print(f"Updated {p} -> {tag}")
print(f"  aarch64 {sha_arm}")
print(f"  x86_64  {sha_x86}")
PY
