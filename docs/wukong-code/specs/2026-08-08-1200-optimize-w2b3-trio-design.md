# W2b③：optimize 长尾三件套设计

- 日期：2026-08-08
- 状态：已批准（Condensed；默认采纳推荐；安全面一致故 **同 PR**）
- 依据：Mole parity roadmap §2.3 W2b③；[`../../findings/2026-07-v2-m2-optimize-spike.md`](../../findings/2026-07-v2-m2-optimize-spike.md)；Mole `opt_network_stack_optimize` / `opt_disk_permissions_repair` / `opt_periodic_maintenance`（`third_party/mole-1.48.1/lib/optimize/tasks.sh`）；`optimize/catalog.rs`；W2b①/② 设计
- 包版本意图：**1.38.0**（MINOR；1.37 由并行 W2c 占用）

## 1. 结论

将 catalog 中下列三项的 `in_m3: false` → **`true`**，纳入 `vole optimize` 主路径（当前 15 → **18**）：

| task_id | plan | apply |
|---|---|---|
| `network_stack_optimize` | 1 条 action sentinel | VPN 活跃或网络健康 → noop `Ok`；否则 `sudo -n route -n flush` + `sudo -n arp -a -d` |
| `disk_permissions_repair` | 1 条 action sentinel | 无需修复 → noop `Ok`；否则 `sudo -n diskutil resetUserPermissions / <uid>` |
| `periodic_maintenance` | 1 条 action sentinel | `periodic` 缺失或 daily 日志未过期（&lt;7d）→ noop `Ok`；否则 `sudo -n periodic daily weekly monthly` |

共性（对齐 W2b①/②）：

- 经既有 **`PrivilegeBackend` + `sudo -n`**；TTY 可至多一次 `sudo -v`（`ensure_privilege_ready`）
- 无凭证且 **实际需要** 特权 → `NeedsPrivilege` + 响亮 `APPLY_PERMISSION_WARN`
- `--plan` 只出 sentinel，不探测、不落真命令

**禁止**本刀实现：`spotlight_*` / `disk_verify` / `login_items_audit` / `shared_file_list_repair`；不碰 uninstall / clean。

## 2. 问题与风险

1. **sudo 交互挂起**：仅 `sudo -n`；`VOLE_TEST_NO_AUTH` / probe 失败绝不落真命令。
2. **第二套特权体系**：禁止新建；仅在 `PrivilegeBackend` 增加三个窄方法（见 §3）。
3. **VPN / 路由误伤**：对齐 Mole `has_active_vpn_interface`（`scutil --nc` Connected + 默认路由 `utun*`）；VPN 活跃时整项 noop，不 flush。
4. **权限修复误触**：仅当 `needs_permissions_repair`（`$HOME` owner≠`$USER`，或 `~/` / `~/Library` / `~/Library/Preferences` 不可写）才调用 `diskutil`。
5. **`periodic` 已移除的系统**（macOS 26+）：命令不存在 → noop 成功文案（对齐 Mole skip）。
6. **协议**：零 `schema_version` bump。

## 3. 采纳路径（单方案 · 同 PR）

| 点 | 决策 |
|---|---|
| catalog | 三 task `in_m3: true`；主路径 **18** |
| plan | sentinel：`~/.vole-optimize-action/{task_id}`；`rule_id=optimize:action:…` |
| 特权方法 | `flush_network_stack`；`reset_user_permissions(uid)`；`run_periodic_maintenance` |
| 门控 | apply 时探测；仅「需要执行特权命令」才 `ensure_privilege_ready` |
| skip 语义 | 需特权无凭证 → `SkipReason::NeedsPrivilege` |
| 测试注入 | `VOLE_TEST_VPN_ACTIVE`；`VOLE_TEST_NETWORK_STACK_UNHEALTHY`；`VOLE_TEST_DISK_PERMISSIONS_NEED_REPAIR`；`VOLE_TEST_PERIODIC_STALE` / `VOLE_TEST_PERIODIC_LOG`；`VOLE_TEST_PERIODIC_AVAILABLE` |
| coverage | optimize plan 长尾 note 自动去掉三 title；全局 `coverage_note`「已落地」补一句 |
| 版本 | **1.38.0** + release 短记 |

### 3.1 Mole 对照

| Mole | Vole |
|---|---|
| `has_active_vpn_interface` + route/dns 健康检查 | 同语义；可用环境变量强制 VPN/不健康 |
| `sudo route -n flush` + `sudo arp -a -d` | `PrivilegeBackend::flush_network_stack`（两步均成功才 `Ok`；对齐 Mole：route 成功但 arp 失败仍算部分成功 → Vole **要求两步皆成功**，任一失败 → `Failed`，更保守） |
| `needs_permissions_repair` + `diskutil resetUserPermissions / $uid` | 同；uid = 当前 `geteuid()` |
| `periodic` 存在性 + `/var/log/daily.out` mtime ≥7d | 同；`VOLE_TEST_PERIODIC_LOG` 可指向测试文件 |
| dry-run 成功文案 | `--plan` 只 sentinel |

> **网络栈成功判定**：Mole 在 route 成功、arp 失败时仍 `return 0`。Vole 选 **两步皆成功**（与 DNS flush 一致的 fail-closed）；文档与单测写死。

## 4. 产品行为

```bash
vole optimize --plan
vole optimize --plan --task network_stack_optimize
vole optimize --plan --task disk_permissions_repair
vole optimize --plan --task periodic_maintenance
vole optimize --apply <plan.json>   # 门控后 sudo -n；失败 NeedsPrivilege + 响亮提示
```

无凭证且实际需要特权时：条目 skip，不静默「成功」。

## 5. 验收

- [ ] catalog：三 id `in_m3`；主路径长度 18；仍不含 spotlight* / disk_verify / login_items / shared_file_list
- [ ] plan：默认扫描含三 `optimize:action:…`；长尾 note 不含其 title
- [ ] apply：Recording / NoPrivilege；门控 noop 不调特权；需特权无 sudo → NeedsPrivilege；mock 成功路径
- [ ] 未实现 spotlight* / disk_verify / login_items_audit / shared_file_list_repair
- [ ] 版本 **1.38.0**；分支 `feat/optimize-w2b3-trio`
- [ ] 合入后小 PR 更新 0119：W2b③ 完成；下一刀写死（spotlight* 后置长尾，或 W2c）

## 6. 非目标

- spotlight_* / disk_verify / login_items_audit / shared_file_list_repair（保持长尾）
- uninstall / clean 无关面
- SMAppService / 新 Helper
- 扩大 `path_allowed_for_privilege` 删除 allowlist（本刀非删路径）
