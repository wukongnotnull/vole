# T3：status / analyze TUI 视觉同构

- 日期：2026-08-09 22:20
- 状态：已批准（父 design §5 T3；会话交付物）
- 父 design：`docs/wukong-code/specs/2026-08-09-2136-tui-interactive-mole-parity-design.md`
- Mole 钉版：`third_party/mole-1.48.1`
- 对照：`cmd/status/view.go`、`cmd/status/main.go` View、`cmd/analyze/view.go` + `format.go`
- Vole 现况：`crates/vole-cli/src/tui/{status_view,analyze_view,theme,widgets}.rs` + `main.rs` 循环
- **不 bump** `Cargo.toml` version / `schema_version`；无 tag/release

## 1. 结论

把 `vole status` / `vole analyze` 的交互 TUI 从「最小可用仪表盘」提升为 mole 级 **区块 / 标签 / 密度 / 窄终端** 同构（ratatui 立即模式），**不**复刻 Bubble Tea / bash 增量光标重绘，**不**改变 JSON / auto-JSON / stream 行为。

成功标准（相对父目标 D 的视觉档 C）：

1. 用户从 mole 迁过来能在同一屏位置读到同名区块与相近标签。
2. 宽屏双列 / 窄屏（≤80）单列可切换，关键信息优先保留。
3. footer 只声明 **已接线** 的快捷键（禁止虚标 Space/O/P/Del 等未实现能力）。
4. 纯布局 helper 可单测；自动化输出路径零回归。

## 2. 范围

### 2.1 做

| 面 | 要求 |
|---|---|
| Theme | mole 色板映射：title `#C79FD7`、primary `#BD93F9`、subtle `#737373`、warn `#FFD75F`、danger `#FF5F5F`、ok `#A5D6A7`、rule `#404040` |
| status 结构 | Header（`Status` + Health ● score + diagnosis）→ 可选 process alert 条 → 卡片区 → footer |
| status 卡片 | CPU / Memory / Disk / Power / Processes / Network（有数据则渲染；空态用 `Collecting...` / `No battery`） |
| status 布局 | `width > 80` 双列；`≤80` 单列；卡片标题 `icon + name` + `╌` 规则线 |
| status 进度条 | 16 格 `█░`，阈值色：≥85 danger / ≥60 warn / else ok（对齐 mole `colorizePercent`） |
| analyze 结构 | Header `Analyze Disk` + path + `Total` → tip → 条目列表 → 可选 Large files 摘要 → footer |
| analyze 行 | 选中 `▶`；序号；相对条；百分比；名；右对齐 size；dir/file 图标；`cleanable` 提示 |
| analyze overview | `out.overview == true` 时标题区用 mole overview 文案节奏（Select a location…），行密度同目录模式 |
| 窄终端 | header 信息按优先级裁剪；卡片行/analyze 名列截断；viewport 按高度计算 |
| 测试 | 纯函数：header 裁剪、进度条、analyze 行字符串、viewport/name width |
| 触达文件 | 优先 `status_view` / `analyze_view` / `theme` / `widgets`；`main.rs` 仅在必须传 width/spinner/footer 状态时最小改动 |

### 2.2 不做（明确）

- purge / installer / whitelist 交互（T1/T2）
- analyze 未接线能力：Space 多选、⌫ 删除、O Open、P Preview、`/` Filter、T Top 切换全屏（可在 Large files 摘要保留只读预览）
- status 动画 mole cat、`k` 隐藏 cat、`c` 循环 core 数持久化（prefs）——记入 T4 长尾
- JSON / `--json-stream` / 非 TTY 人类摘要格式变更
- Formula / Cargo.toml version bump
- 像素级 lipgloss 复刻或第二套 TerminalGuard

## 3. 验收对照表（mole → vole）

### 3.1 status

| # | mole 行为 | vole T3 | 测法 |
|---|---|---|---|
| S1 | 标题 `Status` + `Health ● N` + diagnosis | 同构文案；score 色分档 | helper 单测 + 手工 TTY |
| S2 | 宽屏双列卡片、窄屏单列 | `area.width > 80` 分支 | helper 单测 layout mode |
| S3 | 卡片：CPU / Memory / Disk / Power / Processes / Network | 同序；缺电池显示 `No battery` | 结构单测 |
| S4 | CPU：`Total` 条 + load 行；可选 top cores | Total+Load 必有；per-core 最多 2（无 prefs） | 行标签断言 |
| S5 | Memory：Used/Free 条 + Total/Avail/pressure | 同标签族 | 行标签断言 |
| S6 | Disk：`INTR`/`EXTR` + SMART + I/O | 用 `external`/`smart_status`/`disk_io` | 行标签断言 |
| S7 | Processes：`#n` + bar + cpu% + mem + name | top 3 | 行格式断言 |
| S8 | Network：Down/Up + rate；Proxy/IP 可选 | `network` + `network_history` sparkline 可选简化为条/率 | 行标签断言 |
| S9 | process alert 黄底条 | 有 `process_alerts` 时顶栏一行 | 有/无断言 |
| S10 | 窄 header 丢 OS/uptime，保 model+RAM/Disk | 优先级候选列表 | `fit_status_header` 单测 |
| S11 | footer/keys：`q`/`esc`/`ctrl+c` 退出 | footer 声明；行为不变 | 文案断言 |
| S12 | JSON 路径不变 | 不改 collector / proto 序列化 | 既有 CLI 测 |

