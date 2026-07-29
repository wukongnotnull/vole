#!/usr/bin/env bash
# 确保 protocol.md 提到所有 StreamEvent type 与 Report 关键字段，且已标注冻结。
set -euo pipefail
DOC=docs/protocol.md
fail=0
for needle in progress candidate skipped done aborted trashed_bytes deleted_bytes schema_version FROZEN; do
    if ! grep -q "$needle" "$DOC"; then
        echo "FAIL: $DOC 缺少 $needle" >&2
        fail=1
    fi
done
[[ $fail -eq 0 ]] && echo "OK: protocol.md 关键字段齐全且已 FROZEN"
exit $fail
