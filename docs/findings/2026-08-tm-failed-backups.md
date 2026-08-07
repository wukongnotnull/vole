# Findings: Time Machine failed inProgress backups（1.28.0）

## 动机

Mole `clean_time_machine_failed_backups` 是 system.sh 在 deep system 主链之后唯一实质删除余项。误删进行中备份不可恢复 → fail-closed（Running/Unknown 整规则跳过）+ 48h 安全窗。

## 落地要点

- 仅 `tmutil delete`，禁止 `sudo rm`
- mtime 不可读 → keep
- 测试经 `TmDeps` / `ApplyPlanContext.tm_deps` 注入

## 非目标

本地 APFS 快照删除或报告（另刀）；永不碰 `/Library/Updates`。
