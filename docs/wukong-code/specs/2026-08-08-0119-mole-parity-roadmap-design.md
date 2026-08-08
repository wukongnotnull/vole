# Mole 对齐路线图（近满配 backlog · CLI）

- 日期：2026-08-08（修订：同日 W2c Batch 6+ Chrome DevTools MCP Cache / 1.40.0 合入后更新状态）
- 状态：已批准（盘点文档）；**本文件仍不开实现**，不 bump 包版本
- 依据：Mole `third_party/mole-1.48.1`；[`coverage_note`](../../../crates/vole-core/src/ops/coverage.rs)；[`2026-08-08-0025-mole-system-sh-backlog-design.md`](2026-08-08-0025-mole-system-sh-backlog-design.md)；[`2026-07-30-1900-v2-product-goals-design.md`](2026-07-30-1900-v2-product-goals-design.md)；uninstall / optimize findings
- 范围：相对 Mole 家庭桶的 **近满配** 差距——system 余量、uninstall/optimize 长尾、子命令与桌面边界；**不含**具体实现 plan（开刀时另写 design + plans）

## 1. 结论

产品 v2 CLI 主路径（`status` / `analyze` / `clean` / `history` / `uninstall` / `optimize`）已达；`clean` 的 `system.sh` 深扫主链已对齐。相对 Mole 剩余缺口按 **W0→W3** 收口：**W0（`tm-failed-backups` / 1.28.0）已合入 main（PR #67）**；**W1（本地快照报告 / 1.29.0）已合入 main（PR #70）**；**W2b①（`system_maintenance` + `network_optimization` / 1.31.0）已合入 main（PR #72）**；**W2c 首刀（`filo-production-cache` / 1.32.0）已合入 main（PR #71）**；**W2a①（uninstall brew cask / 1.33.0）已合入 main（PR #69）**；**W2a②（uninstall login items / 1.34.0）已合入 main（PR #77）**；**W2a③（uninstall 系统 LaunchDaemons / `/Library` sudo / 1.35.0）已合入 main（PR #79）**；**W2b②（`memory_pressure_relief` / 1.36.0）已合入 main（PR #81）**；**W2c 续刀（`zed-npm-system-cache` / 1.37.0）已合入 main（PR #82）**；**W2b③（network/disk/periodic / 1.38.0）已合入 main（PR #85）**；**W2c Batch 6+（`antigravity-browser-cache` / 1.39.0）已合入 main（PR #87）**；**W2c Batch 6+（`chrome-devtools-mcp-cache` / 1.40.0）已合入 main（PR #89）**。路径级主空洞（新家族 `Default/Cache`）已收口；**暂停 Batch6 必做**；**optimize 后置长尾（spotlight* 等）保持 coverage**；**W3 不开发**（仅记档）。兄弟路径 / QQ Music `iRRCache` 等若日后要补齐，属可选非必做续刀，须另开 design。

本文件本身不触发实现 PR。

## 2. 波次顺序与并行矩阵

```mermaid
flowchart LR
  W0[W0_TM_done]
  W1[W1_snapshots_done]
  W2a[W2a_uninstall_tail]
  W2b[W2b_optimize_tail]
  W2c[W2c_Batch6_plus]
  W3[W3_never_or_deferred]
  W0 --> W1
  W0 --> W2a
  W0 --> W2b
  W0 --> W2c
  W1 -.-> W3
  W2a -.-> W3
  W2b -.-> W3
  W2c -.-> W3
```

| 波次 | 关系 | 形态标签 |
|---|---|---|
| **W0** | **已完成**（1.28.0 / PR #67） | 可删（TM 失败备份） |
| **W1** | **已完成**（1.29.0 / PR #70） | 仅报告 |
| **W2a/b/c** | W0/W1 已解除阻塞；三轨彼此可并行；**轨内串行发版**；**W2a①②③ / W2b①②③ / W2c 首刀+续刀+Batch6 Antigravity+Chrome DevTools MCP 已完成**；**Batch6 必做已暂停**；optimize 后置长尾 / W3 仅 coverage 记档 | 可删 / 需特权 action |
| **W3** | 不开发 | 永不做 / 延后 |

并行波次现状（与 main 对齐）：

