# Group Container Caches 设计（Mole `clean_group_container_caches` 同形）

- 日期：2026-08-06
- 状态：待实现（设计已批准）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/user.sh` → `clean_group_container_caches`；Vole `should_protect_path` / `is_container_cache_or_tmp`；container stubs 的 FDA degrade 模式；coverage「仍未移植：Group Containers 泛清理」
- 包版本意图：能力扩展 → **`1.7.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole **同形**的 Group Containers 可再生清理：

- 只扫 `$HOME/Library/Group Containers`
- **不删整容器**：候选是 Logs /（条件）Caches·tmp 下的**叶节点**
- `data_protected` 容器：仅清 Logs；非 protected：再清 tmp / Caches
- apply 走普通 `mole_delete_verified`（废纸篓 / `--permanent` 有效）
- 通过**收窄的「可再生路径」识别**扩展 `should_protect_path`，避免 data_protected 让 Logs（及条件性 Caches）空转；**不**放宽整容器保护，**不**做 stubs 式 carve-out

**采纳路径**：custom 规则 + 保护层可再生提示；非整目录 orphan，非硬跳过发现。

## 2. 问题与风险

1. **Group Containers 含跨应用共享数据**：误删整容器会毁掉沙盒共享状态。必须叶子删除 + Apple / Notes / OrbStack 硬保护。
2. **保护层空转**：现有 `is_container_cache_or_tmp` 只认沙盒 `…/Data/Library/Caches|tmp`；Group Containers 的 `…/Library/Caches`、`…/Logs` 对 `data_protected` 容器会在步骤 3 被拦死 → Mole「protected 仍清 Logs」无法落地。须**显式扩展可再生识别**，面积极小且可审计。
3. **Safari 扩展副作用**：清扩展组容器缓存可能唤醒 Safari（Mole 已跳过）。
4. **FDA**：无法列 `~/Library/Group Containers` → 须响亮降级。
5. **TeamID / 整容器 orphan**：Mole apps.sh 写死 NEVER 扫 Group Containers 做 orphan —— 本刀**明确不做**。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. custom + 可再生扩展（推荐，已选）** | handler 同形扫描 + protect 窄扩展 + 普通删除 | 对齐 Mole；无 carve-out | 改保护层需 security-review |
| B. 纯 TOML 通配 | 静态 paths | 简单 | 无法表达 protected 分歧 / Safari 跳过 |
| C. 仅发现硬跳过 | 对齐 system-services | 最安全 | 不兑现清理价值 |
| D. 整容器 orphan | 对齐 stubs | — | Mole NEVER；假阳性高 |

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                 # 可含 Group Containers logs/caches 叶候选
vole clean --apply <plan.json>    # 普通删除（默认废纸篓；--permanent 有效）
```

- `rule_id`：`orphaned-group-container-caches`
- handler：`orphaned_group_container_caches`
- label：`Group container cache: <container_id>/<relative>`
- **不 bump** `schema_version`
- 环境变量：本期不增；禁用走规则 `disabled = true`

### 4.2 扫描根（写死）

| 根 | 内容 |
|---|---|
| `$HOME/Library/Group Containers` | 顶层非 symlink、可读目录 |

**明确不扫**：`Containers` 整目录 orphan、Application Scripts、系统 `/Library`、整容器根删除。

## 5. 判定流水线（plan select）

对每个顶层容器目录，顺序：

1. 是目录且 **不是** symlink；**可读**（不可读 → 跳过该容器，不 degrade）
2. `container_id` **不是** `com.apple.*` / `group.com.apple.*` / `systemgroup.com.apple.*`
3. Safari Web Extension：若 `$HOME/Library/Containers/<container_id>/` 下存在匹配 `*Safari*` / `*safari*` 的条目 → 整容器跳过
4. `protected = should_protect_data(id) || should_protect_data(去 group. 前缀)`
5. 候选子树：
   - 恒有：`<dir>/Logs`、`<dir>/Library/Logs`
   - 仅 `!protected`：`<dir>/tmp`、`<dir>/Library/tmp`、`<dir>/Caches`、`<dir>/Library/Caches`
6. 每个候选子树：是目录且非 symlink；白名单命中整树跳过；枚举 **mindepth 1 maxdepth 1** 子项
7. 每个子项：非 symlink；非空存在；过 `validate_path_for_deletion`；未命中白名单 → 入 plan
8. 空子树跳过；子项 >100 仍枚举（尺寸可 partial，对齐 Mole 行为，不挡删除）

home 经 `VOLE_TEST_HOME` / 既有测试 home 注入覆盖。

## 6. 保护层扩展（审阅硬约束）

在 `should_protect_path` Cleanup 路径步骤 3（容器 bundle 提取）中，与沙盒 `Data/Library/Caches|tmp` 同形，将下列路径视为 **可再生**（`container_cache` / 等价标志 = true），**不**因 `should_protect_data(bundle)` 单独 return true：

| 条件 | 路径形状（均位于 `…/Library/Group Containers/<id>/`） |
|---|---|
| 任意（含 data_protected） | `Logs/**` 直接子项、`Library/Logs/**` 直接子项 |
| 仅 `!should_protect_data(id)` 且 `!should_protect_data(去 group.)` | `tmp/**`、`Library/tmp/**`、`Caches/**`、`Library/Caches/**` 的直接子项 |

实现建议：新增 `is_group_container_regenerable_path(path, catalog)`（或扩展现有 helper），**禁止**把 `<id>` 根或任意其它子树标可再生。

**仍强制保护（不受本扩展影响）**：

- OrbStack runtime（现有 `is_orbstack_runtime_path`）
- Notes / System Settings 等硬编码关键字
- Endpoint Security cache
- 白名单、关键用户路径、catalog cleanup 全路径匹配（可再生标志仅跳过步骤 3 的 data_protected 早退，后续步骤照常）

**禁止**：从 `protection.toml` 删除 `com.macpaw.*` 等；放宽全局 Group Containers 根保护；对本规则走 unlink+rmdir carve-out。

## 7. Apply

- **无** `rule_id` 早分支；叶节点走 `mole_delete_verified` / 既有 verify 闸口
- 默认废纸篓；`--permanent` 与其它 clean 规则同形
- 身份 TOCTOU / 白名单 / protect 重验保持现状

## 8. 权限降级

- `~/Library/Group Containers` 不可列 / 不存在可读失败 → 整规则 degrade：`CustomDegrade::GroupContainersInaccessible` → `Skipped(TccDenied)` + `PlanNotice` + 中文 FDA 警告（风格对齐 `ORPHAN_LIBRARY_WARN` / `CONTAINER_STUBS_WARN`）
- 根可读但无候选 / 部分容器不可读 → 正常空或部分结果，**不** degrade

## 9. 覆盖说明

- 全局 coverage：标明 **Group Containers logs/caches（Mole 同形）已落地**；仍未移植改为 **真 sudo 删除**、其它 sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）；**去掉**「Group Containers 泛清理」未移植措辞
- README：Mole 对比句不再把 Group Containers 泛清理当作「仅 Mole」要点

## 10. 非目标

- 整 Group Container 目录 orphan / TeamID 前缀泛删
- Application Scripts
- 真 sudo
- OrbStack runtime / Notes 容器内容
- `group.com.apple.contentdelivery` 单独重复规则（本刀扫描已覆盖其 Logs，若 id 被 Apple 前缀跳过则保持跳过——与 Mole 主扫描 case 一致；contentdelivery 属 `group.com.apple.*`，Mole 主函数也会 skip；App Support 扫描里的已知列表是**另一条路径**，本期不移植那条 duplicate）
- 新 `SkipReason` 变体
- stubs 式 protect 豁免 / 非 trash 删除

## 11. 测试与安全

- 非 Apple、非 protected fixture → Logs + Caches 叶项入选并可 apply
- `data_protected` 组容器 → **仅** Logs 叶项入选；Caches 不入选
- `group.com.apple.notes` / OrbStack 路径永不入选；property 保护回归不破
- Safari 扩展同 id → 整容器跳过
- 根不可读 → degrade + warn
- 保护函数单测：可再生形状正反例；容器根 / 其它子树仍 protected
- PR：**security-review** 必过（保护层扩展面）

## 12. 验收

1. plan/apply 在 fixture 下可清理非 protected 组容器 Caches 叶项与 protected 组容器 Logs 叶项
2. 保护层仍拒绝 Notes / OrbStack / 整容器根
3. coverage / README 反映已落地
4. 发版 **1.7.0**

## 13. 实现后文档

- `docs/releases/v1.7.0.md`、findings、Formula
- 扩范围（整容器 / Application Scripts）另开 design
