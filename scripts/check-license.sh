#!/usr/bin/env bash
# 校验许可证与归属声明存在。GPL-3.0 是硬要求，见设计文档第 2 节。
set -euo pipefail

fail=0

if ! grep -q 'GNU GENERAL PUBLIC LICENSE' LICENSE; then
    echo "FAIL: LICENSE 不是 GPL" >&2
    fail=1
fi

if ! grep -q 'Version 3, 29 June 2007' LICENSE; then
    echo "FAIL: LICENSE 不是 GPL-3.0" >&2
    fail=1
fi

if grep -q 'Apache License' LICENSE; then
    echo "FAIL: LICENSE 仍含 Apache 文本" >&2
    fail=1
fi

if ! grep -qi 'tw93/Mole' README.md; then
    echo "FAIL: README 缺少 Mole 归属" >&2
    fail=1
fi

if ! grep -qi 'GPL-3.0' README.md; then
    echo "FAIL: README 未声明 GPL-3.0" >&2
    fail=1
fi

[[ $fail -eq 0 ]] && echo "OK: 许可证与归属检查通过"
exit $fail