| 项 | 版本 / PR | 状态 |
|---|---|---|
| W0 TM | 1.28.0 | ✅ |
| W1 本地快照报告 | 1.29.0 / #70 | ✅ |
| W2b① `system_maintenance` / `network_optimization` | 1.31.0 / #72 | ✅ |
| W2c 首刀 `filo-production-cache` | 1.32.0 / #71 | ✅ |
| W2a① uninstall brew cask | 1.33.0 / #69 | ✅ |
| W2a② login items | 1.34.0 / #77 | ✅ |
| W2a③ LaunchDaemons / `/Library` sudo | 1.35.0 / #79 | ✅ |
| W2b② `memory_pressure_relief` | 1.36.0 / #81 | ✅ |
| W2c 续刀 `zed-npm-system-cache` | 1.37.0 / #82 | ✅ |
| W2b③ network/disk/periodic | 1.38.0 / #85 | ✅ |
| W2c Batch 6+ `antigravity-browser-cache` | 1.39.0 / #87 | ✅ |
| W2c Batch 6+ `chrome-devtools-mcp-cache` | 1.40.0 / #89 | ✅ |
| W2c Batch 6+ 再续挑 | — | **暂停必做**（余量仅为兄弟路径 / QQ Music `iRRCache` 等可选长尾，不默认开刀） |
| W2b spotlight* 等后置 | — | **保持 coverage**（不默认开刀） |
| W3 | — | **不开发** / 延后记档 |

### 2.1 W0 — Time Machine 失败备份（已完成）

- **项**：`tm-failed-backups`（**1.28.0**）：`tmbackup` / `plan` / `apply` / coverage / 发版文档；security-review；已合并。
- **计划**：[`../plans/2026-08-08-0057-tm-failed-backups.md`](../plans/2026-08-08-0057-tm-failed-backups.md)
- **形态**：可删（仅 `tmutil delete`；fail-closed）
- **现状**：coverage「已落地」含 Time Machine 失败中备份。

### 2.2 W1 — 本地快照报告（已完成）

- **项**：对齐 Mole `clean_local_snapshots` 的 **报告面**：`tmutil listlocalsnapshots /` → 数量 + review 提示（**1.29.0** / PR #70）。
- **挂载**：`vole status` 与 `analyze` 提示行；proto 可选 `local_snapshots`。
- **形态**：仅报告；**禁止** `clean --apply` 删除本地快照（删快照属 W3 永不做，除非未来另开产品决策）。
- **现状**：coverage「仍未移植」已去掉本地快照报告；余量为桌面特权助手等。

### 2.3 W2 — 三条并行轨（当前推荐）

#### W2a — uninstall 长尾

依据：[`../../findings/2026-07-v2-m1-uninstall.md`](../../findings/2026-07-v2-m1-uninstall.md)

| 顺序 | 项 | 形态 |
|---|---|---|
| **① 已完成** | brew cask 卸载联动（**1.33.0** / PR #69） | 可删 / 编排 |
| **② 已完成** | login items（**1.34.0** / PR #77） | 可删 / 审计 |
| **③ 已完成** | 系统 LaunchDaemons / `/Library` sudo 残留（**1.35.0** / PR #79；复用 PrivilegeBackend / `sudo -n`） | 可删 + 需特权 |

- **触点**：uninstall plan/apply、protection
- **并行**：可与 W2b / W2c 并行；**不得**两 agent 同改 uninstall 核心路径；轨内按 ①→②→③ 发版；③ 已合入后轨内无必做项（广谱边缘 / SMAppService 归 W3 或 coverage）
- **设计 / 计划**：[`2026-08-08-1031-uninstall-system-launchdaemons-design.md`](2026-08-08-1031-uninstall-system-launchdaemons-design.md)；[`../plans/2026-08-08-1032-uninstall-system-launchdaemons.md`](../plans/2026-08-08-1032-uninstall-system-launchdaemons.md)

#### W2b — optimize 长尾择优

依据：[`../../findings/2026-07-v2-m2-optimize-spike.md`](../../findings/2026-07-v2-m2-optimize-spike.md)；`optimize/catalog.rs`；设计 [`2026-08-08-0156-optimize-system-network-design.md`](2026-08-08-0156-optimize-system-network-design.md)、[`2026-08-08-1121-optimize-memory-pressure-design.md`](2026-08-08-1121-optimize-memory-pressure-design.md)、[`2026-08-08-1200-optimize-w2b3-trio-design.md`](2026-08-08-1200-optimize-w2b3-trio-design.md)

