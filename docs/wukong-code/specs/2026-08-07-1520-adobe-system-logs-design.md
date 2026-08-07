# Adobe /Library/Logs 清理设计（system.sh 续刀）

- 日期：2026-08-07；已批准
- 依据：Mole third-party system logs — `safe_sudo_find_delete` Adobe/CreativeCloud（maxdepth 5、≥7d）+ `safe_sudo_remove /Library/Logs/adobegc.log`（≥7d）
- 版本：**1.20.0**；规则 **525 → 526**

## 结论

- 一规则 `adobe-system-logs`
- 目标：`/Library/Logs/Adobe/**`、`/Library/Logs/CreativeCloud/**`（深度 1..=5 普通文件）∨ exact `/Library/Logs/adobegc.log`
- 年龄全部 ≥7d；Privilege + apply 绑谓词 + `sudo -n`
- remap：`VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` → `$ROOT/Library/Logs/{Adobe,CreativeCloud,adobegc.log}`

## 非目标

泛 `/Library/Logs`、其它厂商、交互提权；不打 tag
