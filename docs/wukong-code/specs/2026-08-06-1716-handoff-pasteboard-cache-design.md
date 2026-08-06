# Handoff Pasteboard Cache 设计（Mole `clean_handoff_pasteboard_cache` 同形）

- 日期：2026-08-06（同日审阅修订：探针推翻「步骤 3 必拦 → 必须形状豁免」；本期改为**零保护层改动、零 plan/apply 豁免、零 skip_protection**）
- 状态：待实现（设计已批准；审阅修订已并入）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/user.sh` → `clean_handoff_pasteboard_cache`（#1178）；Vole `should_protect_path` 探针（见 §6）；group-container-caches 的 FDA / truncated 模式
- 包版本意图：能力扩展 → **`1.8.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole **同形**的 Handoff / Universal Clipboard 暂存清理：

- 根写死：`$HOME/Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard`
- 仅 **mindepth 1 / maxdepth 1** 叶节点；**mtime > 60 分钟**；非 symlink
- apply 走普通 `mole_delete_verified`（默认废纸篓；`--permanent` 有效）
- **本期不改保护层，不做形状豁免，不引入 `skip_protection`**：探针证实该根下叶项今天即可通过 `should_protect_path`（§6）

**采纳路径**：独立 custom handler；不耦合 `group-container-caches`；无 stubs 式 carve-out。

## 2. 问题与风险

1. **体量**：`useractivityd` 本应自剪，重 Command+C / 跨设备同步后可堆积到数百 GB（Mole #1178）。
2. **飞行中同步**：&lt;60 分钟内的条目可能仍在传输 → 必须对齐 Mole `-mmin +60`；**apply 必须重验 mtime**（防过期 plan）。
3. **保护层直觉错误**（审阅修订）：初稿以为 `group.com.apple.*` 必被步骤 3 拦住。实测步骤 3 对 **raw** `group.*` id 调用 `should_protect_data`（**不**剥 `group.`），`raw_dp=false`，故 `prot=false`。剥 `group.` 后 `strip_dp=true`，但现网代码路径用不到。**不要**按「必须豁免」来设计。
4. **同容器其它路径**：`…/useractivityd/other` 今天同样 `prot=false` → handler **必须**只扫 `shared-pasteboard`，禁止扫整容器。
5. **FDA**：根不可列 → 响亮降级。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A-. custom + 零豁免（已选，本期）** | handler 扫固定根；plan/apply 全走既有闸口 | 无新暴露面；实现最简 | 依赖现网「raw group. id 不命中步骤 3」行为 |
| A. custom + 形状豁免 + skip_protection | 对齐 stubs | 防御纵深 | **不必要**；扩大 apply 旁路面，security-review 更重 |
| B. 塞进 `group-container-caches` | Apple 特例 | 少规则 | 与「跳过全部 Apple」打架 |
| C. 改 `is_explicit_clean_cache_path` / 剥 group. 归一化 | 改保护层 | — | 超出本刀；可能改变大量 `group.com.apple.*` 行为 |

**修订理由**：探针已证明主要路径今天可删；引入豁免是解不存在的问题。若未来保护层改为对 `group.*` 剥前缀后再判（那是另一 design），再评估是否加形状门防御纵深。

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
- 环境变量：本期不增（60 分钟写死）

## 5. 判定流水线（plan select）

1. 根不存在 → `Ok(空)`；根是 symlink → `Ok(空)`；根存在但 `read_dir` 失败 → degrade（§8）
2. 枚举根下直接子项（`read_dir`，含隐藏项）
3. 跳过 symlink；`symlink_metadata` 失败跳过
4. `modified()` 成功且 `now - mtime > 60 minutes` 才入选（测试注入 `now` / 固定 mtime）
5. plan 层：**照常** `validate_path_for_deletion` + 白名单 + identity（无 rule_id 豁免分支）
6. 规模上限：整规则最多 **2000** 叶；超限停止追加 + `PlanNotice::HandoffPasteboardTruncated`

home 经 `VOLE_TEST_HOME` / 既有 `HOME` 注入。`now` 默认 `SystemTime::now()`；单测可注入。

