# Guard `not_running` 子集落地

**日期**：2026-07-30  
**状态**：已落地（plan + apply 生效）  
**设计**：[`docs/wukong-code/specs/2026-07-30-guard-not-running-design.md`](../wukong-code/specs/2026-07-30-guard-not-running-design.md)

---

## 1. 引擎行为

`[rule.guards].not_running` 在 **plan** 与 **apply** 阶段均生效：

| 项 | 行为 |
|---|---|
| 探测 | `PgrepProcessProbe` 经 `SysCommand` 执行 `pgrep -x <name>`（超时 2s） |
| 匹配 | 精确进程名；空列表不探测；空字符串元素忽略 |
| 跳过 | 任一名字为 **Running**，或探测结果为 **Unknown**（fail-closed） |
| 事件 | `SkipReason::AppRunning`（协议字符串 `app_running`，无 schema bump） |
| apply | 按 `entry.rule_id` 回查当前规则表再检（防不可信 plan / TOCTOU） |

实现：`ProcessProbe` trait、`should_skip_for_not_running` helper；`Orchestrator` 默认注入 `PgrepProcessProbe`；`ApplyPlanContext` 持有 `rules` + `process_probe`。

---

## 2. 规则子集（本轮）

| 动作 | 规则 | `not_running` |
|---|---|---|
| 兑现既有 | `claude-code-old-versions` 等（`ai-agents.toml` / `codex.toml`） | 已声明，引擎生效 |
| 标注既有 | `firefox-cache` | `["Firefox"]` |
| 新增 | `dropbox-cache` | `["Dropbox"]` |
| 新增 | `google-drive-cache` | `["Google Drive"]` |
| 新增 | `onedrive-cache` | `["OneDrive"]` |

路径对齐 mole `user.sh` / `clean_cloud_storage`；Chrome Application Support 宽匹配不在本轮。

---

## 3. 非目标（仍未移植）

- `pgrep -f` / cmdline 子串匹配
- Final Cut / 剪映 **generated** 动态路径发现
- Simulator / XCTest 多进程复合探测
- 改写 `SkipReason` 变体名

`coverage_note` 已更新：部分 guard 已落地；generated / cmdline 类仍未移植。

---

## 4. 规则规模

`data/rules/*.toml` 共 **473** 条 `[[rule]]`（较 Phase 4c+ 收官 470 净增 3：Dropbox / Google Drive / OneDrive cache）。
