# uninstall login items 设计（W2a②）

- 日期：2026-08-08
- 状态：已批准（盘点 Condensed；用户指示默认采纳）→ writing-plans
- 依据：路线图 [`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md) §2.3 W2a②；[`../../findings/2026-07-v2-m1-uninstall.md`](../../findings/2026-07-v2-m1-uninstall.md)；Mole `third_party/mole-1.48.1/lib/uninstall/batch.sh`（`remove_login_item` / `discover_login_item_helper_bundle_ids` / `bootout_login_item_helpers`）；已落地 W2a① [`2026-08-08-0155-uninstall-brew-cask-design.md`](2026-08-08-0155-uninstall-brew-cask-design.md)
- 包版本意图：**1.34.0**（MINOR；若与并行轨冲突则 rebase 顺延）
- **不 bump** `schema_version`

## 1. 结论

在既有 `vole uninstall` plan→apply 上交付 **Login Items 卸载联动**（用户域、无 sudo）：

1. **Classic Login Items**：按显示名从 System Events 删除（对齐 Mole `remove_login_item` / osascript）
2. **Embedded LoginItems helpers**：扫描 `Contents/Library/LoginItems/*.app` 的 CFBundleIdentifier，对 `gui/$uid/$id` 做 `launchctl bootout`（对齐 `discover_*` + `bootout_login_item_helpers`）
3. **sibling 守卫**（对齐 Mole）：
   - 同 bundle 有存活 sibling → **禁止** helper bootout
   - display name 与存活 sibling 冲突（`guard_login`）→ **禁止** 按名删 Login Item
4. **保护层不绕过**：仍走既有 Uninstall 保护 / TOCTOU / whitelist；login-item 动作是 app 条目 apply 的 side-effect，不另开删除漏斗绕过
5. **失败策略**：无 sudo 能做的先做；osascript TCC / Automation 失败 → **NeedsPrivilege**（或等价响亮 skip）+ 可读提示，**不**阻塞同 plan 其余条目；`launchctl bootout` best-effort（失败不计整规则 failed）

**不在本刀**：系统 LaunchDaemons、`/Library/**`（W2a③）；`stop_launch_services` 对 `/Library` 的 unload；现代 SMAppService 强删（Mole 亦仅 detect+warn）；clean/optimize/status。

**采纳路径**：新模块 `vole-core::login_items` + `LoginItemDeps` 可注入；plan 用 **token 编码的侧车条目** 暴露动作；apply fail-closed + 可测 Fake。

## 2. 问题与风险

1. **按名误删 survivor 的 Login Item**：display name 冲突时必须跳过（Mole `guard_login`）。
2. **bootout 误停 survivor helper**：同 bundle sibling 时 helper id 相同 → 跳过 bootout。
3. **`com.apple.*` 命名空间**：helper Info.plist 自称 Apple id 时 **永不** bootout。
4. **AppleScript / Automation TCC**：真机可能弹权限；注入 Fake；生产失败 → 响亮 skip，继续删 app/leftovers。
5. **协议**：靠 `rule_id` 编码；**不**改 PlanEntry 字段、不 bump schema。
6. **路径语义**：侧车条目的 `path` 仍指向待卸 `.app`（便于 TOCTOU / 保护校验）；动作为非删文件，apply 分支识别 token 后 **不** `mole_delete` 该动作本身。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 侧车 plan 条目 + LoginItemDeps（已选）** | plan 写 `uninstall:login-item:…` / `uninstall:login-helper:…`；apply 解析后调 deps | 可见、可测、对齐 W2a① token 模式 | 条目多一点 |
| B. 仅 app apply 隐式 side-effect | 无新 rule_id | 短 | plan 不可见；难测顺序 |
| C. 只写 coverage / README | 文档对齐 | 零风险 | 不兑现 W2a② |

## 4. 产品行为

```bash
vole uninstall --plan …              # 有 LoginItems / 可卸 login item 时出现侧车条目
vole uninstall --plan "Some App" …
vole uninstall --apply <plan.json>   # 侧车 → osascript / launchctl；其余仍 brew 或 mole_delete
```

- label 示例：`Login Item: Foo`、`LoginItems helper: com.example.helper`
- coverage：`login_items=N`；Long-tail **去掉** login items；保留 system LaunchDaemons、`/Library` sudo
- SMAppService 残留：仍提示用户去「系统设置 → 通用 → 登录项与扩展」（可在 coverage / 失败 skip 文案出现一句，非协议字段）

## 5. Plan 接线

对每个通过保护筛选的 app（与 brew/leftovers 同层）：

1. 计算 `siblings`（既有 `find_bundle_siblings`）
2. `login_name_collides`：若存在 sibling，且本 app `display_name`（大小写不敏感）与任一 survivor 显示名/路径 stem 相等 → true（对齐 Mole 名冲突子集；本刀不实现完整 Mole 发现名收缩）
3. 若 **不** `login_name_collides` → 追加侧车：
   - `rule_id = uninstall:login-item:name:{token}`
   - `token` = 去掉 `.app` 的 display_name，经 **URL-safe 百分号编码**（保留可读 ASCII；空格→`%20` 等），保证 rule_id 单行可解析
   - `path` = app_path；`size` = 0
4. 若 **不** `siblings.has_siblings()` → `discover_login_item_helper_bundle_ids(app_path)`，对每个合法 reverse-DNS 且非 `com.apple.*` 的 id 追加：
   - `rule_id = uninstall:login-helper:{bundle_id}`
   - `path` = helper `.app` 路径（若可得）否则主 app_path；`size` = 0
5. 本体 / brew / leftovers 规则不变；**禁止**因 login-item 侧车绕过保护

计数：`login_items` = 侧车条数（name + helpers）。

## 6. Apply 接线

对每条 entry（schema / TTL / 保护 / TOCTOU 与现网一致之后）：

| rule_id 前缀 | 行为 |
|---|---|
| `uninstall:login-item:name:` | 解析 token → 解码名 → `LoginItemDeps::remove_login_item(name)`；成功→succeeded；TCC/权限→NeedsPrivilege skip；其它失败→skip（不 failed 整批） |
| `uninstall:login-helper:` | 解析 bundle_id；校验 reverse-DNS 且非 `com.apple.*`；`LoginItemDeps::bootout_helper(uid, id)`；失败 best-effort skip |
| `uninstall:brew-cask:…` / 其它 `uninstall:` | 现有路径 |

约束：

1. apply **再校验** sibling / name-collision（plan 不可信）：若现检 sibling 冲突 → skip，不调 deps
2. helper apply 时 helper `.app` 可能已被本体删除：bootout **不依赖**文件仍在；以 rule_id 内 bundle_id 为准；path TOCTOU 若 PathVanished → 仍允许 bootout（或跳过 verify 文件存在对 login-helper 分支）—— **推荐**：login-helper / login-item 分支在 verify 失败且原因为 PathVanished 时仍执行动作（名/id 来自 rule_id）；其它 verify 失败仍 skip
3. **永不**对 login 侧车调用 `mole_delete`（避免把主 app 因 side-effect 条目删两次或误删 helper 外路径）
4. Fake deps 单元测顺序与计数；不下真 osascript/launchctl

## 7. `LoginItemDeps`

```rust
pub trait LoginItemDeps {
    fn remove_login_item(&self, display_name: &str) -> Result<(), LoginItemError>;
    fn bootout_helper(&self, uid: u32, helper_bundle_id: &str) -> Result<(), LoginItemError>;
}

pub enum LoginItemError {
    NeedsPrivilege, // TCC / Automation
    Failed(String), // best-effort → skip
}
```

生产：`LiveLoginItemDeps`（`osascript` + `launchctl bootout gui/$uid/$id`，超时有界）。  
测试：`FakeLoginItemDeps` 记录调用。

纯函数（无 I/O）：`discover_login_item_helper_bundle_ids`、rule_id 编解码、`login_name_collides`。

## 8. 覆盖说明 / 文档

- uninstall plan coverage：Long-tail **不再**列 login items；保留 system LaunchDaemons、`/Library` sudo
- `coverage.rs` / README：诚实写 login items 已落地
- findings `2026-07-v2-m1-uninstall.md`：② 标完成
- Formula / workspace：**1.34.0**
- **不在本 PR**改 0119 路线图状态（合入后再开一小 PR）

## 9. 测试与安全

1. Discover：fixture `Contents/Library/LoginItems/Helper.app` + Info.plist → 抽出 bundle id  
2. `com.apple.*` helper → 不进 plan / apply 不 bootout  
3. sibling + 同名 → 无 `login-item:name` 条目；有 sibling → 无 helper 条目  
4. Fake：remove 调一次正确名；bootout 调一次 `gui/uid/id`  
5. NeedsPrivilege → skip + SkipReason::NeedsPrivilege；同 plan 其它条目仍 succeeded  
6. 非法 rule_id / 非 reverse-DNS → skip  
7. Safari / 保护 app → 仍不进 plan（含侧车）  
8. PR：**security-review**；合并 `gh pr merge --merge`

## 10. 验收

1. 单测全绿；保护单测不回归  
2. 版本 **1.34.0**；W2a③ / W2b / W2c / clean 未改动  
3. 真机可选：带 Login Item 的第三方 app `uninstall --plan` 见侧车；`--apply` 后 Login Items 列表无该项（需 Automation 权限）

## 11. 下一步

`writing-plans` → `docs/wukong-code/plans/YYYY-MM-DD-HHmm-uninstall-login-items.md` → branch `feat/uninstall-login-items` 实现。
