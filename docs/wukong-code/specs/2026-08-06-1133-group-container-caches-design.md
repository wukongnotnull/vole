# Group Container Caches 设计（Mole `clean_group_container_caches` 同形）

- 日期：2026-08-06（同日审阅修订：探针推翻「步骤 3 是拦截点」的前提；本期改为**零保护层改动**；TeamID 归一化收严；候选规模设上限；规则改名去 `orphaned-` 前缀）
- 状态：待实现（设计已批准；审阅修订已并入）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/user.sh` → `clean_group_container_caches`；Vole `should_protect_path`（`crates/vole-core/src/protection/path.rs`）；container stubs 的 FDA degrade 模式；coverage「仍未移植：Group Containers 泛清理」
- 包版本意图：能力扩展 → **`1.7.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole **同形**的 Group Containers 可再生清理：

- 只扫 `$HOME/Library/Group Containers`
- **不删整容器**：候选是 Logs /（条件）Caches·tmp 下的**叶节点**
- `data_protected` 容器：仅提 Logs 候选；非 protected：再提 tmp / Caches
- apply 走普通 `mole_delete_verified`（废纸篓 / `--permanent` 有效）
- **本期不改保护层**（审阅修订，见 §6）：被保护层拦下的形状按现状 skip，并在 coverage 注明「部分受保护容器已跳过」

**采纳路径**：custom 规则 + 现有保护层闸口；非整目录 orphan，非硬跳过发现，非 stubs 式 carve-out。

## 2. 问题与风险

1. **Group Containers 含跨应用共享数据**：误删整容器会毁掉沙盒共享状态。必须叶子删除 + Apple / Notes / OrbStack 硬保护。
2. **保护层拦截点与直觉相反**（审阅探针实测，见 §6 矩阵）：绝大多数组容器 id 带 `group.` 前缀，`should_protect_data(raw_id)` 为 **false**，步骤 3 **不**拦；真正拦下的是**步骤 7 叶子文件名 bundle guard**（组容器日志常以 bundle id 命名）与少数不带 `group.` 前缀的 id 在步骤 3 早退。**不要**按「步骤 3 空转」来设计。
3. **TeamID 前缀绕过 data_protected**：`TEAMID.com.tencent.*` 这类 id 只剥 `group.` 无法命中 guard，会被判非 protected 而连 Caches/tmp 一起清（Mole 同此行为）。本期**收严**：见 §5.4。
4. **Safari 扩展副作用**：清扩展组容器缓存可能唤醒 Safari（Mole 已跳过）。
5. **plan 体积与 sizing 成本**：`PlanEntry.size` 由 `path_size` 逐条递归测量，无 partial 概念；组容器 × 缓存条目易上千 → 必须设上限（§5.8）。
6. **FDA**：无法列 `~/Library/Group Containers` → 须响亮降级。
7. **整容器 orphan**：Mole `apps.sh` 写死 NEVER 扫 Group Containers 做 orphan —— 本刀**明确不做**。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A-. custom + 零保护层改动（已选，本期）** | handler 同形扫描；被保护层拦下的形状照常 skip + coverage 注明 | 拿到主要收益（`group.*` 容器今天即可删）；不扩大放行面；security-review 面最小 | 受保护 id 容器与 bundle 命名日志文件清不到 |
| A. custom + 保护层「可再生」扩展 | 另加 helper 使步骤 3/6/7 放行组容器 Logs·Caches·tmp | 完整对齐 Mole | 扩大放行面，需独立安全评审；**改为 1.8.0 另开 design** |
| B. 纯 TOML 通配 | 静态 paths | 简单 | 无法表达 protected 分歧 / Safari 跳过 |
| C. 仅发现硬跳过 | 对齐 system-services | 最安全 | 不兑现清理价值 |
| D. 整容器 orphan | 对齐 stubs | — | Mole NEVER；假阳性高 |