| 顺序 | task_id | 说明 | 形态 |
|---|---|---|---|
| **① 已完成** | `system_maintenance` / `network_optimization`（**1.31.0** / PR #72） | fail-closed + `sudo -n` DNS flush | 需特权 action |
| **② 已完成** | `memory_pressure_relief`（**1.36.0** / PR #81） | 高压时 `sudo -n purge`；低压 noop | 需特权 action |
| **③ 已完成** | `network_stack_optimize` / `disk_permissions_repair` / `periodic_maintenance`（**1.38.0** / PR #85） | 网络栈 / 权限 / periodic | 需特权 action |
| 后置或保持长尾 | `spotlight_index_optimize` / `spotlight_orphan_rules_cleanup` / `disk_verify` / `login_items_audit` / `shared_file_list_repair` | 易误伤或高复杂 | 可 indefinitely 留 coverage（**不默认开刀**） |

- **触点**：`optimize/catalog.rs`、tasks、optimize_plan/apply
- **并行**：与 W2a / W2c 并行；**轨内串行**；catalog `in_m3` 翻转单刀单 PR。
- **现状**：①②③ 已合入；spotlight* / disk_verify / login_items / shared_file_list **保持长尾 coverage**。
- **设计 / 计划（③）**：[`2026-08-08-1200-optimize-w2b3-trio-design.md`](2026-08-08-1200-optimize-w2b3-trio-design.md)；[`../plans/2026-08-08-1201-optimize-w2b3-trio.md`](../plans/2026-08-08-1201-optimize-w2b3-trio.md)

#### W2c — clean 规则 Batch 6+

| 顺序 | 项 | 形态 |
|---|---|---|
| **首刀已完成** | `filo-production-cache`（规则 534；**1.32.0** / PR #71） | 可删 |
| **续刀已完成** | `zed-npm-system-cache`（规则 535；**1.37.0** / PR #82） | 可删 |
| **Batch 6+ 已完成** | `antigravity-browser-cache`（规则 536；**1.39.0** / PR #87） | 可删 |
| **Batch 6+ 已完成** | `chrome-devtools-mcp-cache`（规则 537；**1.40.0** / PR #89） | 可删 |
| **暂停必做** | 余下兄弟路径 / QQ Music `iRRCache` 等 | 可选长尾；不默认开刀 |
| **不进本波必做** | `user.sh` 广域、盲扩 `custom` | 保持「继续用 Mole」 |

- **触点**：`data/rules/`、handlers、coverage 规则计数
- **并行**：与 W2a / W2b 并行；**Batch 6 必做已暂停**；若日后补齐可选窄规则，单刀单 PR + 另开 design。
- **设计 / 计划**：[`2026-08-08-1310-chrome-devtools-mcp-cache-design.md`](2026-08-08-1310-chrome-devtools-mcp-cache-design.md)；[`../plans/2026-08-08-1311-chrome-devtools-mcp-cache.md`](../plans/2026-08-08-1311-chrome-devtools-mcp-cache.md)；前序 [`2026-08-08-1230-antigravity-browser-cache-design.md`](2026-08-08-1230-antigravity-browser-cache-design.md)

### 2.4 W3 — 永不做 / 明确延后（只记档）

| 类别 | 项 | 标签 |
|---|---|---|
| 与 Mole keep / 安全一致 | 删除 `/Library/Updates`、`/macOS Install Data` | **永不做** |
| 产品决策 | `clean` 删除本地快照 | **永不做**（本代际） |
| 产品 v2 §4.2 代际外 | `purge` / `installer` / `touchid` / `hints` / Mole 式 `update` | **永不做（本代际）**；另开代际再议 |
| 桌面 | SMAppService / PrivilegedHelper（vole-macos） | **延后** |

引用：产品目标设计 §4.2；system backlog §「永不做」与桌面排除。

## 3. 选刀优先级（默认下一刀）

落地实现时按此取项（**不在本文件实现**）：

