# Vole 产品 v2 · CLI 做全（续篇）

- 日期：2026-08-08 20:30
- 状态：已批准（本会话锁定；本文件为产品 v2 续篇权威）
- 快照：`main` @ **1.46.0**（近满配 + optimize 闸控 G1–G5）；Mole 钉版 `third_party/mole-1.48.1`
- 依据：本会话 brainstorming；[`2026-07-30-1900-v2-product-goals-design.md`](2026-07-30-1900-v2-product-goals-design.md)（v2 前半：uninstall / optimize，**已完成**）；[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) §1.2 B / §4.2（禁令由本文件推翻）；[`2026-07-30-semver-policy-design.md`](2026-07-30-semver-policy-design.md)
- 范围：产品 v2 后半——把 CLI 能力做全；**不含**具体实现 plan（各命令另开 design → plan → 单 PR）

## 1. 结论

**产品 v2 北极星（续）：CLI 全家桶做全。**  
前半（uninstall / optimize 及近满配 clean）已在 **`1.x`** 完成。后半按顺序高对齐交付：

`purge` → `hints` → `installer` → `touchid` → 自更新 `update` → 自卸载 `remove`

> `hints` **不是子命令**（Mole 亦无 `mo hints`）：它是 `clean` 内的只读提示模块，按里程碑排序仅表示交付次序。**禁止**实现 `vole hints` 顶层子命令。

**判据是命令面而非里程碑计数：** 本续篇「做全」= **Vole 顶层命令面 ⊇ Mole 1.48.1 路由表**（§3.1 豁免项除外），可机械核对（§3.2）。

**版本统一（本会话锁定）：**

| 话术 | 值 |
|---|---|
| 产品代际 | **v2**（不另起 v3） |
| 包版本线 | **`2.x`**；本续篇首个功能发版升 **`2.0.0`** |

这**显式推翻** [`1900`](2026-07-30-1900-v2-product-goals-design.md) §1 / §7「产品 v2 ≠ 强制 `2.0.0`」与 [`1727`](2026-08-08-1727-mole-parity-roadmap-design.md) §1.2 B 对 `purge` / `installer` / `touchid` / `hints` / Mole 式 `update` 的「本代际永不做」。  
SemVer 政策其余条款仍有效：破坏性 CLI/协议变更仍可再升 MAJOR；本续篇内后续兼容能力走 **`2.y.0` MINOR**。

桌面 / SMAppService（D1）**不在本续篇主路径**；不阻塞「产品 v2 CLI 全家桶」宣告。

本文件授权开启续篇里程碑；**不**直接改代码、不在本文 bump 工作区版本。

## 2. 已锁定决策

| 项 | 结论 |
|---|---|
| 北极星 | CLI 做全（Vole 命令面 ⊇ Mole 路由表） |
| 推进方式 | 顺序里程碑；每命令专用 design → plan → 单 PR |
| 命令顺序 | `purge` → `hints` → `installer` → `touchid` → `update` → `remove` |
| 深度 | **高对齐主路径**：`--plan` / `--apply`（或等价两阶段）、JSON、保护层、菜单/补全；长尾 skip + coverage |
| `update` | **自更新通道**（对照 Mole `lib/manage/update.sh` + 安装器）；可与 Homebrew 并存；**非**仅 `brew upgrade` 包装 |
| `remove` | **自卸载**（对照 Mole `lib/manage/remove.sh`）；支持 `--dry-run`；与 `update` 共享安装路径判定 |
| 命令名兼容 | `completions` 增补 `completion` 别名；`optimize` 增补 `optimise`；`analyze` 增补 `analyse`（见 §3.3） |
| 裸调用 | **默认不联网**：`vole` 无子命令时直接进交互菜单，不做 Mole 式 `check_for_updates`（见 §6.5） |
| 产品 / 包 | 产品 **v2**；包线 **`2.x`**；首发 **`2.0.0`** |
| 桌面 | 非主路径；D1 真机验收与 coverage 改写不阻塞本续篇 |
| 禁区保留 | 不删本地快照（apply）；不删 `/Library/Updates`、`/macOS Install Data` |

## 3. 成功标准（产品 v2 CLI 全家桶）

### 3.1 命令面对照（Mole 1.48.1 路由表 → Vole）

| Mole 命令 | Vole 现状（1.46.0） | 本续篇处置 |
|---|---|---|
| `clean` | ✅ | 已达 |
| `uninstall` | ✅ | 已达 |
| `optimize` / `optimise` | ✅（无 `optimise` 别名） | **补别名**（§3.3） |
| `analyze` / `analyse` | ✅（无 `analyse` 别名） | **补别名**（§3.3） |
| `status` | ✅ | 已达 |
| `history` | ✅ | 已达 |
| `completion` | ⚠️ 仅 `completions` | **补别名**（§3.3） |
| `help` / `--help` / `-h` | ✅（clap） | 已达 |
| `version` / `--version` / `-V` | ✅（clap） | 已达 |
| 裸调用 → 交互菜单 | ✅ | 已达；**不**跟进 Mole 的 `check_for_updates`（§6.5） |
| `purge` | ❌ | **M5** |
| `installer` | ❌ | **M7** |
| `touchid` | ❌ | **M8** |
| `update` | ❌ | **M9**（自更新通道） |
| `remove` | ❌ | **M10**（自卸载） |

