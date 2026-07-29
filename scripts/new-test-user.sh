#!/usr/bin/env bash
# 创建一次性本地测试账户。需要管理员权限。
# 见 docs/testing-environment.md。
set -euo pipefail

user=${1:?用法: $0 <username>}

if id "$user" >/dev/null 2>&1; then
    echo "用户 $user 已存在。删除请用：sudo sysadminctl -deleteUser $user" >&2
    exit 1
fi

echo "将创建标准（非管理员）账户 $user。"
read -r -p "继续？[y/N] " ok
[[ "$ok" == "y" ]] || exit 1

sudo sysadminctl -addUser "$user" -fullName "Vole Test" -password -

echo
echo "已创建。切换：su - $user"
echo "用完删除：sudo sysadminctl -deleteUser $user"
echo
echo "注意：新账户的 TCC 授权是全新的，首次跑 clean 会弹一批权限对话框。"
echo "这正是 Phase 0.5 要观测的行为之一。"
