# Handoff Pasteboard Cache 设计（Mole `clean_handoff_pasteboard_cache` 同形）

- 日期：2026-08-06
- 状态：待实现（设计已批准）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/user.sh` → `clean_handoff_pasteboard_cache`（#1178）；Vole `should_protect_path`（`group.com.apple.*` 在步骤 3 `data_protected`）；container stubs / group-container-caches 的形状门与 FDA 模式
- 包版本意图：能力扩展 → **`1.8.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole **同形**的 Handoff / Universal Clipboard 暂存清理：

- 根写死：`$HOME/Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard`
- 仅 **mindepth 1 / maxdepth 1** 叶节点；**mtime > 60 分钟**；非 symlink
- 因路径落在 `group.com.apple.*`，普通 `validate_path_for_deletion` 会被步骤 3 拦下 → 本规则用**窄形状门**在 plan/apply 豁免该校验（不动通用保护层）
- apply：形状门 + mtime 重验 + identity 后走普通 `mole_delete_verified`（默认废纸篓；`--permanent` 有效）

**采纳路径**：独立 custom 规则（不耦合进 `group-container-caches` 的 Apple 跳过逻辑）。

## 2. 问题与风险

1. **体量**：`useractivityd` 本应自剪，但重 Command+C / 跨设备剪贴板同步后可堆积到数百 GB（Mole #1178）。
2. **飞行中同步**：&lt;60 分钟内的条目可能仍在 Handoff 传输 → 必须对齐 Mole 的 `-mmin +60`。
3. **保护层冲突**：`extract_container_bundle_id` 得到 `group.com.apple.coreservices.useractivityd` → `should_protect_data`（`com.apple.*`）为真 → 步骤 3 早退。不得为此放开整棵 Apple 组容器。
4. **过期 / 篡改 plan**：apply 若不重验 mtime，可能删掉刚进入窗口的飞行中条目。
5. **FDA**：根不可列 → 须响亮降级。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 独立 custom + 窄形状豁免（已选）** | handler + plan/apply 形状门 | 边界清晰；不动通用保护 | 多一条规则与少量豁免代码 |
| B. 塞进 `group-container-caches` | Apple 特例分支 | 少一规则 | 与「跳过全部 Apple」打架，耦合 |
| C. 改 `is_explicit_clean_cache_path` | 标可再生 | 无 rule 豁免 | 放宽面大于单根；易误伤同容器其它路径 |
| D. 仅发现 | 硬跳过删除 | 最安全 | 不兑现体积价值 |

## 4. 产品行为

```bash
vole clean --plan                 # 可含 Handoff pasteboard 叶候选（>60min）
vole clean --apply <plan.json>    # 普通废纸篓删除；--permanent 有效
```

- `rule_id`：`handoff-pasteboard-cache`
- handler：`handoff_pasteboard_cache`
- `category`：`app-caches`
- 规则文件：`data/rules/app-caches.toml`（**不得**进 `zzz-orphaned.toml`）
- label：`Handoff pasteboard: <basename>`
- 规则数：515 → **516**
- **不 bump** `schema_version`
- 环境变量：本期不增（mtime 阈值写死 60 分钟）

## 5. 判定流水线（plan select）

1. 根不存在 → `Ok(空)`；根是 symlink → `Ok(空)`；根存在但 `read_dir` 失败 → degrade（§8）
2. 枚举根下**直接**子项（含隐藏，对齐 Mole `find` 默认可见性行为：不启用 `dotglob` 时 find 仍列出点文件以外的项；Vole `read_dir` 列出全部含点文件——与 Group Containers 叶清理一致，接受 `.` 隐藏项）
3. 跳过 symlink；`symlink_metadata` 失败跳过
4. `modified()` 成功且 `now - mtime > 60 minutes` 才入选（测试经注入 `now` / 固定 mtime）
5. plan 层：本 `rule_id` 用 `is_handoff_pasteboard_candidate_path` 代替 `validate_path_for_deletion`；仍走白名单 + `capture_plan_entry_identity`
6. 规模上限：整规则最多 **2000** 叶；超限 → 停止追加 + `PlanNotice::HandoffPasteboardTruncated`（不 degrade）

home 经 `VOLE_TEST_HOME` / 既有 `HOME` 注入。

## 6. 形状门（写死）

`is_handoff_pasteboard_candidate_path(path, home)`：

