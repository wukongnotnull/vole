# Protected Group Container Logs 设计（补齐 1.7.0 覆盖缺口）

- 日期：2026-08-06
- 状态：待实现（设计已批准）
- 依据：`2026-08-06-1133-group-container-caches-design.md` §6/§10/§13（明确 defer 的保护层扩展）；Mole `clean_group_container_caches`（protected → 仅 Logs）；`should_protect_path`（`crates/vole-core/src/protection/path.rs`）步骤 3 / 7
- 包版本意图：能力扩展 → **`1.9.0`**（SemVer MINOR）

## 1. 结论

在 **保护层** 对 Group Containers 下的 **Logs 可再生叶** 做形状豁免，使 1.7.0 已落地的 `group-container-caches` 能真正清到：

1. **无 `group.` 前缀且 `data_protected` 的容器**（如 `com.macpaw.CleanMyMac`）的 Logs 叶——今天被步骤 3 早退拦住
2. **Logs 叶文件名命中 bundle guard**（如 `com.docker.helper.log`）——今天被步骤 7 拦住

**不动 handler / 规则 TOML / apply rule_id 旁路**：handler 对 protected 本来就只提 Logs；本期只让保护层与之一致。

**采纳路径**：保护层形状豁免（方案 A）；非整容器放行；非 stubs 式 `skip_protection`；不把 Caches/tmp 对 protected 一并放行。

## 2. 问题与风险

1. **扩大放行面**：改 `should_protect_path` 影响所有 clean 规则，不只 `group-container-caches`。形状必须写死，禁止泛化到 Application Support / Containers / Caches。
2. **篡改 plan**：若形状豁免过宽，攻击者可把 `com.macpaw.*/Caches/...` 塞进 plan。故 **Caches/tmp 对 data_protected 必须仍拦**（handler 不提 + 保护层双保险）。
3. **Notes / OrbStack**：必须继续拦。步骤 1 关键字与顶部 `is_orbstack_runtime_path` **先于**步骤 3，形状豁免不得前移或短路它们。
4. **顶层 `Logs/` vs `Library/Logs/`**：现网 `is_explicit_clean_cache_path` 只认 `/Library/Logs/`，不认 Group Containers 顶层 `/Logs/`——步骤 7 缺口主要来自后者；两者本期一并覆盖。
5. **与 handoff（1.8.0）交互**：handoff 根在 `group.com.apple.coreservices.useractivityd/shared-pasteboard`，不在 Logs 形状内；本刀零影响。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 保护层 Logs 形状豁免（已选）** | `is_group_container_logs_path`；步骤 3 视同 container_cache；步骤 6/7 经 explicit 放行 | 与既有 `container_cache` / `is_explicit_clean_cache_path` 同形；篡改 plan 时 Caches 仍拦 | 改全局保护层，需 security-review |
| B. rule_id / stubs 式旁路 | plan/apply 对 `group-container-caches` skip protect | 改动局部 | 扩大 apply 旁路面；重复 stubs 模式 |
| C. 新 ProtectionMode / 局部 recheck | 双通道 | 隔离 | 维护成本高；闸口分裂 |

