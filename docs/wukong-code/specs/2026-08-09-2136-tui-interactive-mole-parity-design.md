# TUI 完整交互 mole 级复刻（首刀：分页多选 + uninstall）

- 日期：2026-08-09 21:36
- 状态：已批准（会话 brainstorming：目标 D · 路线 1 · 双轨 · 契约 A · §1–§3 ok）
- Mole 钉版：`third_party/mole-1.48.1`
- 对照：`lib/ui/menu_paginated.sh`、`bin/uninstall.sh` 交互路径、`lib/ui/app_selector.sh`
- 包版本意图：下一 **MINOR**（合入时相对当时 `Cargo.toml` 递增；TTY 裸 `uninstall` 行为变化，不空 bump）
- **不 bump** `schema_version`

## 1. 结论

产品目标（路线图级）：vole 交互 TUI 做到 mole 级——共享分页多选契约 + `status`/`analyze` 视觉同构；覆盖 uninstall / purge / installer / whitelist 等交互面。

**本 design 只交付 T0：**

1. `vole-cli::tui` 共享组件 `PaginatedMultiSelect`（契约档 A）
2. `vole uninstall` 双轨：TTY 且无显式自动化 flag → 多选 → 确认 → 现有 `apply_uninstall_plan`；其余路径行为不变

编排 / 删除 / 保护仍在 `vole-core`；UI 只在 `vole-cli`。禁止第二条删除路径。

## 2. 目标档位与首刀选择（已拍板）

| 项 | 选择 |
|---|---|
| 成功标准 | **D**：UX 契约（B）+ 视觉同构（C），覆盖 mole 全部交互命令面 |
| 首刀 | **A**：共享 `paginated_multi_select` → 先挂 `uninstall` |
| TTY vs 自动化 | **双轨**：TTY 无显式 flag → 交互；`--plan`/`--apply`/`--json*`/非 TTY 保持现路径 |
| 契约严格度 | **A**：核心键位+语义+排序过滤+预选；env 用 `VOLE_MENU_*`（可选兼容读 `MOLE_MENU_*`） |
| 实现路线 | **1**：CLI 共享 ratatui 组件（不把选择会话塞进 core；不仿 bash 增量光标重绘） |

## 3. 架构（T0）

### 3.1 组件 API

```text
select(title, items: &[MenuItem]) -> Result<Vec<usize>, Cancelled>

MenuItem {
  label: String,
  filter_name: Option<String>,
  epoch: Option<i64>,
  size_kb: Option<u64>,
}
```

- 返回值为**原始 items 下标**（过滤/排序后的视图索引须映射回原始下标）。
- 空 `items` → 失败（对齐 mole「No items provided」）。
- 取消（Q / Esc，且无进行中的过滤清除语义）→ `Cancelled`；调用方退出 0、不删。

### 3.2 终端与恢复

- 复用既有 `TerminalGuard`（alt-screen / raw / mouse 策略与现 status/analyze 一致；禁止第二套恢复路径）。
- panic / SIGINT / SIGTERM：先恢复终端再退出（既有语义）。
- 进入选择前 **drain** 待读输入，防止扫描阶段误敲 Enter 直接确认（对齐 mole #726）。

### 3.3 契约档 A（验收表）

| 能力 | 要求 |
|---|---|
| 导航 | ↑ / ↓；按终端高度算每页条数（reserved≈5，clamp 1..=50） |
| 多选 | Space 切换当前项 |
| 确认 | Enter → 返回当前选中集合（允许空集由调用方处理） |
| 取消 | Q / Esc；若正在过滤则先清空过滤再取消 |
| 排序 | `date` \| `name` \| `size`；无 metadata 时强制 `name` 并禁用排序控件 |
| 过滤 | 增量过滤（对 `filter_name` 或 `label`） |
| 预选 | 支持初始选中下标集合（对应 mole `MOLE_PRESELECTED_INDICES` 语义） |
| 忽略首 Enter | `VOLE_MENU_IGNORE_INITIAL_ENTER`（扫描后进入时建议开启） |
| env | `VOLE_MENU_SORT_MODE` / `VOLE_MENU_SORT_REVERSE` / `VOLE_MENU_IGNORE_INITIAL_ENTER`；可选兼容 `MOLE_MENU_*` |

