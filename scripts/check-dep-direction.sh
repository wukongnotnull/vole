#!/usr/bin/env bash
# 固化设计文档 5.1 的分层规则：vole-cli → vole-core → vole-sys → vole-proto。
# 不加这道检查，方向一定会在某次「顺手 import 一下」中被破坏。
set -euo pipefail

fail=0

# 每个 crate 允许直接依赖的 workspace 内 crate。
allowed_for() {
    case "$1" in
        vole-proto) echo "" ;;
        vole-sys)   echo "vole-proto" ;;
        vole-core)  echo "vole-sys" ;;
        vole-cli)   echo "vole-core" ;;
        *)          echo "__unknown__" ;;
    esac
}

for manifest in crates/*/Cargo.toml; do
    crate=$(basename "$(dirname "$manifest")")
    allowed=$(allowed_for "$crate")

    # 抓 [dependencies] 里指向 workspace 内 crate 的 path 依赖。
    deps=$(grep -oE '^vole-[a-z]+ = \{ path' "$manifest" | awk '{print $1}' || true)

    for dep in $deps; do
        if [[ " $allowed " != *" $dep "* ]]; then
            echo "FAIL: $crate 不得依赖 ${dep}（允许：${allowed:-无}）" >&2
            fail=1
        fi
    done
done

[[ $fail -eq 0 ]] && echo "OK: crate 依赖方向合规"
exit $fail