1. ~~**W0** TM 收口合入~~ **已完成**（1.28.0 / PR #67）
2. ~~**W1** 本地快照报告~~ **已完成**（1.29.0 / PR #70）
3. ~~**并行池 · W2b①**~~ **已完成**：`system_maintenance` / `network_optimization`（1.31.0 / PR #72）
4. ~~**并行池 · W2c 首刀**~~ **已完成**：`filo-production-cache`（1.32.0 / PR #71）
5. ~~**并行池 · W2a①**~~ **已完成**：uninstall brew cask（1.33.0 / PR #69）
6. ~~**并行池 · W2a②**~~ **已完成**：uninstall login items（1.34.0 / PR #77）
7. ~~**并行池 · W2a③**~~ **已完成**：系统 LaunchDaemons / `/Library` sudo（1.35.0 / PR #79）
8. ~~**并行池 · W2b②**~~ **已完成**：`memory_pressure_relief`（1.36.0 / PR #81）
9. ~~**并行池 · W2c 续刀**~~ **已完成**：`zed-npm-system-cache`（1.37.0 / PR #82）
10. ~~**并行池 · W2b③**~~ **已完成**：`network_stack_optimize` / `disk_permissions_repair` / `periodic_maintenance`（1.38.0 / PR #85）
11. ~~**并行池 · W2c Batch 6+**~~ **已完成**：`antigravity-browser-cache`（1.39.0 / PR #87）
12. ~~**并行池 · W2c Batch 6+**~~ **已完成**：`chrome-devtools-mcp-cache`（1.40.0 / PR #89）
13. **默认下一刀写死**：**暂停 Batch6 必做**；**optimize 后置长尾保持 coverage**；**W3 不开发**。若刻意补齐可选窄规则（Antigravity/MCP 兄弟路径、QQ Music `iRRCache` 等），须另开 design，**不**视为本路线图必做。

## 4. 与既有文档关系

| 文档 | 关系 |
|---|---|
| [`2026-08-08-0025-mole-system-sh-backlog-design.md`](2026-08-08-0025-mole-system-sh-backlog-design.md) | **system 对照表仍权威**；本文件为其 **超集**；W0/W1/W2a①②③/W2b①②③/W2c 首刀+续刀+Batch6 Antigravity+Chrome DevTools MCP 状态以本文件为准 |
| [`../plans/2026-08-08-0057-tm-failed-backups.md`](../plans/2026-08-08-0057-tm-failed-backups.md) | W0 实施计划（已落地） |
| [`2026-08-08-0156-optimize-system-network-design.md`](2026-08-08-0156-optimize-system-network-design.md) | W2b① 设计（已落地，1.31.0） |
| [`2026-08-08-1121-optimize-memory-pressure-design.md`](2026-08-08-1121-optimize-memory-pressure-design.md) | W2b② 设计（已落地，1.36.0） |
| [`2026-08-08-1200-optimize-w2b3-trio-design.md`](2026-08-08-1200-optimize-w2b3-trio-design.md) | W2b③ 设计（已落地，1.38.0） |
| [`2026-08-08-0953-uninstall-login-items-design.md`](2026-08-08-0953-uninstall-login-items-design.md) | W2a② 设计（已落地，1.34.0） |
| [`2026-08-08-1031-uninstall-system-launchdaemons-design.md`](2026-08-08-1031-uninstall-system-launchdaemons-design.md) | W2a③ 设计（已落地，1.35.0） |
| [`2026-08-08-1121-zed-npm-system-cache-design.md`](2026-08-08-1121-zed-npm-system-cache-design.md) | W2c 续刀设计（已落地，1.37.0） |
| [`2026-08-08-1230-antigravity-browser-cache-design.md`](2026-08-08-1230-antigravity-browser-cache-design.md) | W2c Batch 6+ 设计（已落地，1.39.0） |
| [`2026-08-08-1310-chrome-devtools-mcp-cache-design.md`](2026-08-08-1310-chrome-devtools-mcp-cache-design.md) | W2c Batch 6+ 设计（已落地，1.40.0） |
| [`../../findings/2026-07-v2-m1-uninstall.md`](../../findings/2026-07-v2-m1-uninstall.md) | W2a 长尾清单 |
| [`../../findings/2026-07-v2-m2-optimize-spike.md`](../../findings/2026-07-v2-m2-optimize-spike.md) | W2b 长尾清单 |
| [`2026-07-30-1900-v2-product-goals-design.md`](2026-07-30-1900-v2-product-goals-design.md) | W3「代际外」边界权威 |

## 5. 验收（本文档）

- [x] 波次 W0–W3 无 TBD；并行 / 串行写死
- [x] 每项标明：可删 / 仅报告 / 永不做 / 延后
- [x] 指向 system backlog、TM plan、M1/M2 findings、product goals
- [x] 声明本文件不触发实现、不 bump 版本
- [x] W0/W1/W2a①②③/W2b①②③/W2c 首刀+续刀+Batch6 Antigravity+Chrome DevTools MCP 状态与 main（含 1.40.0 / PR #89）一致；**暂停 Batch6 必做** 写死

下一步：**无默认实现刀**。暂停 Batch6 必做；optimize 后置长尾保持 coverage；W3 不开发。
