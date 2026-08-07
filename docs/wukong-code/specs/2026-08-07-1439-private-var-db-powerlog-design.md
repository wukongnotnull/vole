# `/private/var/db/powerlog` 清理设计（system.sh 续刀）

- 日期：2026-08-07；状态：待实现（已批准）
- 依据：Mole `safe_sudo_find_delete "/private/var/db/powerlog" "*" "$MOLE_LOG_AGE_DAYS"`（maxdepth 5）
- 版本：**1.18.0**；规则 **523 → 524**

## 结论

仿 DiagnosticPipeline：深度 ≤5、普通文件、≥7d、`rule_id=private-var-db-powerlog`、Privilege + `sudo -n`；remap `parent(VOLE_TEST_SYSTEM_LIBRARY)/private/var/db/powerlog`。

## 非目标

memory reports / Adobe / 交互提权；不打 tag
