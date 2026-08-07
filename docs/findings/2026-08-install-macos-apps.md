# Findings: Install macOS*.app（1.27.0）

## 动机

Mole `system.sh` 在清理 Installer 前用 `software_update_pending_or_unknown`：**缺 plist / 解析失败 / RecommendedUpdates 非空**一律当作 pending。背景是 macOS 27 beta 上误判「无更新」清理 staged payload 曾导致无法启动。

## 闸口（已落地）

1. SWU fail-closed（整规则）
2. `pgrep -f` 运行中跳过
3. `DTPlatformVersion` 大版本 == `sw_vers` 大版本 → keep
4. bundle mtime ≥ 14 天
5. allowlist：仅 `{apps_root}/Install macOS*.app`
6. 永久 `sudo -n`；永不碰 `/Library/Updates`、`/macOS Install Data`

## 测试注入

`VOLE_TEST_APPLICATIONS` / `VOLE_TEST_SOFTWARE_UPDATE_PLIST` / `VOLE_TEST_MACOS_MAJOR`
