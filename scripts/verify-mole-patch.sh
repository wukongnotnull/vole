#!/usr/bin/env bash
# 验证一致性补丁未改变 mole 行为：打补丁前后 bats 套件结果必须一致。
# 见设计文档第 7 节 A 类的「补丁保真度」要求。
set -euo pipefail

# 绝对路径先算好：下面要在子 shell 里 cd，相对路径会失效。
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MOLE_DIR="$REPO/third_party/mole-1.48.1"
PATCH="$REPO/third_party/patches/001-conformance-jsonl.patch"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

command -v bats >/dev/null || { echo "需要 bats：brew install bats-core" >&2; exit 1; }

# 基线：两份隔离副本，避免在仓库工作树上跑测试留下副作用。
rsync -a --exclude '.git' "$MOLE_DIR/" "$WORK/pristine/"
rsync -a --exclude '.git' "$MOLE_DIR/" "$WORK/patched/"
(cd "$WORK/pristine" && patch -p1 -R < "$PATCH" >/dev/null)

echo "=== 跑 pristine 基线 ==="
(cd "$WORK/pristine" && MOLE_TEST_NO_AUTH=1 ./scripts/test.sh) > "$WORK/before.log" 2>&1 || true
grep -oE '[0-9]+ tests?, [0-9]+ failures?' "$WORK/before.log" | tail -1 > "$WORK/before.summary"

echo "=== 跑打补丁版本 ==="
(cd "$WORK/patched" && MOLE_TEST_NO_AUTH=1 ./scripts/test.sh) > "$WORK/after.log" 2>&1 || true
grep -oE '[0-9]+ tests?, [0-9]+ failures?' "$WORK/after.log" | tail -1 > "$WORK/after.summary"

echo "补丁前: $(cat "$WORK/before.summary")"
echo "补丁后: $(cat "$WORK/after.summary")"

if diff -q "$WORK/before.summary" "$WORK/after.summary" >/dev/null; then
    echo "OK: 补丁未改变 bats 套件结果"
else
    echo "FAIL: 补丁改变了 bats 结果，基准不可信" >&2
    diff "$WORK/before.log" "$WORK/after.log" | head -50 >&2
    exit 1
fi
