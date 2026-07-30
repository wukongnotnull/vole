# M2：Optimize spike — 主路径 vs 长尾

**日期**：2026-07-30  
**状态**：完成  
**Mole 钉版**：`third_party/mole-1.48.1`  
**计划**：[`docs/wukong-code/plans/2026-07-30-2012-v2-m2-m3-optimize.md`](../wukong-code/plans/2026-07-30-2012-v2-m2-m3-optimize.md)

## 1. 结论

M3 交付 **12** 个用户域、无 sudo 的高对齐任务；其余 **11** 个（含需提权 / 诊断向 / 高复杂度）进 `coverage_note`，提示继续用 Mole。

本 spike **未扩大**计划中的主路径；`shared_file_list_repair` 与 `login_items_audit` 仍属长尾。

交互：`--plan` / `--apply` 对齐 clean；不移植 Mole health 大屏与 `bc` 依赖。

## 2. Mole 对照

| Mole | 路径 | Vole M3 |
|---|---|---|
| catalog 注册 | `lib/optimize/catalog.sh` | `optimize/catalog.rs` |
| `execute_optimization` | `lib/optimize/tasks.sh` | `optimize/tasks` + apply 分发 |
| `opt_*` handlers | `lib/optimize/tasks.sh` | 同名 task_id |
| `fix_broken_preferences` | `lib/optimize/maintenance.sh` | `discover_fix_broken_configs` |
| `--dry-run` | `bin/optimize.sh` | `--plan` |
| bats | `tests/optimize*.bats` | unit + `optimize_cli` + fixtures |

## 3. 主路径（M3 必做）

| action | 形态 | plan 可预览 | 关键安全点 |
|---|---|---|---|
| `cache_refresh` | delete（+ apply 前 `qlmanage -r`） | 3 个 cache 路径若存在 | Cleanup 保护；重建由系统完成 |
| `saved_state_cleanup` | delete | `*.savedState` mtime>30d | 跳过受保护路径 |
| `fix_broken_configs` | delete | `plutil -lint` 失败的非 Apple plist | 不删 `com.apple.*`；尊重保护清单 |
| `quarantine_cleanup` | action | DB 存在且行数>0 → 1 条目 | 仅清 LSQuarantineEvent |
| `sqlite_vacuum` | action | 目标 DB 存在 → 条目 | app 运行中 skip；integrity 后 VACUUM |
| `prevent_network_dsstore` | action | 任 key 未设 → sentinel | `defaults write` Network+USB |
| `legacy_overrides_audit` | action | 命中 override → 条目（按 key） | 只 `defaults delete`，不整文件删 |
| `launch_agents_cleanup` | delete | Program 绝对路径缺失 | 跳过 PATH 短名与未挂载 `/Volumes` |
| `notification_cleanup` | action | NC db ≥50MB | sqlite DELETE+VACUUM；busy 则 skip |
| `coreduet_cleanup` | action | Knowledge 合计 ≥100MB | 删 WAL/SHM + 旧 ZOBJECT；busy skip |
| `dock_refresh` | action | 恒 1 条 sentinel | `killall Dock` |
| `launch_services_rebuild` | action | 恒 1 条 sentinel | `lsregister -gc` / `-r -f` |

## 4. 长尾（coverage_note，不实现）

| action | 原因 |
|---|---|
| `system_maintenance` / `network_optimization` | DNS / mDNSResponder 需 sudo |
| `memory_pressure_relief` | `sudo purge` |
| `network_stack_optimize` | `sudo route` / `arp` |
| `disk_permissions_repair` | `sudo diskutil resetUserPermissions` |
| `spotlight_index_optimize` / `spotlight_orphan_rules_cleanup` | 索引/规则面；常需 sudo 或易误伤 |
| `periodic_maintenance` | `sudo periodic` |
| `disk_verify` | 默认关闭；可能卡住系统 |
| `login_items_audit` | AppleScript + 可选 sudo `sfltool` |
| `shared_file_list_repair` | 共享列表 DB 修复复杂度高 |

## 5. 协议约定（零 schema bump）

- `optimize:delete:<task_id>` → `mole_delete_verified`（默认 Trash）
- `optimize:action:<task_id>` → 具名 handler；`path` 为真实目标或 sentinel
- 保护模式：`ProtectionMode::Cleanup`（**不用** Uninstall 放宽）

## 6. 建议 fixture

| fixture | 覆盖 |
|---|---|
| `saved_state_old.json` + temp tree | 旧/新 savedState |
| `broken_pref` | 损坏非 Apple plist |
| `broken_launch_agent` | Program 指向缺失绝对路径 |
| `quarantine_db` | 最小 SQLite + 1 行 |
| `empty` | 无候选；coverage_note 非空 |

## 7. 风险

1. action 类 apply 不可逆（VACUUM / defaults / killall）——必须经 plan TTL + 用户显式 `--apply`
2. `notification` / `coreduet` 阈值门槛高，日常 plan 常为空——属预期
3. `lsregister` / `Dock` 有短暂 UI 闪烁——与 Mole 一致，coverage 中可一句提示
