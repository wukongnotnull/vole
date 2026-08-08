# M4：CLI 做全 · Mole 库存与安全面 Spike

**日期**：2026-08-08  
**状态**：进行中（骨架）  
**Mole 钉版**：`third_party/mole-1.48.1`  
**规格**：[`2026-08-08-2030-v2-cli-complete-design.md`](../wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md)  
**计划**：[`2026-08-08-2051-v2-m4-cli-complete-spike.md`](../wukong-code/plans/2026-08-08-2051-v2-m4-cli-complete-spike.md)  
**命令面表**：[`2026-08-v2-m4-command-surface.md`](2026-08-v2-m4-command-surface.md)

## 1. 结论

（Task 10 收口填写）本 spike **docs-only**：核定 Mole 1.48.1 命令面与六命令+hints 主路径/长尾，留下 §3.2 闸门 stub；**不**实现产品行为、**不** bump `2.0.0`。

## 2. §3.1 核定摘要

见 [`2026-08-v2-m4-command-surface.md`](2026-08-v2-m4-command-surface.md)。骨架已含规格全部行；核定列待 Task 8/10。

## 3. 分命令库存

### 3.1 `purge`（→ M5）

**Mole 入口：** `bin/purge.sh` → `start_purge` / `perform_purge`（逻辑在 `lib/clean/project.sh` + `lib/clean/purge_shared.sh`）。  
**配置：** `~/.config/mole/purge_paths`；`--paths` → `lib/manage/purge_paths.sh` `manage_purge_paths`。  
**对照 bats：** `tests/purge.bats`、`purge_config_paths.bats`。

| 面 | Mole 事实 |
|---|---|
| Flag | `--dry-run`/`-n`、`--paths`、`--include-empty`、`--debug`、`--help`（**无**独立 `--apply`；TTY 确认后删除） |
| Targets | `MOLE_PURGE_TARGETS`：`node_modules` `target` `build` `dist` `venv` `.venv` `.pytest_cache`（7） |
| 默认搜索根 | `www` `dev` `Projects` `GitHub` `Code` `Workspace` `Repos` `Development` `Library/CloudStorage` `$HOME` `.codex/worktrees` `.claude/worktrees`（12；点目录仅显式） |
| 指标 | monorepo 4；project indicators 16（含 `package.json`/`Cargo.toml`/`.git` 等） |
| 年龄 / 深度 | `MIN_AGE_DAYS=7`；depth 默认 1–6 |
| 超时 | 扫描约 60s（`MO_PURGE_SCAN_TIMEOUT_SEC`）；activity total / size du 有界；超时 fail-closed |
| 删除 | 经 `mole_delete`；云同步项非交互跳过或需确认 |

**Vole 映射建议：** Mole dry-run ≈ `--plan`；交互确认删除 ≈ `--apply` + TTL/TOCTOU；增 JSON / `plan_out` / `--permanent`；配置名 `purge_paths`（或 `~/.config/vole/...` 等价）。

**主路径（建议进 M5 / 2.0.0）**

1. 发现项目根（默认搜索路径 + `$HOME/*/` 容器探测；点目录仅显式列表）
2. 按钉版 `PURGE_TARGETS` 匹配重建型产物
3. 年龄门槛（默认 7 天）与扫描/探测超时
4. `--plan` / `--apply` 两阶段；JSON；默认废纸篓
5. 删除走 `mole_delete_verified` + 保护层；`purge_paths` 配置
6. 菜单 + 补全；建议同 PR 交付 §3.3 别名

**长尾**

- TTY 分页多选 UI（用 plan JSON 代替）
- 整棵 worktree「可删」判定（Mole AGENTS 禁止；只清产物）
- 未在钉版 targets 证明的扩张目录名
- sudo / 系统域；cloud 特殊交互的全量复刻

**安全面：** 禁止平行 `rm -rf`；gitignore/秘密不能当可删证据；与规格 §6.1–6.2 对齐。

### 3.2 `hints`（→ M6，非顶层命令）

（Task 3 填写）

### 3.3 `installer`（→ M7）

（Task 4 填写）

### 3.4 `touchid`（→ M8）

（Task 5 填写）

### 3.5 `update`（→ M9）

（Task 6 填写）

### 3.6 `remove`（→ M10）

（Task 7 填写）

## 4. 别名与裸调用

（Task 8 填写）

## 5. 主路径 vs 长尾总表

（各命令 Task 填完后汇总）

## 6. 安全面与禁区

- 删除类命令（purge / installer / remove）必须走既有安全漏斗；禁止平行 `rm -rf`
- 不删本地快照（apply）；不删 `/Library/Updates`、`/macOS Install Data`
- `hints` 只读；`touchid` 验证禁真授权挂起；`update` 校验 fail-closed；裸调用不联网

## 7. §3.2 闸门草案

（Task 9：checklist + `scripts/check-command-surface.sh`）

## 8. 后续 design 输入清单（M5–M10）

（Task 10 填写完整必答表）

## 9. 明确未做

- 无 `crates/` / `data/rules` 产品行为变更
- 无包版本 bump
- §3.2 完整 CI 强制留给收口；M4 仅 stub + 清单