- 必须 `strip_prefix(home.join("Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard"))` 成功
- 相对路径恰好一个 `Component::Normal`（拒绝更深、`..`、根本身、家外路径）

apply 重验：`recheck_handoff_pasteboard_entry(path, home, now)` =

1. 形状门通过
2. 非 symlink 且仍存在
3. `now - mtime > 60 minutes`

然后 `verify_plan_entry`（identity）+ `mole_delete_verified`（**注意**：`mole_delete_verified` 内部仍会 `validate_path_for_deletion`）。因此 apply 必须：

- **要么**在进入 `mole_delete` 前对本规则走与 stubs 类似的旁路（仅 identity 校验 + 专用删除/或带「跳过 protect」的删除选项），
- **要么**给 `mole_delete_verified` / `verify_plan_entry_for_apply` 增加「已通过形状门」的窄旁路标志。

**采纳（写死）**：对齐 container stubs 的结构——`apply_plan` 对本 `rule_id` 早分支：`recheck_*` + `verify_plan_entry`（仅身份，**不**走 `verify_plan_entry_for_apply`）+ 调用 **普通** `mole_delete` 变体中**仍校验 protect** 的路径会失败。

更精确的实现约束：

> 现有 `mole_delete_verified` 总会 `validate_path_for_deletion`。对本规则，在 `recheck` 通过后调用一个**仅跳过 protect、仍做 TOCTOU / trash / permanent** 的删除入口（可复用 `mole_delete` 内部并加 `skip_protection: true`，且 `skip_protection` 仅允许在 recheck 已证明形状门的路径上打开）。**禁止**对任意路径开放该标志。

## 7. Apply

- `rule_id == handoff-pasteboard-cache` 早分支（不经 `verify_plan_entry_for_apply`）
- 顺序：`recheck_handoff_pasteboard_entry` → `verify_plan_entry`（identity）→ `mole_delete`（`skip_protection` 仅此路径）
- 默认废纸篓；`--permanent` 同形
- 失败 → skip（`PathVanished` 或既有映射）；**不**用 stubs 的 unlink+rmdir（这里可以是文件或目录叶项，应对齐 `safe_remove` / `mole_delete` 递归进废纸篓能力）

## 8. 权限降级

- 根不存在 / 是 symlink → 空结果，不 degrade  
- 根存在但不可读 → `CustomDegrade::HandoffPasteboardInaccessible` → `Skipped(TccDenied)` + `PlanNotice::HandoffPasteboardInaccessible` + 中文 FDA 警告  
- 规模截断 → `PlanNotice::HandoffPasteboardTruncated`（非 degrade）

## 9. 覆盖说明

- coverage：标明 **Handoff pasteboard（mtime>60min）已落地**；仍未移植保持真 sudo、受保护组容器缓存完整放行、Rosetta `/Library`、claude pending-uploads 等
- README：规则数 516；Mole 对比不改全家桶主句

## 10. 非目标

- 其它 Apple Group Containers / `group-container-caches` 的 Apple 跳过反转
- 修改 `should_protect_path` / `protection.toml` 通用逻辑
- Application Scripts orphan
- 真 sudo
- 删除 `shared-pasteboard` **目录本身**
- 可配置 mtime 阈值（本期写死 60）
- 新 `SkipReason` 变体

## 11. 测试与安全

- `>60min` 叶文件/目录 → 入选并可 apply 到废纸篓
- `<60min` → 不入选；plan 含过期条目时 apply 因 mtime 重验 skip
- 形状：根外、深层 `a/b`、`..`、symlink 叶 → 不入选；apply 拒绝
- 根 symlink / 缺失 → 空；根不可读 → degrade + FDA
- 白名单命中叶 → skip
- `protection::` / `safety::property` 零回归（通用保护未改）
- PR：**security-review** 必过（形状门 + `skip_protection` 仅限 recheck 后）

## 12. 验收

1. fixture 下可清理 >60min pasteboard 叶项（废纸篓）
2. 飞行窗口内条目不被 plan/apply 删除
3. 形状外路径永不因本规则删除
4. coverage / README 反映已落地
5. 规则数 516；发版 **1.8.0**

## 13. 实现后文档

- `docs/releases/v1.8.0.md`、findings、Formula
- 受保护组容器 Logs 完整对齐仍另开 design（与本刀无关）
