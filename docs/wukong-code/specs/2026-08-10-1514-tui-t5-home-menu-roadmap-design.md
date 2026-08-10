# T5：裸 `vole` 首页 mole 同构 + T6–T8 路线图

- 日期：2026-08-10 15:14
- 状态：已批准（会话 brainstorming：目标 C · 分波路线 1 · Clean 确认选 2 · §1–§3 ok）
- Mole 钉版：`third_party/mole-1.48.1`
- 对照：`mole` 入口 `interactive_main_menu` / `show_main_menu` / `_main_menu_controls_line`
- 父权威：[`2026-08-09-2136-tui-interactive-mole-parity-design.md`](2026-08-09-2136-tui-interactive-mole-parity-design.md)（目标 D · T0–T4）+ [`2026-08-09-2303-tui-t4-interactive-closeout-design.md`](2026-08-09-2303-tui-t4-interactive-closeout-design.md)（**2.8.0** 收口）
- 发版承接：[`docs/releases/v2.8.0.md`](../../releases/v2.8.0.md)
- 包版本意图：每波合入时按仓惯例 **MINOR**（相对当时 `Cargo.toml`）；本文件不预写具体版本号
- **不 bump** `schema_version`

## 1. 结论

**目标 D（T0–T4）已在 2.8.0 收口**：共享分页多选、uninstall/purge/installer/whitelist 双轨、status/analyze 视觉同构均已交付。缺口在**裸入口首页**：今日 `crates/vole-cli/src/interactive.rs` 仍是 Phase 5 数字 `Select [1-11]`（多数写死 `--plan`），与 `mo` 的 `interactive_main_menu` 心智不一致。

本 design 批准：

1. **T5（本波详设）**：裸 `vole` ratatui 首页与 `mo` 同构（品牌 / 五项 / footer / exec 式启动）
2. **T6–T8（同文档路标）**：Clean/Optimize 确认双轨 → analyze 进阶键 → status 抛光与文档收口

编排 / 删除 / 保护仍在 `vole-core`；首页只做导航壳，UI 留在 `vole-cli`。禁止第二条删除路径。不 bump `schema_version`。

## 2. 目标与波次（§1）

**产品目标：** 裸 `vole` 首页与 `mo` 同构（品牌条 / 五项主菜单 / footer）；再按波次补齐「进真实交互」与 analyze/status 长尾。

| 波次 | 内容 | 发版意图 |
|---|---|---|
| **T5** | 主入口 ratatui 首页：ASCII「Vole」+ tagline + URL；五项（Clean / Uninstall / Optimize / Analyze / Status）+ 描述列；↑↓ / 数字 / Enter / Q；footer：`M` help · `V` version · `T` TouchID（未配置时）· 可选 `U` Update；Enter 后 **exec 式**拉起对应子命令（不回菜单，对齐 mole） | MINOR |
| **T6** | `clean`（及 `optimize` 若缺）TTY 双轨：裸 TTY → 扫 plan → 摘要 → 确认 → `apply_*`；`--plan` / `--json*` / 非 TTY 不变 | MINOR |
| **T7** | analyze：Space 多选、⌫ 删除（走保护/废纸篓漏斗）、O/P、`/` Filter、T Top；footer 只标已接线键 | MINOR |
| **T8** | status 动画 cat + `k`/`c` prefs；`optimize --whitelist`；README/对照表收口 | MINOR 或与 T7 合并 |

**T5 诚实边界：** Clean / Optimize Enter 后仍走现有 plan-only 路径，直到 T6；README / help 必须写明。

**全程不做：**

| 项 | 说明 |
|---|---|
| bash 像素级光标重绘 | ratatui 立即模式替代 `menu_paginated` / 主菜单增量重绘 |
| 选择会话进 `vole-core` / 桌面 | UI 留在 `vole-cli` |
| bump `schema_version` | 无协议字段变化 |

