# `/private/var/db/DiagnosticPipeline` 清理设计（system.sh 续刀）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：Mole `safe_sudo_find_delete "/private/var/db/DiagnosticPipeline" "*" "$MOLE_LOG_AGE_DAYS"`（maxdepth 5，type f）；`is_private_allowlisted` 已放行
- 包版本意图：**`1.17.0`**

## 1. 结论

- **根**：`/private/var/db/DiagnosticPipeline`
- **深度**：1..=5；普通文件；无扩展名过滤
- **年龄**：≥7 天（无 `.tracev3` 分龄）
- **Privilege**：形状谓词 + apply 绑谓词/`is_file`/年龄/`sudo -n` permanent
- `rule_id`：`private-var-db-diagnostic-pipeline`
- 规则数：**522 → 523**；不 bump schema；不打 tag

## 2. 实现要点

- 谓词 / candidates / apply 仿 `private-var-db-diagnostics`（去掉分龄）
- remap：`parent(VOLE_TEST_SYSTEM_LIBRARY)/private/var/db/DiagnosticPipeline`
- 回归：旧文件删；新鲜 skip；三树 + 本 rule_id skip

## 3. 非目标

- powerlog / memory reports / Adobe Logs / 交互提权

## 4. 验收

规则 523；版本 1.17.0；PR security-review；CI 绿
