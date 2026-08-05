# B4 Orphaned App Data — 安全评审勾选

**日期**：2026-08-05  
**状态**：实现完成（分支 `feat/b4-orphaned-app-data`）  
**设计**：[`docs/wukong-code/specs/2026-08-05-1642-b4-orphaned-app-data-design.md`](../wukong-code/specs/2026-08-05-1642-b4-orphaned-app-data-design.md)  
**计划**：[`docs/wukong-code/plans/2026-08-05-1701-b4-orphaned-app-data.md`](../wukong-code/plans/2026-08-05-1701-b4-orphaned-app-data.md)

## §7 清单

| # | 项 | 证据 |
|---|---|---|
| 1 | 扫描根 = Caches/Logs/Saved State；NEVER 列表在模块注释 | `orphan/mod.rs`、`select.rs` `orphan_root_index`、`zzz-orphaned.toml` |
| 2 | mdfind 超时/错误 fail-closed | `judge.rs` + `not_orphan_when_mdfind_errors` |
| 3 | Spotlight 不可用不误判 | `not_orphan_when_spotlight_disabled` |
| 4 | 敏感族 + `should_protect_data` | `sensitive_and_system_denylists`；judge 调 `should_protect_data` |
| 5 | apply 重判 | `apply_skips_orphaned_when_rejudge_fails_after_plan` |
| 6 | 规则顺序：具名胜 orphaned | `orphaned_rule_loads_last_among_enabled` + `plan_orphaned_loses_dedup_to_named_rule` |
| 7 | `MOLE_ORPHAN_AGE_DAYS` clamp | `age_clamp_rejects_zero_and_garbage` |
| 8 | 默认 Trash；`--permanent` 仅 apply | 复用既有 clean apply（无新删除漏斗） |
| 9 | 无 sudo；无 `/Library` 删除 | 扫描根仅 `$HOME/Library/...` |
| 10 | 无 Containers / Group Containers / LaunchAgents 删除 | `orphan_root_index` 拒绝；LaunchAgents 仅作活跃证据 |
| 11 | FDA：`Library/Caches` 不可读 → 空候选 | `LibraryInaccessible` → handler 返回 `[]` |
| 12 | 迭代上限 100 + mdfind 64 | `MAX_ORPHAN_ITERATIONS` / `MdfindBudget` |
| 13 | CI 注入假 deps | `FakeOrphanDeps` + `Orchestrator::with_orphan_deps`；单测不触真 mdfind |
| 14 | 本 findings | 本文件 |

## 非目标（确认未做）

- Claude VM orphan（B4.1）
- system services orphan
- container stubs / orphan dotdir hints
