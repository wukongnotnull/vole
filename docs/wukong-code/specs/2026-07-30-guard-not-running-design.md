# Guard `not_running` 子集设计

- 日期：2026-07-30
- 状态：已批准（brainstorming：范围 B + 精确进程名）
- 参照：Mole v1.48.1；设计文档 §6.1 Guards；`SkipReason::AppRunning`（协议已冻结，字符串 `app_running`）

## 1. 目标

让 `[rule.guards].not_running` **真正生效**：所列进程任一以精确名在跑（或探测失败）时，整条规则在 plan（及 apply）阶段跳过，发出 `SkipReason::AppRunning`。

顺带：

- 兑现已声明但未执行的规则（`ai-agents.toml` / `codex.toml`）
- 移植/标注一小撮 **路径静态 + 仅需 `pgrep -x`** 的 mole guard

## 2. 非目标

- `pgrep -f` / cmdline 子串匹配（下一轮）
- Final Cut / 剪映 **generated** 动态路径发现
- Simulator / XCTest 多进程复合探测
- 改写已冻结的 `SkipReason` 变体名（只用既有 `app_running`）

## 3. 匹配语义

| 项 | 规定 |
|---|---|
| 输入 | `not_running = ["Name1", "Name2", …]` |
| 匹配 | 精确进程名，对齐 `pgrep -x` |
| 跳过条件 | 任一名字为 Running，**或**探测结果为 Unknown（超时 / `pgrep` 不可用 / 非 0/1 退出） |
| 空列表 | 不探测、不跳过 |
| 空字符串元素 | 忽略 |

实现：`SysCommand` 跑 `pgrep -x <name>`，超时走现有 `SysCommand` 超时路径 → Unknown → fail-closed 跳过。

## 4. 架构

```
Rule.guards.not_running
        │
        ▼
ProcessProbe::exact_name_running(name) → Running | Idle | Unknown
        │
        ├── plan：规则循环开头，命中则 emit Skipped{AppRunning}，不进候选
        └── apply：按 entry.rule_id 查当前规则表，再检一次（不可信 plan / TOCTOU）
```

- 新增 `ProcessProbe` trait（`vole-core`），默认实现 `PgrepProcessProbe`（`vole_sys::macos::MacSysCommand`）。
- 测试用 `FakeProcessProbe { running: HashSet<String> }`；探测错误可配置为 Unknown。
- `Orchestrator` 持有 `Arc<dyn ProcessProbe>`（默认 pgrep）；`build_plan` 使用它。
- `apply_proto_plan` / `ApplyPlanContext` 增加 `rules: &[Rule]` + `process_probe`；CLI apply 路径已加载 rules，传入即可。
- **不**改 Plan JSON schema（靠 rule_id 回查规则，避免把进程名信源放进不可信 plan）。

## 5. 规则子集（本轮）

| 动作 | 规则 | `not_running` |
|---|---|---|
| 兑现既有 | `claude-code-old-versions` 等 ai-agents / codex | 已写，引擎生效即可 |
| 标注既有 | `firefox-cache` | `["Firefox"]` |
| 新增 | `dropbox-cache`（mole 两条路径合并或拆两条同 guard） | `["Dropbox"]` |
| 新增 | `google-drive-cache`（mole 对应 Caches 路径） | `["Google Drive"]` |
| 新增 | `onedrive-cache`（mole 对应） | `["OneDrive"]` |

路径以 mole `clean_cloud_storage` / Firefox 段为准；不碰 Chrome Application Support 段（mole 另有 `-f` 宽匹配，本轮不做）。

## 6. 测试

1. **单元**：`FakeProcessProbe` — 进程在跑 → 无候选 + 收到 `AppRunning`；空闲 → 正常候选；超时/Unknown → 跳过。
2. **Apply**：假 plan entry + 规则带 `not_running` + probe 报 Running → skip，不删。
3. **Fixture**：静态路径选中（进程未跑的默认 probe）；不要求 JSON fixture 伪造真实 pgrep。
4. **回归**：`cargo test -p vole-core`；`verify_clean_fixtures` 全绿。

## 7. 验收

- [ ] `not_running` 非空时 plan/apply 行为符合上表
- [ ] 既有 Claude/Codex 声明生效（单测覆盖）
- [ ] Firefox / Dropbox / Google Drive / OneDrive 子集落地
- [ ] 协议仍为 `app_running`；无 schema_version bump
- [ ] 文档：本 design + findings 短记；coverage 文案可提一句 guard 已开始落地