### 3.2 analyze

| # | mole 行为 | vole T3 | 测法 |
|---|---|---|---|
| A1 | 标题 `Analyze Disk` + path + `Total:` | 同构；scanning 时示 spinner/文案 | helper 单测 |
| A2 | overview：`Select a location to explore:` | `overview` 真时启用 | 文案断言 |
| A3 | 行：`▶` / 序号 / bar / % / 名 / size | 同构（无多选 ○/●，因未接线） | `format_analyze_row` 单测 |
| A4 | 百分比着色阈值（≥50/20/5） | theme 分档 | 单测 |
| A5 | viewport = height − reserved，clamp 1..=30 | `calculate_viewport` | 单测 |
| A6 | name 列宽随终端 | `calculate_name_width` | 单测 |
| A7 | footer 随模式变化 | **仅声明已接线**：`↑↓` `Enter` `Esc Back` `Q/Ctrl+C Quit` | 文案断言 |
| A8 | Large files 区 | 底部只读摘要（最多 4），标题 `Large files` | 渲染存在性 |
| A9 | local_snapshots tip | 保留现有 tip 行 | 行为不变 |
| A10 | JSON / 后台扫描 / cancel 130 | 不改 `cmd_analyze` JSON 枝与 cancel | 既有路径 |

## 4. 实现要点

### 4.1 纯布局层

在 `widgets` / 各 view 内抽出：

```text
plain_progress_bar(percent) -> String          // 16×█░
color_bucket(percent) -> Ok|Warn|Danger
fit_status_header(parts, width) -> String
status_layout_mode(width) -> Single|TwoColumn
format_analyze_row(entry, idx, selected, max_size, total, name_width) -> String
calculate_viewport(term_height, reserved) -> usize
calculate_name_width(term_width) -> usize
format_bytes_bin / format_bytes_si                        // status 用 bin 口径文案；analyze 贴近 mole SI
status_footer() / analyze_footer(can_go_back) -> String
```

`render_*` 只把上述字符串/行喂给 ratatui `Paragraph`/`List`，便于无 TTY 单测。

### 4.2 Theme

`Theme::default()` 改为 mole 色；保留 `ok`/`warn`/`danger`/`label`/`value`/`selected`/`title`/`subtle`/`primary`/`rule`。

### 4.3 main.rs（最小）

- status：仍只读键 `q`/`esc`/`ctrl+c`；渲染签名可增 `anim` 无关字段时优先不加。
- analyze：可把 `spinner` 帧或 `tick` 传入 view（扫描行动画）；**不**接线新业务键。

### 4.4 冲突隔离

避免改 `paginated_select` / `menu_state` / uninstall 路径；T1/T2 并行时本 PR 冲突面应限于 theme 色值（可接受）与 `mod.rs` re-export（尽量不动）。

## 5. 测试计划

1. `cargo test -p vole-cli`（macOS）：新 layout 单测全绿。
2. 手工：`vole status`、`vole analyze ~` 在 ≥100 列与 ≤80 列终端各看一眼。
3. `vole status --json` / `vole analyze --json` 字段集与行为不变（抽查或既有测）。
4. Linux CI：不强制整仓 build；若 CI 已跳过 vole-cli darwin，文档测仍随 PR 走。

## 6. 风险

- 卡片信息变多可能挤掉 tip/footer → viewport 与单列优先保障 footer 1 行。
- 色板变更影响 paginated select 外观（共用 Theme）→ 可接受的一致性收益；若冲突过大则 status/analyze 用局部 `StatusTheme`（默认不拆）。
- 不引入 prefs / 动画 cat，避免与 mole 键位「看似对齐实则半残」——footer 诚实。

## 7. 成功判据（一句话）

T3 后：TTY 上 `status`/`analyze` 的区块标签、双列/窄屏节奏与 mole 同构；JSON 自动化零破坏；未接线能力不出现在 footer。