**修订理由**：1.7.0 探针已证明缺口仅在步骤 3/7；handler 语义已对齐 Mole；保护层最小形状补洞即可，旁路方案不必要。

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                 # 受保护容器的 Group Containers Logs 叶可入选
vole clean --apply <plan.json>    # 普通删除（默认废纸篓；--permanent 有效）
```

- `rule_id` 仍为 **`group-container-caches`**（无新规则）
- 规则数 **515 不变**；**不 bump** `schema_version`
- handler 对 protected 仍只提 Logs（1.7.0 行为零改）

### 4.2 闸口目标矩阵（实现后）

路径均在 `~/Library/Group Containers/` 下，`ProtectionMode::Cleanup`：

| 形状 | 1.7.0 现状 | 1.9.0 目标 | 备注 |
|---|---|---|---|
| `com.macpaw.CleanMyMac/Logs/x` | 拦（步骤 3） | **放行** | 本刀主收益 |
| `com.macpaw.CleanMyMac/Library/Logs/x` | 拦（步骤 3） | **放行** | 同 |
| `com.macpaw.CleanMyMac/Caches/x` | 拦（步骤 3） | **仍拦** | 纵深：handler 也不提 |
| `com.macpaw.CleanMyMac/Library/Caches/x` | 拦（步骤 3） | **仍拦** | 同 |
| `com.macpaw.CleanMyMac/tmp/x` | 拦（步骤 3） | **仍拦** | 同 |
| `group.com.docker…/Logs/com.docker.helper.log` | 拦（步骤 7） | **放行** | bundle 命名叶 |
| `group.com.docker…/Library/Logs/com.docker.helper.log` | 视现网 | **放行** | explicit 已覆盖 `/Library/Logs/`；步骤 3 对 `group.` 本就不拦 |
| `group.com.docker…/Library/Caches/foo` | 放行 | 放行 | 无变化 |
| `group.com.apple.notes/Logs/x` | 拦（步骤 1） | **仍拦** | 关键字 |
| `HUAQ24HBR6.dev.orbstack/Caches/x` | 拦（OrbStack） | **仍拦** | 顶部闸口 |
| `…/Application Support/…/com.macpaw…` | 拦 | **仍拦** | 非 Group Containers |

## 5. 实现

### 5.1 新 helper

在 `crates/vole-core/src/protection/path.rs`：

```rust
/// Group Containers 下可再生 Logs 路径（1.9.0 Cleanup 形状豁免）。
/// 仅 Logs / Library/Logs；不含 Caches / tmp。
fn is_group_container_logs_path(path: &str) -> bool
```

**判定写死（全部满足）**：

1. `path` 含 `/Library/Group Containers/`
2. 在该前缀之后，路径分量中存在可再生 Logs 段：
   - `…/<container_id>/Logs/…`，或
   - `…/<container_id>/Library/Logs/…`
3. **不得**仅因含字符串 `Logs` 命中（避免 `…/NotLogs/…` 误伤）；按路径分量匹配 `Logs` 为独立分量，且父级为容器 id 或 `…/Library`
4. **明确返回 false**：同容器下的 `Caches` / `Library/Caches` / `tmp` / `Library/tmp`，以及非 Group Containers 路径

可用字符串稳健实现：在 `/Library/Group Containers/` 之后找 `/Library/Logs/` 或 `/Logs/`，并验证该段属于容器相对路径的合法位置（相对容器根 depth 1 的 `Logs`，或 depth 2 的 `Library/Logs`）。禁止在容器根之外任意深度的 `Logs` 泛匹配（例如 `…/<id>/Other/Logs/x` **不**放行——Mole / 现 handler 也不提该树）。

### 5.2 接入 `should_protect_path`

**步骤 3**（沙盒 bundle）：在 `is_container_cache_or_tmp(path)` 为 false 时，若 `is_group_container_logs_path(path)` → 设 `container_cache = true`（与 Data/Caches·tmp 同形），**不要**因 `should_protect_data(bundle_id)` 早退。

**步骤 6/7**：在 `is_explicit_clean_cache_path` 内增加：

```rust
if is_group_container_logs_path(path) {
    return true;
}
```

这样顶层 `/Logs/` 与 bundle 命名叶一并放行；`/Library/Logs/` 亦被覆盖（冗余但无害）。

### 5.3 明确不改

- `groupcaches` handler / select / label / 规模上限 / FDA degrade
- `data/rules/app-caches.toml` / 规则数
- `apply_plan.rs` 任何 `rule_id` 分支 / `skip_protection`
- `protection.toml` / `is_container_cache_or_tmp` 的 Data 沙盒语义
- `ProtectionMode::Uninstall`（本豁免只影响 Cleanup 路径上步骤 3/7；Uninstall 本就不走步骤 3 data_protected 早退语义，无需新行为）

## 6. 覆盖说明

- 全局 `coverage_note`：标明 **Group Containers logs/caches（含受保护容器 Logs / bundle 命名日志）已落地**
- **去掉**「仍未移植：…受保护容器的组容器缓存…」中该项
- 仍未移植保留：真 sudo 删除、其它 sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）
- 单测 `coverage.rs` 同步：断言不再要求 `unported.contains("受保护容器的组容器缓存")`；改为断言该短语 **不在** unported 段
- README：Mole 对比句改为完整覆盖（不再「部分受保护容器已跳过」）；规则数仍 515；版本 **1.9.0**

## 7. 非目标

- 对 protected 容器放行 Caches / tmp
- 整 Group Container orphan / Application Scripts
- 真 sudo / Rosetta `/Library` / claude pending-uploads
- 新规则 / 新 `SkipReason` / schema bump
- stubs 式 protect 豁免 / 非 trash 删除
- 改 handler 的 protected 判定（TeamID 收严等保持 1.7.0）

## 8. 测试与安全

必写单测（`protection/path.rs`）：

1. `…/Group Containers/com.macpaw.CleanMyMac/Logs/x` → Cleanup **不**保护
2. 同容器 `Library/Logs/x` → **不**保护
3. 同容器 `Caches/x`、`Library/Caches/x`、`tmp/x`、`Library/tmp/x` → **仍**保护
4. `…/group.com.docker.docker/Logs/com.docker.helper.log` → **不**保护
5. `…/group.com.apple.notes/Logs/x` → **仍**保护
6. OrbStack runtime 路径 → **仍**保护
7. 非 Group Containers 的 `com.macpaw` Application Support 数据路径 → 行为与改前一致（仍保护）
8. `…/<id>/Other/Logs/x` → **仍**保护（非法 Logs 位置不放行）
9. `safety::property` 与既有保护单测全绿

PR：**security-review 必过**（放行面 + Logs-only 边界 + Notes/OrbStack 不变量）。

集成：扩展或追加 `groupcaches` / plan fixture——`com.macpaw.CleanMyMac` Logs 叶须入选并可 apply（不再仅 assert skip）。

## 9. 验收

1. plan/apply 可清理受保护容器（无 `group.`）的 Logs 叶与 bundle 命名 Logs 叶
2. 同容器 Caches/tmp 仍被保护层拒绝（篡改 plan 也删不掉）
3. Notes / OrbStack / 非 Group Containers 行为零退化
4. coverage / README 反映完整覆盖；去掉「受保护容器的组容器缓存」未移植措辞
5. 规则数 515；发版 **1.9.0**

## 10. 实现后文档

- `docs/releases/v1.9.0.md`、findings（贴 §4.2 矩阵）、Formula
- 真 sudo / Application Scripts / 整容器 orphan 仍另开 design
