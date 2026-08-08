# optimize `spotlight_orphan_rules_cleanup` 设计（闸控轨 G2）

- 日期：2026-08-08 18:22
- 状态：已批准（用户明确「批准执行轨 G2」；本会话 design 落盘后直接实现）
- 依据：[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) §3.3 P2；计划 [`../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md) Task G2；Mole `opt_prune_spotlight_orphan_rules`（`third_party/mole-1.48.1/lib/optimize/tasks.sh` ≈832）；G1 范例 [`2026-08-08-1754-optimize-login-items-audit-design.md`](2026-08-08-1754-optimize-login-items-audit-design.md)
- 包版本意图：**1.43.0**（MINOR；相对当前 workspace `1.42.0`）
- **不 bump** `schema_version`

## 1. 结论

将 catalog 中 **`spotlight_orphan_rules_cleanup`** 的 `in_m3: false` → **`true`**，纳入 `vole optimize` 主路径（19 → **20**）：

| 阶段 | 行为 |
|---|---|
| **plan** | 读取用户域 `com.apple.spotlight` / `EnabledPreferenceRules`；按误伤模型分类；仅当存在可删 orphan 时产出 **1** 条 action 候选（label 含数量） |
| **apply** | 经 `defaults` 重写 keep 数组（或 `defaults delete` 空 keep）；**禁止**直接改 plist 文件绕过 cfprefsd |
| **失败** | 读失败 / `VOLE_TEST_NO_AUTH` → 空 plan / apply Skipped；写失败 → Failed；不确定是否已安装 → **保留**（fail-closed） |

**禁止**：静默大范围清空 Spotlight 规则；碰 `System.*` / `com.apple.*`；本轨不做 G3–G5 / D1。

## 2. 误伤模型与回滚 / skip

| 规则条目 | 处置 |
|---|---|
| `System.*` | **永久 keep** |
| `com.apple.*` | **永久 keep** |
| 非 reverse-DNS（`is_reverse_dns_bundle_id` 否） | **keep**（形状不明，不删） |
| reverse-DNS 且 `app_installed` = true | keep |
| reverse-DNS 且 `app_installed` = false（确认消失） | **remove** |
| reverse-DNS 且探测失败 / 不确定 | **keep**（fail-closed；比 Mole 略保守） |
| `EnabledPreferenceRules` 键缺失 / 读失败 | 已 clean → **0** 候选；不写 |

**回滚 / skip：**

- plan 预览：label `Would remove N orphan Spotlight rule(s)`；用户可不选该 entry
- apply 幂等：再扫一遍后写 keep；若已无 orphan → Ok noop
- `VOLE_TEST_NO_AUTH`：Live 不读不写系统 prefs；plan 空；若 plan 已含条目则 apply → `Skipped`
- 写失败：`Failed`（响亮）；不部分写

## 3. 采纳路径

| 点 | 决策 |
|---|---|
| catalog | `spotlight_orphan_rules_cleanup.in_m3 = true`；主路径 **20**；余 `spotlight_index_optimize` / `shared_file_list_repair` / `disk_verify` 仍 `false` |
| deps | 新模块 `optimize/tasks/spotlight_orphan_rules.rs`：`SpotlightOrphanDeps` + `Live` + `Fake` |
| list | Live：`defaults read com.apple.spotlight EnabledPreferenceRules`（失败→空/已 clean）；解析多行 / 数组文本 |
| installed | Live：`mdfind kMDItemCFBundleIdentifier` + `/Applications`/`~/Applications` Info.plist / LaunchServices helper 扫描（对齐 Mole `bundle_has_installed_app` 主路径）；探测 IO 失败 → **视为 installed（keep）** |
| rewrite | `defaults write … EnabledPreferenceRules -array keep…`；keep 空 → `defaults delete … EnabledPreferenceRules` |
| 候选 | 单条：`path=home/.vole-optimize-action/spotlight_orphan_rules_cleanup`；`size=0`；`task_id` 同上 |
| 接线 | `optimize_plan` `allow(...)`；`apply_optimize_action` 分支；coverage 补一句 |
| 版本 | **1.43.0** + `docs/releases/v1.43.0.md` + README / Formula |

## 4. Mole 对照

| Mole | Vole |
|---|---|
| `opt_prune_spotlight_orphan_rules` | `plan_` / `apply_` + deps |
| PlistBuddy 枚举 + `defaults write -array` | plan 用 defaults/读；apply 只用 defaults（cfprefsd） |
| `System.*` / `com.apple.*` keep | 同 |
| reverse-DNS + `!bundle_has_installed_app` → remove | 同；探测失败改为 keep |
| dry-run 只报告 | plan 候选；apply 才写 |
| 无 `MOLE_TEST_NO_AUTH` 特判 | `test_no_auth()` → Live 空 / Skipped |

## 5. 产品行为

```bash
vole optimize --plan
vole optimize --plan --task spotlight_orphan_rules_cleanup
vole optimize --apply <plan.json>   # 重写 keep 数组；无 orphan → noop
```

## 6. 测试策略

- **Catalog**：含 `spotlight_orphan_rules_cleanup`，`main.len()==20`，仍排除三长尾
- **plan**：Fake 注入 System/Apple/installed/orphan；断言仅 orphan 触发 1 候选；全健康 / 无键 → 0
- **分类纯函数**：`classify_spotlight_rules` 单测钉 keep/remove 集合
- **apply**：Fake 记录 `write_rules` / `delete_rules`；orphan → keep 不含 orphan；空 keep → delete；`test_no_auth` Live → Skipped
- **禁止**：单测触发真 `defaults write` 改开发机 Spotlight（一律 Fake）

## 7. 验收

- [ ] catalog `in_m3` + 单测 20
- [ ] plan/apply 接线 + hermetic GREEN
- [ ] coverage / README / Formula / `docs/releases/v1.43.0.md` / workspace **1.43.0**
- [ ] 未实现 G3–G5 / D1
- [ ] 单轨 PR，merge commit 合入

## 8. 非目标

- `spotlight_index_optimize`（G3）/ `shared_file_list_repair`（G4）/ `disk_verify`（G5）
- 直接编辑 `~/Library/Preferences/com.apple.spotlight.plist` 绕过 cfprefsd
- SMAppService / 桌面特权助手
