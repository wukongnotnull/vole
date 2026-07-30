#!/usr/bin/env bash
# Minimal local install helper for vole (pre-Homebrew / ad-hoc release).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

echo "Building vole (release)…"
cargo build -q -p vole-cli --release --manifest-path "$ROOT/Cargo.toml"

mkdir -p "$PREFIX/bin" "$PREFIX/share/vole/rules"
install -m 755 "$ROOT/target/release/vole" "$PREFIX/bin/vole"
cp "$ROOT/data/rules/"*.toml "$PREFIX/share/vole/rules/"

echo "Installed: $PREFIX/bin/vole"
echo "Rules:     $PREFIX/share/vole/rules (auto-discovered from bin/)."
echo "Override only if needed:"
echo "  export VOLE_RULES_DIR=\"$PREFIX/share/vole/rules\""
echo "Ensure $PREFIX/bin is on PATH."
echo "Completions: vole completions zsh  # see README"
echo "Release / signing: docs/findings/2026-07-phase5-signing.md"
