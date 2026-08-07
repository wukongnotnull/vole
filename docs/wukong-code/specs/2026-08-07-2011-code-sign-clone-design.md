# `*.code_sign_clone` 清理设计（system.sh 续刀）

- 日期：2026-08-07；已批准
- 依据：Mole `find /private/var/folders -maxdepth 5 -type d -name "*.code_sign_clone" -path "*/X/*"`，跳过 `is_endpoint_security_cache_path`，`safe_sudo_remove`（无年龄阈）
- 版本：**1.24.0**；规则 **529 → 530**

## 结论

- 一规则 `code-sign-clone`
- 目标：folders 下相对根深度 ≤5、路径段含 `X`、叶目录名以 `.code_sign_clone` 结尾
- 无年龄阈；Privilege + apply 绑谓词 + 显式跳过 EDR + `sudo -n` 永久删除整目录
- remap：`VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` → `$ROOT/private/var/folders`

## 非目标

GPU Metal caches、Install macOS*.app、`/Library/Updates`；不打 tag
