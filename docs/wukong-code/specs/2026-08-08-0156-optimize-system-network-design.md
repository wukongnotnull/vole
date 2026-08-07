# W2b①：`system_maintenance` + `network_optimization` 设计

- 日期：2026-08-08
- 状态：已批准（Condensed；默认采纳推荐）
- 依据：Mole parity roadmap §2.3 W2b①；[`../../findings/2026-07-v2-m2-optimize-spike.md`](../../findings/2026-07-v2-m2-optimize-spike.md)；Mole `opt_system_maintenance` / `opt_network_optimization` / `flush_dns_cache`（`third_party/mole-1.48.1/lib/optimize/tasks.sh`）；`optimize/catalog.rs`
- 包版本意图：**1.31.0**（MINOR）

## 1. 结论

将 catalog 中 **`system_maintenance`** 与 **`network_optimization`** 的 `in_m3: false` → **`true`**，纳入 `vole optimize` 主路径：

- **plan**：各产出 1 条 action sentinel（对齐 `dock_refresh`）；无文件删改
- **apply**：经既有 **`PrivilegeBackend` + `sudo -n`** 刷新 DNS（`dscacheutil -flushcache` + `killall -HUP mDNSResponder`）；无凭证 fail-closed → `NeedsPrivilege` + 响亮 `APPLY_PERMISSION_WARN`
- **同 session 去重**：两任务共享一次 flush（对齐 Mole `MOLE_DNS_FLUSHED`）
- **`system_maintenance` 额外**：只读 `mdutil -s /` 校验 Spotlight（不 sudo、不改索引）

**禁止**本刀实现：`memory_pressure_relief` 及 W2b②③/后置项；不碰 uninstall / clean TM / status 快照。

## 2. 问题与风险

1. **sudo 交互挂起**：仅 `sudo -n`；`VOLE_TEST_NO_AUTH` / probe 失败绝不落真命令。
2. **第二套特权体系**：禁止新建；仅在 `PrivilegeBackend` 增加窄方法（如 `flush_dns_cache`），CLI 注入 `SudoNoninteractive`，复用 `ensure_privilege_ready`（TTY 至多一次 `sudo -v`）。
3. **双重 flush**：同 apply 若两任务均在 plan，必须只执行一次 DNS/mDNS 操作。
4. **误报成功**：任一步 `dscacheutil` / `killall -HUP` 失败 → 整次 flush 失败 → skip，不记 succeeded。
5. **协议**：零 `schema_version` bump。

## 3. 采纳路径（单方案）

| 点 | 决策 |
|---|---|
| catalog | 两 task `in_m3: true`；主路径计数 12 → **14** |
| plan | sentinel：`~/.vole-optimize-action/{task_id}`；`rule_id=optimize:action:…` |
| DNS 特权 | `PrivilegeBackend::flush_dns_cache` → `sudo -n dscacheutil -flushcache` && `sudo -n killall -HUP mDNSResponder` |
| optimize apply 接线 | `OptimizeApplyContext` 注入 `privilege` + `privilege_acquire_attempted`；对齐 clean `ensure_privilege_ready` |
| skip 语义 | 无特权 → `SkipReason::NeedsPrivilege`（勿再用 `PathVanished`） |
| coverage | optimize plan `coverage_note` 长尾列表自动不含这两 title；clean 全局 `coverage_note`「已落地」可补一句 DNS/mDNS optimize（可选，以实现 PR 为准） |
| 版本 | **1.31.0** + release 短记 |

## 4. Mole 对照

| Mole | Vole |
|---|---|
| `flush_dns_cache` + `optimize_sudo_available` | `PrivilegeBackend::flush_dns_cache` + `probe` / `sudo -n` |
| `MOLE_DNS_FLUSHED` | apply 上下文内 `dns_flushed: bool` |
| `opt_system_maintenance`：flush + `mdutil -s /` | 同；Spotlight 仅日志/人读，不进 plan 删路径 |
| `opt_network_optimization`：若已 flushed 则 noop 成功文案 | 已 flushed → 第二任务 succeeded（幂等） |
| dry-run 设 flushed 不执行 | `--plan` 不调用 flush |

## 5. 产品行为

```bash
vole optimize --plan                 # 含两 sentinel（及既有 12 主路径）
vole optimize --plan --task system_maintenance
vole optimize --apply <plan.json>    # sudo -n；失败 NeedsPrivilege + 响亮提示
```

无凭证时：条目 skip，不静默「成功」。

## 6. 验收

- [ ] catalog：两 id `in_m3`；单测主路径含二者、仍不含 `memory_pressure_relief`
- [ ] plan：默认扫描含两 `optimize:action:…`；长尾 note 不含其 title
- [ ] apply：RecordingPrivilege / NoPrivilege 单测；无 sudo → NeedsPrivilege；成功路径 mock flush
- [ ] 同 plan 两任务只调用一次 `flush_dns_cache`
- [ ] 未实现 memory_pressure / network_stack / disk_permissions / periodic / spotlight* / disk_verify / login_items / shared_file_list
- [ ] 版本 1.31.0；分支 `feat/optimize-system-network`

## 7. 非目标

- W2b②③ 及后置 optimize 长尾
- uninstall / clean TM / status 本地快照
- SMAppService / 新 Helper
- 扩大 `path_allowed_for_privilege` 删除 allowlist（本刀非删路径）