## 6. 闸口矩阵（审阅探针实测）

`should_protect_path(..., Cleanup)`，路径均在 `$HOME/` 下：

| 形状 | 现状保护 | 说明 |
|---|---|---|
| `…/useractivityd/shared-pasteboard/item1` | **否** | raw bid=`group.com.apple.…` → `should_protect_data` false |
| `…/shared-pasteboard/.hidden` | **否** | 同上 |
| `…/shared-pasteboard/a/b` | **否** | 深层也不被步骤 3 拦（handler 不会提深层） |
| `…/useractivityd/other` | **否** | 故 handler 绝不可扫整容器 |
| `group.com.example.app/Caches/x` | **否** | 对照 |

若对 `bid.trim_start_matches("group.")` 再查，`com.apple.coreservices.useractivityd` 的 `should_protect_data` 为 **true**——这是**未来若归一化剥 `group.`** 时本路径会重新被拦的信号；届时需另开 design（形状门或 explicit-cache）。本期不预先加豁免。

**禁止**：修改 `should_protect_path` / `protection.toml`；对本规则做 protect 豁免、`skip_protection`、stubs 式 carve-out。

## 7. Apply

- **无** `rule_id` 早分支
- 在进入普通 `mole_delete_verified` **之前**，对本规则条目调用 `recheck_handoff_pasteboard_entry(path, home, now)`：
  1. 路径仍在 `shared-pasteboard` 根下且恰好单层名（**产品约束校验**，不是 protect 豁免）
  2. 非 symlink 且存在
  3. `now - mtime > 60 minutes`
- 重验失败 → skip（`PathVanished`）
- 通过后走既有 `verify_plan_entry_for_apply` + `mole_delete_verified`（protect + identity + trash 全开）

> 形状/根校验放在 apply 重验的目的：防止过期/篡改 plan 用本 `rule_id` 挂上同容器其它路径（`…/useractivityd/other`）——那些路径今天也可能 `prot=false`。这是**规则政策重验**，不是绕过保护层。

## 8. 权限降级

- 根不存在 / 是 symlink → 空结果，不 degrade
- 根存在但不可读 → `CustomDegrade::HandoffPasteboardInaccessible` → `Skipped(TccDenied)` + `PlanNotice::HandoffPasteboardInaccessible` + FDA 中文警告
- 截断 → `PlanNotice::HandoffPasteboardTruncated`（非 degrade）

## 9. 覆盖说明

- coverage：标明 **Handoff pasteboard（mtime>60min）已落地**
- 仍未移植：真 sudo、受保护容器的组容器缓存、Rosetta `/Library`、claude pending-uploads
- README：规则数 516

## 10. 非目标

- 其它 Apple Group Containers / 反转 `group-container-caches` 的 Apple 跳过
- 修改保护层（含「剥 group. 后再判 data_protected」的归一化）
- protect 豁免 / `skip_protection` / stubs 式删除
- Application Scripts / 真 sudo
- 删除 `shared-pasteboard` 目录本身
- 可配置 mtime 阈值
- 新 `SkipReason` 变体

## 11. 测试与安全

- `>60min` 叶 → 入选并可 trash apply
- `<60min` → 不入选；过期 plan 条目 apply 因 mtime 重验 skip
- 挂在 `…/useractivityd/other` 或深层路径的假 plan → apply recheck 拒绝（即使 prot=false）
- symlink 叶 / 根 symlink / 根缺失 → 不入选或空
- 根不可读 → degrade + FDA
- 白名单叶 → skip
- `protection::` / `safety::property` 零回归
- PR：**security-review** 必过（扫描面缩到单根 + apply 政策重验）

## 12. 验收

1. fixture 下可清理 >60min pasteboard 叶项
2. 飞行窗口内与规则根外路径不可因本规则删除
3. 保护层代码零 diff
4. coverage / README 反映已落地
5. 规则数 516；发版 **1.8.0**

## 13. 实现后文档

- `docs/releases/v1.8.0.md`、findings（含本节探针矩阵）、Formula
- 若日后保护层改为剥 `group.` 归一化导致本路径被拦，再开「形状门 / explicit-cache」design
