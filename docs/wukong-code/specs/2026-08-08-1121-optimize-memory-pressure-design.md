# W2b②：`memory_pressure_relief` 设计

- 日期：2026-08-08
- 状态：已批准（Condensed；默认采纳推荐）
- 依据：Mole parity roadmap §2.3 W2b②；[`../../findings/2026-07-v2-m2-optimize-spike.md`](../../findings/2026-07-v2-m2-optimize-spike.md)；Mole `opt_memory_pressure_relief` / `is_memory_pressure_high`（`third_party/mole-1.48.1/lib/optimize/tasks.sh`）；`optimize/catalog.rs`；W2b① design [`2026-08-08-0156-optimize-system-network-design.md`](2026-08-08-0156-optimize-system-network-design.md)
- 包版本意图：**1.36.0**（MINOR；若并行轨已占用则顺延）

## 1. 结论

将 catalog 中 **`memory_pressure_relief`** 的 `in_m3: false` → **`true`**，纳入 `vole optimize` 主路径：

- **plan**：产出 1 条 action sentinel（对齐 `dock_refresh` / DNS 任务）；无文件删改
- **apply**：若内存压力非 warning/critical → **noop 成功**（对齐 Mole「already optimal」）；若高压 → 经既有 **`PrivilegeBackend` + `sudo -n purge`** 释放 inactive memory；无凭证 fail-closed → `NeedsPrivilege` + 响亮 `APPLY_PERMISSION_WARN`
- **可测**：`VOLE_TEST_MEMORY_PRESSURE=1|0` 强制高压/低压；`VOLE_TEST_NO_AUTH` / probe 失败绝不落真 `purge`

**禁止**本刀实现：`network_stack_optimize` / `disk_permissions_repair` / `periodic_maintenance` / `spotlight*` / `disk_verify` / `login_items_audit` / `shared_file_list_repair`；不碰 uninstall / clean。

## 2. 问题与风险

1. **sudo 交互挂起**：仅 `sudo -n`；禁止交互 `sudo purge`。
2. **第二套特权体系**：禁止新建；仅在 `PrivilegeBackend` 增加窄方法（如 `purge_inactive_memory`），复用 W2b① 的 `OptimizeApplyContext` / `ensure_privilege_ready`。
3. **与产品 `purge` 子命令混淆**：本刀是 optimize action 的 **`sudo purge`（内核 inactive pages）**，**不是** Mole `mo purge` 项目产物清理（W3 代际外）。
4. **压力探测失败**：`memory_pressure` 不可用或解析失败 → 视为非高压 → noop 成功（对齐 Mole `is_memory_pressure_high` 返回 1）。
5. **协议**：零 `schema_version` bump。

## 3. 采纳路径（单方案）

| 点 | 决策 |
|---|---|
| catalog | `memory_pressure_relief.in_m3 = true`；主路径计数 14 → **15** |
| plan | sentinel：`~/.vole-optimize-action/memory_pressure_relief`；`rule_id=optimize:action:memory_pressure_relief` |
| 压力门 | apply 时：`is_memory_pressure_high()`；非高压 → `Ok(())` 不调 sudo |
| 特权 | `PrivilegeBackend::purge_inactive_memory` → `sudo -n purge` |
| skip 语义 | 无特权且需 purge → `SkipReason::NeedsPrivilege` + 响亮提示 |
| coverage | optimize plan `coverage_note` 长尾自动不含「Memory Optimization」title |
| 版本 | **1.36.0** + release 短记（冲突则顺延） |

## 4. Mole 对照

| Mole | Vole |
|---|---|
| `is_memory_pressure_high`（`memory_pressure -Q` 匹配 warning\|critical） | 同逻辑；`VOLE_TEST_MEMORY_PRESSURE` 可注入 |
| dry-run 直接成功文案 | `--plan` 只出 sentinel，不测压力、不 purge |
| 非高压 → 「already optimal」return 0 | apply noop `Ok(())` |
| `optimize_sudo_available` + `sudo purge` | `ensure_privilege_ready` + `PrivilegeBackend::purge_inactive_memory` |
| 无特权警告 skip | `NeedsPrivilege` + `APPLY_PERMISSION_WARN` |

## 5. 产品行为

```bash
vole optimize --plan                              # 含 memory_pressure_relief sentinel
vole optimize --plan --task memory_pressure_relief
vole optimize --apply <plan.json>                 # 低压 noop；高压 sudo -n purge
```

无凭证且实际需要 purge 时：条目 skip，不静默「成功」。

## 6. 验收

- [ ] catalog：`memory_pressure_relief.in_m3`；单测主路径含之、仍不含 network_stack / spotlight*
- [ ] plan：默认扫描含 `optimize:action:memory_pressure_relief`；长尾 note 不含 Memory Optimization
- [ ] apply：RecordingPrivilege / NoPrivilege；低压不调 purge；高压无 sudo → NeedsPrivilege；高压 mock purge 成功
- [ ] 未实现 network_stack / disk_permissions / periodic / spotlight* / disk_verify / login_items / shared_file_list
- [ ] 版本 **1.36.0**（或顺延）；分支 `feat/optimize-memory-pressure`

## 7. 非目标

- W2b③ 及后置 optimize 长尾
- uninstall / clean / Mole 式 `purge` 子命令
- SMAppService / 新 Helper
- 扩大 `path_allowed_for_privilege` 删除 allowlist（本刀非删路径）
