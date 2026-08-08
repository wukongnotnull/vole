#!/usr/bin/env bash
# M4 stub for §3.2 command-surface gate (spec 2026-08-08-2030).
# Full CI enforcement lands at closeout; default is report-only.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOLE="$ROOT/third_party/mole-1.48.1/mole"
VOLE_MAIN="$ROOT/crates/vole-cli/src/main.rs"
INTERACTIVE="$ROOT/crates/vole-cli/src/interactive.rs"
REPORT_ONLY=1
[[ "${1:-}" == "--enforce" ]] && REPORT_ONLY=0

required=(
  clean uninstall optimize optimise analyze analyse status history
  completion purge installer touchid update remove
)

if [[ ! -f "$MOLE" || ! -f "$VOLE_MAIN" ]]; then
  echo "error: missing Mole pin or vole-cli main.rs" >&2
  exit 2
fi

mole_routes=$(
  {
    # Quoted command tokens in mole dispatch / early history
    grep -Eo '"[a-z]+"' "$MOLE" | tr -d '"'
    echo history
  } | sort -u
)

vole_cmds=$(
  {
    if grep -Eq '^\s+Clean\b' "$VOLE_MAIN"; then echo clean; fi
    if grep -Eq '^\s+Uninstall\b' "$VOLE_MAIN"; then echo uninstall; fi
    if grep -Eq '^\s+Optimize\b' "$VOLE_MAIN"; then echo optimize; fi
    if grep -Eq '^\s+Status\b' "$VOLE_MAIN"; then echo status; fi
    if grep -Eq '^\s+Analyze\b' "$VOLE_MAIN"; then echo analyze; fi
    if grep -Eq '^\s+History\b' "$VOLE_MAIN"; then echo history; fi
    if grep -Eq '^\s+Completions\b' "$VOLE_MAIN"; then
      echo completions
    fi
    if grep -Eqi 'visible_alias.*"completion"|alias.*"completion"|#\[command\(.*alias.*"completion"' "$VOLE_MAIN"; then
      echo completion
    fi
    if grep -Eqi 'optimise' "$VOLE_MAIN"; then echo optimise; fi
    if grep -Eqi 'analyse' "$VOLE_MAIN"; then echo analyse; fi
    if grep -Eq '^\s+Purge\b' "$VOLE_MAIN"; then echo purge; fi
    if grep -Eq '^\s+Installer\b' "$VOLE_MAIN"; then echo installer; fi
    if grep -Eqi '^\s+Touchid\b|^\s+TouchId\b' "$VOLE_MAIN"; then echo touchid; fi
    if grep -Eq '^\s+Update\b' "$VOLE_MAIN"; then echo update; fi
    if grep -Eq '^\s+Remove\b' "$VOLE_MAIN"; then echo remove; fi
  } | sort -u
)

echo "=== Mole routes (extracted) ==="
echo "$mole_routes" | tr '\n' ' '
echo
echo "=== Vole cmds/aliases (detected) ==="
echo "$vole_cmds" | tr '\n' ' '
echo
echo "=== Required coverage gaps ==="
gaps=0
for c in "${required[@]}"; do
  if ! printf '%s\n' "$vole_cmds" | grep -qx "$c"; then
    echo "MISSING: $c"
    gaps=$((gaps + 1))
  fi
done

if grep -Eq '^\s+Hints\b' "$VOLE_MAIN"; then
  echo "UNEXPECTED: top-level Hints command"
  gaps=$((gaps + 1))
fi

# Bare-call no-network: interactive menu must not probe updates
if grep -Eqi 'check_for_updates|api\.github|github\.com/.*/releases|brew outdated|brew upgrade' "$INTERACTIVE"; then
  echo "UNEXPECTED: interactive menu appears to probe updates/network"
  gaps=$((gaps + 1))
else
  echo "OK: interactive.rs has no update/network probe markers"
fi

if [[ "$gaps" -gt 0 ]]; then
  echo "gaps=$gaps (expected during M4–M9; closeout --enforce must be 0)"
  if [[ "$REPORT_ONLY" -eq 1 ]]; then
    exit 0
  fi
  exit 1
fi

echo "OK: command surface covers required set"
exit 0
