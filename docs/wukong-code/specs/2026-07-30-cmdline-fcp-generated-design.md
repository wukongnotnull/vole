# cmdline `pgrep -f` + Final Cut Pro generated 设计

- 日期：2026-07-30
- 状态：已批准（brainstorming：打包 A）
- 前置：`2026-07-30-guard-not-running-design.md`

## 1. 目标

1. 扩展 process guard：支持 cmdline 子串探测（对齐 `pgrep -f`）。
2. 移植 mole `clean_final_cut_pro_generated_caches`：仅 `~/Movies` 下 `.fcpbundle` 内可再生成媒体目录；FCP 在跑则整条跳过。

## 2. Schema

```toml
[rule.guards]
not_running = ["Final Cut Pro"]                 # 既有：pgrep -x
not_running_cmdline = ["/Final Cut Pro.app/"]   # 新增：pgrep -f
```

- 任一列表命中 Running/Unknown → 跳过（`AppRunning`）。
- 空列表不探测；空字符串忽略。
- **不做**结构化 `{exact, cmdline}` 联合对象（YAGNI；两字段足够）。

## 3. ProcessProbe

```rust
fn exact_name_running(&self, name: &str) -> ProcessState;
fn cmdline_substring_running(&self, needle: &str) -> ProcessState; // 新增
```

- 默认：`pgrep -f <needle>`，映射同 `state_from_pgrep_status`。
- `should_skip_for_not_running` 重命名或扩展为同时检查两字段；plan/apply 传入整个 `GuardsConfig` 或两个 slice。
- `FakeProcessProbe` 增加 `cmdline_running` / `cmdline_unknown` 集合。

## 4. FCP generated

| 项 | 规定 |
|---|---|
| rule id | `final-cut-pro-generated-cache` |
| label | `Final Cut Pro generated cache` |
| paths | `["~/Movies/*.fcpbundle"]`（只展开 library 根） |
| strategy | `custom` / `handler = "final_cut_pro_generated_caches"` |
| 选中 | 库内相对路径匹配 `*/Render Files/High Quality Media` 或 `*/Transcoded Media/Proxy Media` |
| 排除 | Original Media、Analysis Files、Motion Templates、Backups、plist/flexolibrary、symlink 库、非 Movies 下的 bundle |
| guards | `not_running` + `not_running_cmdline` 如上 |

Handler 忽略「是否已在 entries 里」的深层路径，对每个 library 目录做有界 walk（对齐 mole：不跟随 symlink；遇保护组件 prune）。

## 5. 非目标

- 剪映 generated、Simulator/XCTest、Chrome 批量 cmdline 回填
- FDA/TCC 行为变更
- schema_version bump

## 6. 验收

- cmdline 单测：`-f` 命中 / 空闲 / Unknown fail-closed
- FCP handler 单测或 fixture：只选安全目录；Documents 下 bundle 不选
- plan：FCP 进程/cmdline 命中 → 无候选 + `AppRunning`
- `cargo test -p vole-core` 绿；规则计数 +1