## 3. T5 信息架构与键位契约（§2）

对照 mole `interactive_main_menu` / `show_main_menu`：vole 首页只做**导航壳**，业务仍在各子命令。

### 3.1 布局（上→下）

1. **品牌区**：ASCII `Vole`（ok/绿色）+ 右侧链接 `https://github.com/wukongnotnull/vole` + 下一行 tagline：`Deep clean and optimize your Mac.`
2. **可选更新条**：复用现有 update 缓存/检查结果（有则显示，无则省略）
3. **主菜单 5 项**（文案对齐 mole）：

| # | 标题 | 描述 |
|---|---|---|
| 1 | Clean | Free up disk space |
| 2 | Uninstall | Remove apps completely |
| 3 | Optimize | Refresh caches and services |
| 4 | Analyze | Explore disk usage |
| 5 | Status | Monitor system health |

选中行：`>` + cyan；未选中：缩进空格 + 白字。标题/描述两列对齐。

4. **footer**（subtle）：`↑↓  |  Enter  |  M More  |  V Version  |  [T TouchID]  |  [U Update]  |  Q Quit`

| 条件键 | 显示规则 |
|---|---|
| `T TouchID` | 仅 TouchID **未**配置时显示（对齐 mole） |
| `U Update` | 仅有更新提示时显示 |
| `M More` | 清屏后打印 help（等价 `--help` 精简面），然后 **exit 0**（不回菜单） |

### 3.2 键位

| 键 | 行为 |
|---|---|
| ↑ / ↓ | 在 1–5 间移动 |
| `1`–`5` | 直接启动对应项 |
| Enter | 启动当前项 |
| M | help 后 exit 0 |
| V | version 后 exit 0 |
| T / U | 条件显示；分别进 `touchid` / `update` |
| Q / Esc / Ctrl+C | 恢复光标后 exit 0 |

### 3.3 启动语义（相对今日 `interactive.rs`）

| 项 | 今日 | T5 |
|---|---|---|
| 菜单形态 | 数字 `Select [1-11]` | ratatui 首页（§3.1） |
| 子命令 | `spawn` 后**回到**菜单 | **exec 式**（进程替换或 spawn 后同码退出），不回菜单 |
| Clean / Optimize | 菜单写死 `--plan` | 裸子命令（TTY 仍 plan-only，直到 T6） |
| Uninstall / Analyze / Status | 未进主五项 / 或 `--plan` | 裸 `uninstall` / `analyze` / `status`（已有 TTY 交互或 TUI） |
| purge / installer / history… | 摊在主列表 | **不进主五项**；经 `M` help 发现（对齐 mole） |

### 3.4 实现落点

| 落点 | 动作 |
|---|---|
| 新 | `vole-cli::tui::home_menu`（或 `main_menu`）+ 可单测纯逻辑 `HomeMenuState` |
| 改 | `interactive.rs` → 调 TUI；非 TTY 仍 stderr 提示 + exit 2 |
| 复用 | `Theme`、`TerminalGuard` |
| 不复用 | `PaginatedMultiSelect`（单选导航，不是多选） |
| 输入 | 进入前 **drain** 待读输入（防误 Enter，对齐 mole #726 / T0 契约） |

### 3.5 T5 验收

1. TTY 裸 `vole`：五项 + footer；无虚标未接线键
2. Enter / 数字键启动正确 argv（无多余 `--plan`）
3. 非 TTY / 已带子命令：行为不变
4. Q / Esc / Ctrl+C：终端可恢复
5. README「TUI 交互对齐」补首页一行；注明 Clean/Optimize 确认执行见 T6

## 4. T6–T8 边界、风险与成功判据（§3）

### 4.1 T6：Clean / Optimize 确认双轨