不要求：bash 增量行重绘、footer 自适应裁剪的像素级复刻。

## 4. `uninstall` CLI 语义（T0）

### 4.1 进入交互的条件（全部满足）

- `stdin` 与 `stdout` 均为 TTY
- 未指定：`--plan` / `--dry-run` / `-n` / `--apply` / `--json` / `--json-stream` / `--plan-out`
- 未带 `target` 位置参数

任一不满足 → **保持现有 plan/apply 自动化路径**（行为不变）。

### 4.2 交互流程

1. 复用现有扫描 / 保护逻辑，得到**应用级**候选（粒度是 app，不是 plan 里每条残留路径）。
2. drain → `PaginatedMultiSelect`。
3. 取消 → 退出 0，不删。
4. 空选 → 提示后重新进入选择（对齐 mole），不误删。
5. 有选中 → 离开 alt-screen → 打印摘要 → `Proceed with uninstallation? [y/N]`。
6. 非 `y`/`Y` → `Aborted.`，退出 0。
7. 确认 → **内存中**为选中项构造 `ProtoPlan` → `apply_uninstall_plan`（保护 / 废纸篓 / oplog / sibling guard 全复用）。
8. 人类摘要走 stderr；计数对齐现有 human report。

### 4.3 flag 微调

| 项 | 行为 |
|---|---|
| `--permanent` | 交互路径允许单独使用（放宽现有 `requires = apply`）；确认后 apply 用永久删除 |
| `--plan` / `-n` / `--json*` / `--plan-out` / `--apply` / `target` | 永不进交互 |
| 非 TTY | 永不进交互；默认仍产出 plan（现行为） |

### 4.4 有意行为变化

TTY 下裸 `vole uninstall`：从「打印 plan」改为「多选并可能删除」。`--help` 与 README 必须写明双轨。

## 5. 里程碑（相对目标 D）

| 波次 | 内容 | 本 design |
|---|---|---|
| **T0** | `PaginatedMultiSelect` + `MenuContract` 测试 + `uninstall` 双轨 | **本文件** |
| **T1** | 挂载 `purge` / `installer` 交互多选 | 另开 design |
| **T2** | `whitelist` 切到同组件 | 另开 design |
| **T3** | `status` / `analyze` 视觉同构 | 另开 design（可与 T1 并行） |
| **T4** | 收口：对照表勾选 + README「TUI 交互对齐」+ 清 coverage 长尾 | [`2026-08-09-2303-tui-t4-interactive-closeout-design.md`](2026-08-09-2303-tui-t4-interactive-closeout-design.md)（**2.8.0**） |

## 6. 测试与验收（T0）

1. 单元：`MenuContract` 表驱动覆盖 §3.3。
2. CLI：非 TTY / `--plan` / `--json` / `target` 断言不进交互。
3. TTY：expect 或伪终端覆盖取消、空选重入；确认路径用 fixture + 注入，不挂真授权。
4. `VOLE_TEST_NO_AUTH=1`；apply 仍走既有保护与删除漏斗。
5. 终端异常退出可恢复（复用 `TerminalGuard`）。
6. 文档：`--help` / README 双轨说明；去掉「无 TTY 多选」类长尾措辞（若有）。

## 7. 明确不做（T0）

- status/analyze 视觉同构
- purge / installer / whitelist 挂载（仅预留组件 API）
- 选择会话协议进入 `vole-core` / 桌面
- bash `menu_paginated.sh` 增量光标重绘实现细节
- bump `schema_version`

## 8. 风险

- TTY 默认从「只 plan」变为「可删」→ 确认文案与 help 必须醒目。
- 候选粒度必须是 **app**，避免把残留路径条目直接塞进多选导致误操作面爆炸。
- 不引入平行 `rm` / 绕过 `apply_uninstall_plan` 的捷径。

## 9. 成功判据（一句话）

T0 后：mole 用户在 TTY 上对 `vole uninstall` 的心智模型成立；脚本与 conformance 零破坏；后续交互面复用同一组件推进到目标 D。
