#!/usr/bin/env bash
# Minimal local install helper for vole (pre-Homebrew).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

echo "Building vole (release)…"
cargo build -q -p vole-cli --release --manifest-path "$ROOT/Cargo.toml"

mkdir -p "$PREFIX/bin"
install -m 755 "$ROOT/target/release/vole" "$PREFIX/bin/vole"
echo "Installed: $PREFIX/bin/vole"
echo "Ensure $PREFIX/bin is on PATH."
echo "Completions: vole completions zsh  # see README"
echo "Signing/Homebrew: docs/findings/2026-07-phase5-signing.md"