**修订理由**：审阅探针证明「`group.` 前缀容器的 Logs/Caches/tmp 今天就能通过保护层」，主要收益无需改保护层即可拿到；改保护层的部分（受保护 id + bundle 命名文件）收益小、风险高，单独立项更划算。

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                 # 可含 Group Containers logs/caches 叶候选
vole clean --apply <plan.json>    # 普通删除（默认废纸篓；--permanent 有效）
```

- `rule_id`：**`group-container-caches`**（不用 `orphaned-` 前缀：容器所属 app 仍安装，清的是可再生缓存）
- handler：`group_container_caches`
- `category`：`app-caches`
- 规则文件：**`data/rules/app-caches.toml`**（与 `final-cut-pro-generated-cache` 等 custom 规则同处；**不得**放入 `zzz-orphaned.toml`，那里有「`orphaned-system-services` 为最后一条启用规则」的加载顺序断言）
- label：`Group container cache: <container_id>/<relative>`（相对候选目录，含中间层）
- 规则数：514 → **515**
- **不 bump** `schema_version`
- 环境变量：本期不增；禁用走规则 `disabled = true`

### 4.2 扫描根（写死）

| 根 | 内容 |
|---|---|
| `$HOME/Library/Group Containers` | 顶层非 symlink、可读目录 |

**明确不扫**：`Containers` 整目录 orphan、Application Scripts、系统 `/Library`、整容器根删除。

## 5. 判定流水线（plan select）

对每个顶层容器目录，顺序：

1. 根不存在 → `Ok(vec![])`；根存在但 `read_dir` 失败 → degrade（§8）
2. 顶层条目须是目录且**不是** symlink（普通文件直接跳过）；**可读**（`read_dir` 失败 → 跳过该容器，不 degrade，避免重复 TCC 弹窗）
3. `container_id` **不是** `com.apple.*` / `group.com.apple.*` / `systemgroup.com.apple.*`
4. **Safari Web Extension 跳过（fail-closed）**：若 `$HOME/Library/Containers/<container_id>` 存在，则列其顶层条目，任一名字（不分大小写）含 `safari` → 整容器跳过；**该目录存在但不可读 → 同样整容器跳过**（fail-closed，宁可少清）
5. `protected` 判定（比 Mole 收严一档）：对 `container_id` 依次做 `should_protect_data`，命中任一即 protected：
   - 原始 id
   - 去 `group.` 前缀后
   - 去**前导 TeamID**（形如 `^[A-Z0-9]{10}\.`）后
   - 先去 TeamID 再去 `group.` 后（覆盖 `TEAMID.group.com.foo` 变体）
6. 候选子树：
   - 恒有：`<dir>/Logs`、`<dir>/Library/Logs`
   - 仅 `!protected`：`<dir>/tmp`、`<dir>/Library/tmp`、`<dir>/Caches`、`<dir>/Library/Caches`
7. 每个候选子树：是目录且非 symlink；白名单命中**整树**跳过（plan 层白名单只作用于叶子，此处为新增的目录级跳过）；枚举 **mindepth 1 maxdepth 1** 子项（`read_dir` 默认含隐藏项，与 Mole `dotglob` 一致；`.DS_Store` 等会各自成为候选）
8. 每个子项：非 symlink；`symlink_metadata` 成功。是否入 plan 由 plan 层既有闸口决定（`validate_path_for_deletion` + 白名单 + identity），handler **不**自行放行
9. **规模上限**（新增，替代 Mole 的 partial 计数）：单个候选子树最多提 **200** 个叶项，整规则最多 **2000** 个；任一上限触发时，该候选子树**整体不提候选**并记 `PlanNotice::GroupContainersTruncated`（coverage 注明「条目过多已跳过，请用 Mole 或缩小范围」）。**禁止**改为提「目录本身」当候选（会退化成整目录删除）

home 经 `VOLE_TEST_HOME` / 既有测试 home 注入覆盖。

## 6. 闸口矩阵（审阅探针实测，写死为实现基线）

`should_protect_path(..., Cleanup)` 现状（路径均在 `~/Library/Group Containers/` 下）：

| 形状 | 现状保护 | 拦截点 |
|---|---|---|
| `group.com.docker.docker/Library/Caches/foo` | 否 | 无（`is_explicit_clean_cache_path` 放行步骤 6/7；步骤 3 因 `group.` 前缀不命中） |
| `group.com.docker.docker/Caches/foo` | 否 | 无 |
| `group.com.docker.docker/Logs/com.docker.helper.log` | **是** | **步骤 7 叶子文件名 bundle guard** |
| `group.com.docker.docker/Logs/plain.log` | 否 | 无 |
| `TEAMID.com.tencent.xinWeChat/Library/Logs/x` | 否 | 无（TeamID 前缀不命中 guard → 故 §5.5 收严） |
| `com.macpaw.CleanMyMac/Logs/x` | **是** | 步骤 3 `data_protected` |
| `com.macpaw.CleanMyMac/Library/Caches/x` | **是** | 步骤 3（先于步骤 6/7 返回） |
| `group.com.apple.notes/Logs/x` | 是（应保持） | 步骤 1 关键字 |
| `HUAQ24HBR6.dev.orbstack/Caches/x` | 是（应保持） | 顶部 `is_orbstack_runtime_path` |

**本期取舍（零保护层改动）**：

- `group.*` / TeamID 前缀容器的 Logs·Caches·tmp 叶项 → 正常进 plan 并可 apply（主要收益）
- 不带 `group.` 前缀且 `data_protected` 的容器（如 `com.macpaw.*`）→ 其 Logs 会在 plan 层被 `ProtectedPath` 拦下，按现状 emit skip
- 叶子文件名命中 bundle guard（如 `com.docker.helper.log`）→ 同样被拦下并 skip
- 这两类跳过是**已知且可接受**的覆盖缺口，coverage 注明；完整对齐留 1.8.0 另开 design + security review

**禁止**：本期修改 `should_protect_path` / `is_container_cache_or_tmp` / `is_explicit_clean_cache_path` / `protection.toml`；对本规则做 protect 豁免或 carve-out 删除。

## 7. Apply

- **无** `rule_id` 早分支；叶节点走 `mole_delete_verified` 与既有 `verify_plan_entry_for_apply`
- 默认废纸篓；`--permanent` 与其它 clean 规则同形
- 身份 TOCTOU / 白名单 / protect 重验保持现状
- **不需要** stubs 式 `recheck_*`：本规则不豁免任何闸口，篡改/过期 plan 仍由保护层拒绝；未被保护的路径本来就可删，无新增暴露面

## 8. 权限降级

- `~/Library/Group Containers` **不存在** → `Ok(vec![])`，不降级
- 根存在但 `read_dir` 失败（FDA / 权限）→ 整规则降级：`CustomDegrade::GroupContainersInaccessible` → `Skipped(TccDenied)` + `PlanNotice::GroupContainersInaccessible` + 中文 FDA 警告（风格对齐 `ORPHAN_LIBRARY_WARN` / `CONTAINER_STUBS_WARN`）
- 根可读但个别容器不可读 / 无候选 → 正常部分或空结果，**不**降级
- 规模上限触发 → `PlanNotice::GroupContainersTruncated`（非 degrade，不 emit Skipped）

## 9. 覆盖说明

- 全局 coverage：标明 **Group Containers logs/caches（Mole 同形，受保护容器与 bundle 命名文件除外）已落地**；仍未移植改为 **真 sudo 删除**、**受保护容器的组容器缓存**、其它 sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）；**去掉**「Group Containers 泛清理」整体未移植的措辞
- `crates/vole-core/src/ops/coverage.rs` 现有单测断言 `unported.contains("Group Containers 泛清理")`，须随文案同步改写
- README：`data/rules/` 计数 514 → 515；Mole 对比句（README:38）不再把 Group Containers 泛清理当作「仅 Mole」全家桶要点，改为注明部分覆盖

## 10. 非目标

- 整 Group Container 目录 orphan / TeamID 前缀泛删
- **保护层改动**（放行受保护容器 Logs 与 bundle 命名文件）→ 1.8.0 另开 design
- Application Scripts
- 真 sudo
- OrbStack runtime / Notes 容器内容
- `group.com.apple.contentdelivery`：Apple 前缀容器一律跳过，故本刀**不覆盖**它；Mole 在 Application Support 扫描里另有一份 known-list 处理该容器 Logs，属**另一条路径**，本期不移植
- 新 `SkipReason` 变体
- stubs 式 protect 豁免 / 非 trash 删除

## 11. 测试与安全

- 非 Apple、非 protected fixture（`group.com.example.app`）→ Logs + Caches + tmp 叶项入选并可 apply
- `data_protected` 且**不带** `group.` 前缀的容器（`com.macpaw.CleanMyMac`）→ handler 只提 Logs；plan 层因 `ProtectedPath` skip（断言 skip 而非入选）
- TeamID 前缀 + 受保护 vendor（`TEAMID.com.tencent.xinWeChat`）→ §5.5 收严后判 protected → **不**提 Caches/tmp 候选
- `group.com.apple.notes` / OrbStack 路径永不入选；`safety::property` 不变量回归不破（本期不动保护层，应零影响）
- Safari 扩展同 id → 整容器跳过；该 Containers 目录不可读 → 同样跳过（fail-closed）
- symlink 容器 / symlink 叶项 / 普通文件顶层条目 → 不入选
- 隐藏文件（`.DS_Store`）→ 入选（与 Mole 一致）
- 单候选子树 > 200 项 → 整树不提候选 + truncated notice
- 根不存在 → 空结果不降级；根不可读 → degrade + FDA warn
- PR：**security-review** 必过（扫描面 + fail-closed + 上限）

## 12. 验收

1. plan/apply 在 fixture 下可清理非受保护组容器的 Logs·Caches·tmp 叶项
2. 受保护容器（含 TeamID 前缀 vendor）不提 Caches/tmp；受保护 id 的 Logs 由保护层拦下并 skip
3. 保护层行为零变化：Notes / OrbStack / 整容器根仍被拒；property 测试与既有保护单测全绿
4. plan 规模可控：上限触发时给出 notice 而非爆条目
5. coverage / README 反映「部分覆盖」而非「未移植」
6. 规则数 515；发版 **1.7.0**

## 13. 实现后文档

- `docs/releases/v1.7.0.md`、findings（含探针闸口矩阵）、Formula
- 保护层扩展（受保护容器 Logs / bundle 命名文件）另开 design；整容器 / Application Scripts 亦然
