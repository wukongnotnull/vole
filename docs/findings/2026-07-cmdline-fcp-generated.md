# cmdline `pgrep -f` + Final Cut Pro generated

**日期**：2026-07-30  
**状态**：已落地  
**设计**：`docs/wukong-code/specs/2026-07-30-cmdline-fcp-generated-design.md`

## 引擎

- `guards.not_running_cmdline` → `pgrep -f`（与 `not_running` / `pgrep -x` 并存）
- `should_skip_for_guards`：任一列表 Running/Unknown → `AppRunning`
- plan + apply 均生效

## 规则

| id | 说明 |
|---|---|
| `final-cut-pro-generated-cache` | `~/Movies/*.fcpbundle` → custom handler；只选 Render HQ / Proxy Media；FCP `-x` + `/Final Cut Pro.app/` `-f` |

规则总数：**474**

## 非目标（仍未做）

~~剪映 generated~~（见 `2026-07-jianyingpro-generated.md`）、Simulator/XCTest、Chrome 批量 cmdline 回填