| 命令 | 进交互条件 | 流程 | 不变 |
|---|---|---|---|
| `clean` | stdin+stdout 均为 TTY，且无 `--plan` / `-n` / `--apply` / `--json*` / `--plan-out` / whitelist 系 flag | 现有扫 plan → 人类摘要 → `Proceed? [y/N]` → 内存 plan → 既有 `apply_proto_plan`（或等价 apply 漏斗） | 自动化路径零破坏；删除不另开漏斗 |
| `optimize` | 同上（无 whitelist 系） | 同形：plan → 确认 → apply | `--plan` / `--json*` / 非 TTY 不变 |

- **有意行为变化**：TTY 裸 `vole clean` / `vole optimize` 从「只 plan」变为「可确认后执行」；`--help` 与 README 必须醒目。
- **本波不做**：`optimize --whitelist`（留给 T8）；clean 分页多选单条（mole 亦非此模型）。
- **与 T5 衔接**：T5 已 exec 裸命令；T6 落地后首页进 Clean/Optimize 即真确认流。

### 4.2 T7：analyze 进阶键

接线 mole 已有能力：Space 多选、⌫/Delete 删除、O Open、P Preview、`/` Filter、T Top。

硬约束：

- 删除只走保护 + 废纸篓（或既有 analyze 安全路径）
- **禁止**平行 `rm`
- footer **只声明已接线**键（延续 T3/T4 诚实 footer 原则）

### 4.3 T8：抛光与收口

- status：动画 cat、`k` 隐藏、`c` 循环 core prefs（若成本过高可降级为「文档标不做」）
- `optimize --whitelist`（mole 独立清单）
- README / 对照表勾满 T5–T7；清除「首页仍是数字菜单」类措辞；MINOR 发版

### 4.4 风险

| 风险 | 缓解 |
|---|---|
| T5 后 Clean 仍只 plan，用户以为「坏了」 | README + help 一句：确认执行见 T6 / 下一版本 |
| T6 TTY 默认可删 | 确认文案 `[y/N]` 默认 N；双轨门控单测 |
| analyze 删除误伤 | 复用保护路径语义；fixture + `VOLE_TEST_NO_AUTH=1` |
| exec 不回菜单 | 有意对齐 mole；文档一句说明 |

### 4.5 成功判据

T5 后裸 `vole` 与 `mo` 首页心智一致；T6 后主路径 Clean/Optimize 可确认执行；T7–T8 吃掉 analyze / status / optimize-whitelist 长尾；全程自动化与删除漏斗零破坏。

## 5. 文档与版本

| 项 | 约定 |
|---|---|
| 本 epic | 本文件（T5 详设 + T6–T8 路标） |
| T6/T7/T8 开工 | 可另开窄 design，或直接按本文件路标写 plan |
| `schema_version` | **不 bump** |
| 发版 | 每波合入时按仓惯例 MINOR；不与本 design 绑定具体版本号 |

## 6. 与 T0–T4 的关系

| 波次 | design | 状态 |
|---|---|---|
| T0 | [`2026-08-09-2136-tui-interactive-mole-parity-design.md`](2026-08-09-2136-tui-interactive-mole-parity-design.md) | 已交付（2.7.0） |
| T1 | [`2026-08-09-2224-tui-t1-purge-installer-interactive-design.md`](2026-08-09-2224-tui-t1-purge-installer-interactive-design.md) | 已交付 |
| T2 | [`2026-08-09-2223-tui-t2-whitelist-interactive-design.md`](2026-08-09-2223-tui-t2-whitelist-interactive-design.md) | 已交付 |
| T3 | [`2026-08-09-2220-tui-t3-status-analyze-visual-parity-design.md`](2026-08-09-2220-tui-t3-status-analyze-visual-parity-design.md) | 已交付 |
| T4 | [`2026-08-09-2303-tui-t4-interactive-closeout-design.md`](2026-08-09-2303-tui-t4-interactive-closeout-design.md) | 已交付（**2.8.0**） |
| **T5–T8** | **本文件** | 已批准；实现另开 plan（先 T5） |
