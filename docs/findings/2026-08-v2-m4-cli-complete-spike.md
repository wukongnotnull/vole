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

（Task 2 填写）

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
