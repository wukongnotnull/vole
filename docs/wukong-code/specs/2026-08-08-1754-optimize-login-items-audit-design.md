# optimize `login_items_audit` 设计（闸控轨 G1）

- 日期：2026-08-08 17:54
- 状态：已批准（用户明确「批准执行轨 G1」；本会话 design 落盘后直接实现）
- 依据：[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) §3.3 P1；计划 [`../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md) Task G1；Mole `opt_login_items_audit`（`third_party/mole-1.48.1/lib/optimize/tasks.sh` ≈1442）；既有 uninstall [`2026-08-08-0953-uninstall-login-items-design.md`](2026-08-08-0953-uninstall-login-items-design.md)
- 包版本意图：**1.42.0**（MINOR；相对当前 workspace `1.41.0`）
- **不 bump** `schema_version`

## 1. 结论

将 catalog 中 **`login_items_audit`** 的 `in_m3: false` → **`true`**，纳入 `vole optimize` 主路径（18 → **19**）：

| 阶段 | 行为 |
|---|---|
| **plan** | 经可注入 `LoginItemsAuditDeps` 快照登录项并做存在性探测；**仅**对判定为 broken 的项产出 `optimize:action:login_items_audit` 候选（label 含「Broken… · remove via System Settings…」） |
| **apply** | **只确认、不删除**：对对应条目返回成功（幂等 noop）；**永不**调用 `LoginItemDeps::remove_login_item` / `bootout_helper` |
| **失败** | `VOLE_TEST_NO_AUTH` / Live 快照失败 → 不产破坏性动作；发出响亮 skip / 不可用 sentinel（见 §3），fail-closed |

**禁止**：默认弹 GUI 的非特权 `sfltool dumpbtm`；本轨不做 G2–G5 / D1；不改 uninstall 语义。

## 2. Audit vs Uninstall（语义边界）

| | `optimize` · `login_items_audit`（本轨） | `uninstall` · Login Items（已有） |
|---|---|---|
| 目的 | 发现损坏登录项并报告 | 卸载目标 app 时拆除其登录项 / helper |
| 删除 | **禁止** | `osascript` delete + `launchctl bootout` |
| 触发 | 用户跑 `optimize --plan/--apply` | 用户跑 `uninstall` 且 plan 含侧车 |
| rule_id | `optimize:action:login_items_audit` | `uninstall:login-item:name:…` / `uninstall:login-helper:…` |
| deps | **新** `LoginItemsAuditDeps`（只读） | 既有 `LoginItemDeps`（写） |

两者可共享只读探测思路，但 **模块与 trait 分离**，避免 audit 路径误接删除。

## 3. 采纳路径

| 点 | 决策 |
|---|---|
| catalog | `login_items_audit.in_m3 = true`；主路径 **19**；其余四条长尾仍 `false` |
| deps | 新模块 `optimize/tasks/login_items_audit.rs`：`LoginItemsAuditDeps` + `Live` + `Fake` |
| snapshot | Live：对齐 Mole `_login_items_snapshot`（System Events AppleScript → `name\tpath` 行） |
| exists | 对齐 Mole `_login_item_app_exists`：① item path 存在 → ② Spotlight `mdfind` 名变体 → ③ `/Applications` + `~/Applications` find/metadata → ④ **仅** `sudo -n true` 成功时才 `sudo -n sfltool dumpbtm` |
| **禁止** | 无活跃 sudo 会话时调用裸 `sfltool dumpbtm`（会弹管理员 GUI；Mole 注释已警示） |
| `VOLE_TEST_NO_AUTH` / `test_no_auth()` | Live：plan 返回空；apply 若命中 → `Skipped`；**永不**真 osascript / sudo / sfltool |
| 快照失败（非 test） | 产出 **1** 条 sentinel（path=`~/.vole-optimize-action/login_items_audit`，label 标明 Automation/System Events 不可用）；apply → `NeedsPrivilege`（响亮 skip，对齐 optimize 其它特权失败） |
| 全健康 / 无登录项 | plan **0** 条（对齐 Mole 仅 stdout 提示、无删改） |
| broken | 每项 1 候选：`path=home/.vole-optimize-action/login_items_audit/<percent-encoded-name>`；`size=0`；label 含 Broken + System Settings 指引 |
| apply | `match "login_items_audit"` → `Ok(())`（确认型）；不可用 sentinel → `NeedsPrivilege`（label 前缀或 path 等于 sentinel 根可区分，见实现） |
| 接线 | `optimize_plan` `allow("login_items_audit")`；`apply_optimize_action` 分支；coverage「已落地」补一句 audit（仍「仍未移植：桌面…」） |
| 版本 | **1.42.0** + `docs/releases/v1.42.0.md` + README / Formula |

## 4. Mole 对照

| Mole | Vole |
|---|---|
| `MOLE_TEST_NO_AUTH=1` → skip | `test_no_auth()` → Live 空 plan / apply Skipped |
| `_login_items_snapshot` | `LoginItemsAuditDeps::snapshot` |
| `_login_item_app_exists` + 条件 `sudo -n sfltool dumpbtm` | `app_exists`；BTM 仅 `sudo -n` |
| 仅打印 Broken，指引 System Settings | plan label 同文案；apply 不删 |
| 无独立 plan/apply | ProtoPlan 候选 + apply noop |

## 5. 产品行为

```bash
vole optimize --plan
vole optimize --plan --task login_items_audit
vole optimize --apply <plan.json>   # 对 broken 条目成功且不删；不可用 → NeedsPrivilege + 响亮提示
```

## 6. 测试策略

- **Catalog**：`m3_main_path_flags` 含 `login_items_audit`，`main.len()==19`，仍排除四条长尾
- **plan**：`FakeLoginItemsAuditDeps` 注入 broken / healthy / empty；断言候选数与 label；Live 在 `VOLE_TEST_NO_AUTH=1` 下为空
- **exists 纯函数**（可选）：path 命中 / 缺失时 Fake 可控，不调 mdfind
- **apply**：broken 条目 → succeeded；不可用 sentinel → NeedsPrivilege；断言 **零** `LoginItemDeps::remove_*` 调用（Fake uninstall 计数保持 0，或 audit Fake 无 remove API）
- **禁止**：单测触发真 `sudo` / 真 `osascript` / 非特权 `sfltool`

## 7. 验收

- [ ] catalog `in_m3` + 单测 19
- [ ] plan/apply 接线 + hermetic GREEN
- [ ] coverage / README / Formula / `docs/releases/v1.42.0.md` / workspace **1.42.0**
- [ ] 长尾 coverage_note 不再含「Login Items」title（自动随 `in_m3`）
- [ ] 未实现 G2–G5 / D1
- [ ] 单轨 PR，merge commit 合入

## 8. 非目标

- 自动删除损坏登录项（那是 uninstall / 用户 System Settings）
- `spotlight_*` / `disk_verify` / `shared_file_list_repair`
- SMAppService / 桌面特权助手
- 扩大 `LoginItemDeps` 写路径
