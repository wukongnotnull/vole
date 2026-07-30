# M0：Uninstall spike — 主路径 vs 长尾

**日期**：2026-07-30  
**状态**：完成  
**Mole 钉版**：`third_party/mole-1.48.1`  
**计划**：[`docs/wukong-code/plans/2026-07-30-1910-v2-m0-m1-uninstall.md`](../wukong-code/plans/2026-07-30-1910-v2-m0-m1-uninstall.md)

## 1. 结论

M1 按计划交付 **高对齐主路径**（枚举 → 保护策略 → 用户域残留 → sibling guard → plan/apply/废纸篓）。  
下列长尾 **不进 M1**，在 `coverage_note` 中诚实提示继续用 Mole。

本 spike **未扩大**计划中的主路径范围。

## 2. Mole 对照（将对齐的函数 / 测试）

| Mole | 路径 | Vole M1 对应 |
|---|---|---|
| `scan_applications` | `bin/uninstall.sh` | `ops/uninstall_plan::scan_applications` |
| `uninstall_list_apps` / `--dry-run` | `bin/uninstall.sh` | `--plan` / `--json`（无 TTY 多选 UI） |
| `should_protect_from_uninstall` | `lib/core/app_protection.sh` | `protection/uninstall.rs` |
| `official_uninstaller_vendor` | 同上 | 同上 |
| `find_app_files`（用户域子集） | 同上 | `protection/leftovers.rs` |
| `MOLE_UNINSTALL_MODE=1` | `batch.sh` export | `ProtectionMode::Uninstall` |
| `uninstall_bundle_id_has_surviving_sibling` | `lib/uninstall/batch.sh` | `find_bundle_siblings` + leftovers 跳过共享域 |
| bats | `tests/uninstall_safety.bats`、`uninstall_naming_variants.bats`、`uninstall_remove_file_list.bats` | Rust 单测 + `tests/fixtures/uninstall/` |

扫描根：Mole 含 `/Applications`、`$HOME/Applications`、`/Volumes/*/Applications`。  
**M1 主路径**：前两者；`/Volumes` 仅用于 **sibling 探测**（不主动枚举为卸载目标，除非同 bundle 冲突判定需要）。

## 3. M1 主路径（确认）

1. 枚举 `/Applications` + `$HOME/Applications` 下 `*.app`
2. `Contents/Info.plist` → bundle id / 显示名
3. `should_protect_from_uninstall`（system-critical，Apple 可卸 allowlist 除外）
4. `official_uninstaller_vendor` 命中 → skip（不进删除条目）
5. 用户域残留（精确 bundle id + 命名变体；拒绝通用词 / 短名；**无 TeamID 通配**）：
   - Containers / Group Containers（精确）
   - Preferences / Application Support / Caches / Logs / Saved Application State
   - 用户域 LaunchAgents（Program 指向该 app）
   - 命名变体路径（空格/连字符/小写等，对齐 `find_app_files` 头部变体逻辑）
6. Sibling：同 bundle id 其他 `.app` 仍在（含 `/Volumes` 探测）→ leftovers 共享域全 skip；当前 `.app` 仍可进 plan，coverage 注明
7. Plan：`rule_id` 前缀 `uninstall:`；复用 `schema_version=1`
8. Apply：TTL + TOCTOU + Uninstall 模式保护 + `mole_delete_verified`；默认废纸篓

## 4. M1 长尾（不做）

| 项 | Mole 位置 | 处理 |
|---|---|---|
| Homebrew cask zap | `lib/uninstall/brew.sh` + batch | coverage 提示 |
| Login items / AppleScript | `batch.sh` | 跳过 |
| 系统 LaunchDaemons / PrivilegedHelperTools | batch | 跳过（需 sudo） |
| `/Library` 广域残留 / sudo 删除 | batch `needs_sudo` | 跳过 |
| TTY 分页多选 UI | `menu_paginated` | 用 plan JSON 代替 |
| ByHost preferences 全量清理 | `find_app_files` 旁路 | 首版不做（安全面大） |
| 独立 CLI 保护名单（Claude/Codex/opencode home 目录） | naming_variants #993 | **主路径必须保留**：对齐 bats，禁止误删 `~/.claude` 等 |
| orphaned / purge | 另轨 | 不做 |

说明：#993 类「独立 CLI 状态目录保护」属于主路径安全约束，不是长尾功能。

## 5. 风险

| 风险 | 缓解 |
|---|---|
| 命名变体过宽误删 | 短名地板、通用词拒绝、bats→Rust 回归 |
| Sibling 漏判删共享数据 | 强制 sibling 探测后再扩 leftovers |
| 与 clean 共用 Plan 被误 apply | 独立 `apply_uninstall_proto_plan`；`rule_id` 前缀约定 |
| data-protected 在 uninstall 应可删 | `ProtectionMode::Uninstall` |
| 无 sudo 静默少清 | coverage_note 响亮列出跳过类别 |

## 6. 建议 fixture 来源

| 来源 bats | 首批 JSON fixture 主题 |
|---|---|
| `uninstall_naming_variants.bats` | 连字符/无空格变体；空名不匹配 |
| `uninstall_safety.bats` | 官方卸载器 vendor；畸形 bundle id 拒绝；system-critical 不进 plan |
| `uninstall_remove_file_list.bats` | （若适用）删除列表边界 | 可选第二批 |
| 自造 sibling | 两份同 bundle `.app` → leftovers skip |

## 7. 下一步

按计划 Task 2 起实现：`ProtectionMode` → uninstall 策略 → leftovers → plan/apply → CLI → **1.1.0**。