**豁免（非顶层命令，不计入「⊇」判据）：**

| 项 | 说明 |
|---|---|
| `hints` | Mole 内是 `lib/clean/hints.sh`，非子命令；本续篇按 **M6** 交付为 `clean` 内只读提示 |
| `whitelist` | Mole 是 `optimize`/`clean` 内的交互式 `manage_whitelist`；Vole 已有 `clean --whitelist` 系列 flag。**能力已有、形态不同，属可接受差异**，不作为缺口重开 |

### 3.2 命令面核对闸门（收口必过）

收口时须有一处**可机械执行**的核对，断言 Vole 顶层命令面（含别名）覆盖 §3.1 的 Mole 路由表减豁免项——形态可为脚本或测试（例如比对 `vole --help` 命令列表与钉版 `mole` 路由表清单），与 `scripts/inventory-mole-rules.py` 在规则级的作用等价。具体实现形态由收口里程碑的 plan 决定。

### 3.3 命令名与别名兼容

Mole 用户的既有脚本必须能直接改名调用：

- `vole completion` 作为 `completions` 的别名（保留 `completions` 为主名，不破坏现有用户）
- `vole optimise` / `vole analyse` 作为英式别名
- 别名走 clap `visible_alias` 或等价，纳入 shell 补全与 `--help`

此项为**兼容性追加**，不破坏既有命令，按 MINOR 处理；可并入任一里程碑落地（建议随 M5 一并交付，使 `2.0.0` 即具备完整命令名兼容）。

### 3.4 逐命令成功标准

1. `vole purge` 高对齐：项目构建物发现 → plan → apply；`purge_paths`（或等价）配置；JSON；保护/废纸篓/oplog
2. `hints`：挂在 `clean` 的非破坏提示主路径（含 purge 快捷探针等 Mole 主路径子集）；有超时/浅扫预算
3. `vole installer` 高对齐：扫描安装包 → plan → apply
4. `vole touchid`：PAM Touch ID 引导开关；`VOLE_TEST_NO_AUTH` 下可测；验证不挂起真授权
5. `vole update`：自更新检测 → 下载 → 校验 → 安装；支持 `--force` / `--nightly`（名称以 M9 design 为准）；校验失败 fail-closed
6. `vole remove`：自卸载；`--dry-run` 预览；仅删本工具自身安装产物与配置（范围在 M10 design 写死）；brew 管理安装时提示走 `brew uninstall`
7. 交互菜单与 shell 补全覆盖上述命令与别名
8. §3.2 命令面核对闸门通过
9. README 明确「产品 v2 CLI 全家桶」；包版本线在 **`2.x`**，且至少完成首发 **`2.0.0`** 与收口时的最新 MINOR
10. coverage / findings 诚实记录长尾与桌面非目标

## 4. 范围

### 4.1 必做

- **`purge`**：重型项目构建物清理（对照 Mole `bin/purge.sh` + `lib/clean/project.sh`）；复用删除漏斗与保护层；默认废纸篓（可 `--permanent`）
- **`hints`**：只读；对照 `lib/clean/hints.sh` 主路径子集；不引入第二套删除路径
- **`installer`**：对照 `bin/installer.sh`；Downloads/Desktop 等扫描根；immutable delete-plan 校验精神对齐
- **`touchid`**：对照 `bin/touchid.sh`；优先 `sudo_local` + `pam_tid.so`；安全回滚路径在专用 design 写死
- **`update`（自更新）**：见 §6.5；发版流水线（签名/公证/Release资产）须能支撑该通道
- **`remove`（自卸载）**：对照 `lib/manage/remove.sh`；`--dry-run` 预览；与 `update` 复用安装来源判定
- **命令名/别名兼容**：`completion` / `optimise` / `analyse`（§3.3）
- 编排住在 `vole-core::ops`；`vole-cli` 薄前端
- 协议优先追加字段；破坏性协议变更 bump `schema_version` 并按 SemVer 评估

### 4.2 明确不做（本续篇）

- 桌面主路径 / SMAppService 深度联动 / Uninstall UI 全量
- `clean --apply` 删除本地快照
- 删除 `/Library/Updates`、`/macOS Install Data`
- Linux；MAS 分发
- 把 clean/purge 规则引擎搬进特权 Helper
- 盲扩 Mole `user.sh` 广域 custom 循环（继续用 Mole；窄规则另开 design）

### 4.3 可穿插、不阻塞宣告

- clean / uninstall 广谱边缘
- fixture / conformance / 文档 / 缺陷修复
- D1 真机 `uid==0` 验收与 coverage「仍未移植」改写（另仓 / 另 PR）

## 5. 里程碑与版本

