#!/usr/bin/env bash
set -euo pipefail
REPO=$(cd "$(dirname "$0")/.." && pwd)
cd "$REPO"

VOLE_JSON=$(cargo run -q -p vole-cli -- status --json)
python3 - "$VOLE_JSON" <<'PY'
import json, sys
v = json.loads(sys.argv[1])
assert v.get("host"), "host missing"
assert v.get("platform"), "platform missing"
hw = v["hardware"]
assert hw.get("total_ram"), "total_ram missing"
assert hw.get("os_version"), "os_version missing"
assert 0 <= v["cpu"]["usage"] <= 100
assert 0 <= v["memory"]["used_percent"] <= 100
assert 0 <= v["health_score"] <= 100
print("OK: vole status JSON structure and ranges")
PY

MOLE="$REPO/third_party/mole-1.48.1/mo"
if [[ -x "$MOLE" ]]; then
  MOLE_JSON=$(env HOME="${HOME:-/tmp}" "$MOLE" status --json 2>/dev/null || true)
  if [[ -n "$MOLE_JSON" ]] && echo "$MOLE_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
    python3 - "$MOLE_JSON" "$VOLE_JSON" <<'PY'
import json, sys
m, v = json.loads(sys.argv[1]), json.loads(sys.argv[2])
for key in ("host", "platform"):
    if m.get(key) and v.get(key):
        assert m[key] == v[key], f"{key} mismatch"
for key in ("total_ram", "os_version"):
    if m["hardware"].get(key) and v["hardware"].get(key):
        assert m["hardware"][key] == v["hardware"][key], f"hardware.{key} mismatch"
print("OK: static fields match mo status --json")
PY
  else
    echo "SKIP: mole status binary unavailable (run mo update to build)"
  fi
fi
