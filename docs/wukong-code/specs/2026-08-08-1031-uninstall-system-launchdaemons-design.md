# uninstall 系统 LaunchDaemons / `/Library` sudo 残留设计（W2a③）

- 日期：2026-08-08
- 状态：已批准（盘点 Condensed；用户指示默认采纳）→ writing-plans
- 依据：路线图 [`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md) §2.3 W2a③；[`../../findings/2026-07-v2-m1-uninstall.md`](../../findings/2026-07-v2-m1-uninstall.md)；Mole `third_party/mole-1.48.1/lib/core/app_protection.sh`（`find_app_system_files`）+ `lib/uninstall/batch.sh`（`stop_launch_services` / `mole_delete` sudo）；已落地 W2a① [`2026-08-08-0155-uninstall-brew-cask-design.md`](2026-08-08-0155-uninstall-brew-cask-design.md)、W2a② [`2026-08-08-0953-uninstall-login-items-design.md`](2026-08-08-0953-uninstall-login-items-design.md)；clean PrivilegeBackend（`sudo -n` + TTY `sudo -v`）
- 包版本意图：**1.35.0**（MINOR；若与并行轨冲突则 rebase 顺延）
- **不 bump** `schema_version`

## 1. 结论

在既有 `vole uninstall` plan→apply 上交付 **系统 LaunchDaemons / `/Library` sudo 残留卸载主路径**：

1. **plan**：对通过保护筛选且 **无 sibling** 的 app，扫描 Mole 对齐的系统残留主路径，产出侧车条目  
   `rule_id = uninstall:system-leftover:{kind}:{token}`（path 为绝对系统路径）
2. **apply**：复用现有 **`PrivilegeBackend`**（`SudoNoninteractive` / Fake）：TTY 下可至多一次 `acquire_interactive`（`sudo -v`），再 `sudo -n`；LaunchAgent/Daemon plist 先 `launchctl_unload`，再 `remove_permanent`；无凭证 → **`NeedsPrivilege` + 响亮 skip**，不阻塞同 plan 其余条目
3. **保护层不绕过**：仍走 Uninstall 保护 / TOCTOU / whitelist / `path_allowed_for_privilege`；**禁止**第二套特权实现
4. **sibling**：有存活同 bundle sibling → **整刀系统残留不进 plan**（对齐用户域 leftovers / Mole 共享残留守卫）

**不在本刀**：SMAppService 常驻 Helper（W3）；Mole `find_app_system_files` 广谱边缘（Frameworks / kext / Plug-Ins / Input Methods / Audio / QuickLook / PreferencePanes / Screen Savers / Extensions / StartupItems / vendor-nested 深路径 / `/Users/Shared` / Raycast 特例）— **coverage 诚实记「边缘未移植」**；clean/optimize/status 无关改动。

**采纳路径**：新模块 `vole-core::system_leftovers`（发现 + rule_id 编解码）+ uninstall plan/apply 窄接线；PrivilegeBackend allowlist **按需扩展**到本刀允许的 `/Library` 叶与 receipts；测试 Fake + `VOLE_TEST_SYSTEM_LIBRARY` fixture。

## 2. Condensed 方案（已选 · ≤5 点）

1. 发现对齐 Mole 主路径：LaunchDaemons/Agents（bundle_id 边界 + 名守卫 glob）、PHT（bundle_id 边界）、窄 `/Library` exact 叶（Application Support / Preferences / Caches / Logs / Receipts）+ `/private/var/db/receipts` 边界。
2. plan 侧车 `uninstall:system-leftover:…`；sibling → 零系统残留条目。
3. apply 复用 PrivilegeBackend：`ensure_privilege_ready` 同形（probe → TTY `sudo -v` 一次 → `sudo -n`）；plist unload + permanent 删。
4. 无凭证 / allowlist 拒绝 / 保护命中 → NeedsPrivilege 或既有 skip，响亮中文提示。
5. 发版 **1.35.0**；coverage/README/findings 去掉 W2a③ 长尾；0119 路线图另开小 PR。

## 3. 问题与风险

1. **误删系统服务**：plan 篡改 + 提权面大。必须：绝对路径、禁 `..`、**特权 allowlist**、`com.apple.*` 永不删、apply 再校验形状与命名锚定、identity TOCTOU。
2. **无 sudo 凭证**：不得阻塞等密码；skip + 提示可先 `sudo -v`（对齐 clean system-services）。
3. **废纸篓不可用**：系统路径写死 **permanent**（即使用户默认 trash）。
4. **可读子集 vs sudo 发现**：plan **不**用 sudo 扩扫；root 拥有不可列目录 → 漏检可接受，coverage 说明；与 clean orphan 契约一致。
5. **名 glob 误伤**：`*$app_name*.plist` 须 Mole 同形守卫（名 ≥5、非 COMMON_WORDS、跳过 `com.apple.*`）。
6. **协议**：靠 `rule_id` 编码；**不**改 PlanEntry 字段、不 bump schema。

## 4. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 侧车 + PrivilegeBackend（已选）** | system-leftover rule_id；apply 调既有特权 | 可测、零第二套特权、对齐 W2a①② | allowlist 需窄扩 |
| B. 仅 coverage / 指引 Mole | 文档 | 零风险 | 不兑现 W2a③ |
| C. 新 SMAppService Helper | 桌面提权 | 体验好 | W3 延后；本刀禁止 |

## 5. 产品行为

```bash
vole uninstall --plan …              # 可读系统残留 → system-leftover 侧车
vole uninstall --plan "Some App" …
sudo -v                              # 可选：缓存凭证
vole uninstall --apply <plan.json>   # 有 -n 凭证 → unload + permanent；否则 NeedsPrivilege
```

- label 示例：`System LaunchDaemon: com.example.helper.plist`、`System leftover: /Library/Application Support/Foo`
- coverage：`system_leftovers=N`；Long-tail **去掉** system LaunchDaemons / `/Library` sudo；可保留「Mole 广谱边缘未移植」一句
- **不**改 brew / login-item 行为

## 6. 发现范围（主路径写死）

输入：`AppIdentity` + `SiblingPresence`；有 sibling → `[]`。

测试根：`VOLE_TEST_SYSTEM_LIBRARY` 映射 `/Library`；receipts 可用同 fixture 下 `../private/var/db/receipts`（与 privilege 模块既有映射风格一致）或 live `/private/var/db/receipts`。

### 6.1 必做（主路径）

| 类 | 规则 |
|---|---|
| LaunchAgents / LaunchDaemons | reverse-DNS bundle_id：`{id}.plist` / `{id}.*.plist`（点边界）；名 glob：`display_name` ≥5、非 COMMON_WORDS、basename 非 `com.apple.*` |
| PrivilegedHelperTools | reverse-DNS：basename bundle_id 边界前缀；非 `com.apple.*` |
| `/Library` exact 叶 | 对 `naming_variants`：`Application Support/{v}`、`Preferences/{v}`、`Preferences/{v}.plist`、`Caches/{v}`、`Logs/{v}`、`Receipts/{bundle_id}.bom`、`Receipts/{bundle_id}.plist`；**单层叶**；存在才入选；禁空名落到目录根 |
| receipts | `/private/var/db/receipts` maxdepth 1：basename bundle_id 边界；reverse-DNS 才扫 |

### 6.2 边缘（本刀不做 · coverage）

Frameworks、Internet Plug-Ins、Input Methods、Audio Plug-Ins、QuickLook、PreferencePanes、Screen Savers、Extensions、StartupItems、vendor-nested 深路径、`/Users/Shared`、Raycast 特例、BOM 载荷广扫（`find_app_receipt_files` 深挖）。

## 7. Plan 接线

对每个通过保护筛选的 app：

1. 既有 siblings / brew / login / 用户域 leftovers 不变  
2. `find_system_leftovers(identity, siblings)` → hits  
3. 每条：`rule_id = uninstall:system-leftover:{kind}:{token}`  
   - `kind` ∈ `launchd` \| `pht` \| `library` \| `receipt`  
   - `token` = 绝对 path 的 URL-safe 百分号编码（与 login-item name token 同形，保证单行可解析）  
   - `path` = 绝对系统路径；`size` = 尽力 `du` 或 0  
4. 保护命中的路径 **不进 plan**（`should_protect_from_uninstall` / path protection）  
5. 计数 `system_leftovers=N`

## 8. Apply 接线

在 schema / TTL / 保护 / TOCTOU 之后：

| rule_id 前缀 | 行为 |
|---|---|
| `uninstall:system-leftover:` | 解码 path；形状 + allowlist；sibling 再检；`ensure_privilege_ready`；若 launchd plist → `launchctl_unload`（best-effort）；`remove_permanent` |
| 其它 | 现有 brew / login / mole_delete |

约束：

1. **复用** `PrivilegeBackend`（注入 `UninstallApplyContext::privilege`，默认 `SudoNoninteractive`）；**禁止**新 sudo 封装  
2. `ensure_privilege_ready` 与 clean 同形：probe →（TTY 且未尝试过）`acquire_interactive` → 再 probe；失败 → NeedsPrivilege + 中文「需要非交互 sudo（可先执行 sudo -v）」  
3. `VOLE_TEST_NO_AUTH=1` → 永不真 sudo  
4. allowlist 拒绝 → Refused → skip（非 silent success）  
5. PathVanished → 既有 skip  
6. **永不**对系统路径走用户 Trash  
7. apply 再校验：path 仍锚定 bundle_id / naming variant / 名守卫；sibling 出现 → skip

## 9. Privilege allowlist 扩展

在既有三树单层叶之上，**仅**为本刀增加：

- `/Library/Application Support/<single>`（非空、非 `com.apple.*` 前缀名按安全策略）  
- `/Library/Preferences/<single>`  
- `/Library/Caches/<single>`  
- `/Library/Logs/<single>`  
- `/Library/Receipts/<single>`  
- `/private/var/db/receipts/<single>`（basename 须 bundle_id 边界；调用方保证）

Launch*/PHT 规则不变（`.plist` / 非 `com.apple.*`）。Backend 内仍二次断言绝对路径 + allowlist。

## 10. 覆盖说明 / 文档

- uninstall coverage：Long-tail **不再**列 system LaunchDaemons / `/Library` sudo；可含 `system_leftovers=N`；一句边缘未移植  
- `coverage.rs` / README：诚实写本能力已落地（1.35.0）  
- findings `2026-07-v2-m1-uninstall.md`：③ 标完成  
- Formula / workspace：**1.35.0**  
- **不在本 PR**改 0119（合入后小 PR：W2a③ 完成；下一刀写死 W2b② `memory_pressure_relief` 或 W2c 续刀）

## 11. 测试与安全

1. fixture `VOLE_TEST_SYSTEM_LIBRARY`：LaunchDaemon plist + PHT + App Support 叶 → plan 含对应 rule_id  
2. sibling → 零 system-leftover  
3. `com.apple.*` / 保护 app → 不进 plan  
4. Fake Privilege：unload + remove 各一次；denying → NeedsPrivilege  
5. allowlist 外路径 → 拒绝  
6. 名 glob：短名 / COMMON_WORDS → 不匹配无关 plist  
7. PR：**security-review**（系统删路径）必过；合并 `gh pr merge --merge --delete-branch`

## 12. 验收

1. 单测全绿；保护 / brew / login 不回归  
2. 版本 **1.35.0**；无凭证 skip 响亮  
3. W3 Helper / 边缘广谱 / 无关 clean·optimize **未改**  

## 13. 下一步

`writing-plans` → `docs/wukong-code/plans/2026-08-08-1032-uninstall-system-launchdaemons.md` → 在本分支实现。
