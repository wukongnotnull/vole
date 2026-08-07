# MemoryLimitViolations 清理设计（system.sh 续刀）

- 日期：2026-08-07；已批准
- 依据：Mole `safe_sudo_find_delete "/private/var/db/reportmemoryexception/MemoryLimitViolations" "*" "30"`（maxdepth 5）
- 版本：**1.19.0**；规则 **524 → 525**

## 结论

- 根：`/private/var/db/reportmemoryexception/MemoryLimitViolations`
- 深度 ≤5；普通文件；**≥30d**（非 7）
- `rule_id`：`private-var-db-memory-limit-violations`
- 仿 powerlog：形状谓词 + Privilege + apply 绑谓词/`sudo -n`
- remap：`parent(VOLE_TEST_SYSTEM_LIBRARY)/private/var/db/reportmemoryexception/MemoryLimitViolations`

## 非目标

Adobe Logs / 交互提权；不打 tag