| 里程碑 | 内容 | 包版本 |
|---|---|---|
| **M4** | 六命令（`purge` / `installer` / `touchid` / `update` / `remove` + `hints` 模块）Mole 库存与安全面 spike；划主路径 vs 长尾；核定 §3.1 命令面对照 | docs-only（可仍停在 1.46.x） |
| **M5** | `purge` + 测试 / 菜单 / 补全 | **`2.0.0`**（升 MAJOR，产品/包对齐） |
| **M6** | `hints` | **`2.1.0`** |
| **M7** | `installer` | **`2.2.0`** |
| **M8** | `touchid` | **`2.3.0`** |
| **M9** | 自更新 `update` | **`2.4.0`** |
| **M10** | 自卸载 `remove` | **`2.5.0`** |
| **收口** | §3.2 命令面核对闸门 + findings + README「产品 v2 CLI 全家桶」 | 当时最新 `2.x` |

顺序：M4 → M5 / **2.0.0** → M6 → M7 → M8 → M9 → M10 → 收口。  

PATCH 用于缺陷修复，不占用上表里程碑号。

> M9 / M10 共享安装来源判定与安装产物清单；若 M9 design 显示两者耦合度高，允许合并为单里程碑单 PR（版本相应合并为一个 MINOR）。

> 示意 MINOR 号可按实际穿插调整（例如中间穿插无关 MINOR），但 **`2.0.0` 必须落在 M5（`purge`）首发**，不得在无新 CLI 能力时空 bump MAJOR。

## 6. 架构约束（实施时遵守）

1. **复用** clean：路径校验、保护层、废纸篓、oplog、不可信 plan 闸口。
2. **`purge` / `installer`** 删除必须走既有安全漏斗；禁止平行 `rm -rf` 捷径。
3. **`hints`** 只读、超时与浅扫预算；慢路径降级为跳过提示，不阻塞 `clean`。
4. **`touchid`** 测试与 CI：`VOLE_TEST_NO_AUTH` / mock；禁止验证路径触发真 Touch ID / 交互 sudo 挂起。
5. **`update`（自更新）**
   - 对照 Mole：检测 → 下载 → 校验 → 安装；`--force` / `--nightly`
   - **校验失败 fail-closed**（checksum / 签名或 attestation）；禁止静默降级到未校验源码安装
   - 成功以**安装后** `vole --version` 为准，不以安装器 stdout
   - 与 Homebrew 并存：检测安装前缀/来源；若判定为 brew 管理安装，默认提示优先 `brew upgrade`，自更新须显式确认或 `--force`（细节 M9 design 写死）
   - 自愈/重装路径若存在：不得引入「校验失败仍继续」的后门
   - **裸调用不联网**：`vole` 无子命令时进交互菜单即可，**不**跟进 Mole 的 `check_for_updates`；版本检查只在用户显式 `vole update`（或其只读检查形态）时发生
6. **`remove`（自卸载）**：删除范围限 Vole 自身安装产物与自身配置，逐项可预览（`--dry-run`）；**禁止**波及用户数据、oplog/审计历史（除非用户显式要求）、或 brew 管理的其它包；判定为 brew 安装时提示 `brew uninstall` 而非自行删除其产物
7. **许可证**：GPL-3.0-only
8. **Mole 钉版**：`third_party/mole-1.48.1`；升级钉版另议，不盲跟上游

## 7. 与既有文档关系

| 文档 | 关系 |
|---|---|
| **本文件** | **产品 v2 续篇（CLI 做全）权威**；授权 `2.x` / `2.0.0` |
| [`2026-07-30-1900-v2-product-goals-design.md`](2026-07-30-1900-v2-product-goals-design.md) | v2 **前半**（uninstall / optimize）仍有效；§4.2 五命令禁令与「≠2.0.0」由本文件覆盖 |
| [`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) | 近满配盘点仍权威；§1.2 B 五命令「永不做」对**本续篇**失效；快照/Updates 禁区仍有效 |
| [`2026-07-30-semver-policy-design.md`](2026-07-30-semver-policy-design.md) | 三位含义仍有效；**本续篇**以产品决策将首发定为 MAJOR `2.0.0`（对齐产品 v2） |
| [`../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md) | 收口/闸控轨历史；D1 不并入本续篇主路径 |

## 8. 文档阶段验收

- [x] 产品 v2 + 包 `2.x` / 首发 `2.0.0` 写死
- [x] 命令顺序与高对齐深度写死
- [x] §3.1 覆盖 Mole 1.48.1 **全部**路由项（含 `remove`、别名、裸调用），豁免项写明理由
- [x] §3.2 命令面核对闸门定为收口必过
- [x] `update` = 自更新通道（非 brew 包装）、`remove` = 自卸载写死
- [x] `hints` 明确不是子命令
- [x] 裸调用默认不联网写死
- [x] 明确不做与禁区保留写死
- [x] 与 1900 / 1727 / SemVer 关系无自相矛盾
- [x] 声明本文件不直接实现、不空 bump 版本

## 9. 下一步

1. 人类审阅本规格  
2. 通过后：`writing-plans` 写 **M4 spike** 实施计划（或合并 M4+M5 若 spike 极短）  
3. 各命令落地前仍须**专用 design**（尤其 `update` 校验与 brew 共存、`remove` 删除范围）
