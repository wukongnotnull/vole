# T10：analyze 导航别名 + `S` live sort

- 日期：2026-08-11 12:50
- 状态：已批准（会话：实现 T9 余项 1+2 · 默认采纳 condensed 方案）
- Mole 钉版：`third_party/mole-1.48.1`（`cmd/analyze/update.go` `left`/`h`/`b` / `right`/`l`；`s`/`S` + `live_config.go`）
- 父权威：[`2026-08-11-1201-tui-t9-analyze-f-r-keys-design.md`](2026-08-11-1201-tui-t9-analyze-f-r-keys-design.md) §4「不做」；[`2026-08-10-1514-tui-t5-home-menu-roadmap-design.md`](2026-08-10-1514-tui-t5-home-menu-roadmap-design.md)
- T9 现况：[`docs/releases/v2.13.0.md`](../../releases/v2.13.0.md)（包线 **2.13.0**）
- 包版本意图：合入时 **MINOR**（相对当前 `2.13.0` → **2.14.0**）
- **不 bump** `schema_version`

## 1. 结论

T9 有意留下的两键本波交付：

1. **导航别名**：`←` / `h` / `b` → 返回（与 Esc 同形；Top 内先退出 Top）；`→` / `l` → 进入目录（与 Enter 同形）
2. **`S` live sort**：扫描中切换 `continuous` ↔ `freeze-on-move`；为此引入**根子项渐进进度**（非整盘 cache）

硬约束：

- 删除漏斗零变更；导航 / S **不**触发删除
- footer 声明 `←→`（对齐 mole）；**不**在 footer 写 `S`（mole 亦不声明；状态行 `Live sort: …`）
- `analyze --json` / 非 TTY 仍走整包 `analyze_directory`（无进度回调要求）
- **不做** mole 磁盘 / overview cache
- 不 bump `schema_version`

## 2. 键位契约

### 2.1 导航别名

| 键 | 映射 | 行为 |
|---|---|---|
| `←` / `h` / `H` / `b` / `B` | `AnalyzeKey::Back` | 同 Esc：清 Top/过滤 → 否则 `GoBack` 或根上 Quit |
| `→` / `l` / `L` | `AnalyzeKey::Forward` | 同 Enter：目录则 `EnterDir`；Top / 文件 noop |
| 过滤输入态 | — | 字母进 filter；方向键不映射（忽略） |

Footer（Directory / 有返回时）：`↑↓←→`；根目录无返回：`↑↓→`（对齐 mole overview/根形态精神：无 ← 时可不声明 Back，本波简化为：**有 `can_go_back` 用 `↑↓←→`，否则 `↑↓→`**）。Top 模式：`↑↓←`（可退出 Top，无 Forward）。

### 2.2 `S` live sort

| 项 | 约定 |
|---|---|
| 触发 | `s` / `S`（非 filtering）；**仅** `scanning && !overview && !show_large_files` |
| 模式 | `freeze-on-move`（默认）↔ `continuous` |
| 状态行 | `Live sort: freeze-on-move` / `Live sort: continuous` |
| env | `VOLE_ANALYZE_LIVE_SORT=continuous`；若未设则读 `MOLE_ANALYZE_LIVE_SORT`（同值）；其它/空 → freeze |
| freeze | 扫描中自动按 size 降序重排，直到用户**有效** ↑/↓ 移动光标后停止重排；无效边界键不冻结 |
| continuous | 每次子项进度后重排，并尽量保持当前选中 path |
| 扫完 | 用最终 `AnalyzeOutput` 替换列表；若结束时仍为 freeze 且未冻结过，选中钉在第 0 行（对齐 mole `pinFirstRow`） |
| overview / 非扫描 / Top / delete confirm | 忽略 S |

### 2.3 渐进扫描（支撑 S）

| 项 | 约定 |
|---|---|
| API | `scan_directory_with_progress(root, cancel, on_child: FnMut(&DirEntry))`；`scan_directory` = 空回调包装 |
| analyze | `analyze_directory_with_progress`：每完成一个根子项回调 `AnalyzeEntry`；结束再交完整 `AnalyzeOutput`（含 large_files / 截断 TOP） |
| TUI | 扫描线程经 channel 发 `Child` / `Done` / `Err`；UI 合并 Child 进 `out.entries`（path upsert）、累加 `total_size`；Done 整表替换 |
| 截断 | 进度阶段可暂时超过 `MAX_ENTRIES` 显示；**Done** 后与今日 JSON 口径一致（top 30） |
| JSON | **不**改 `analyze_directory` 签名与行为 |

## 3. 实现落点

| 落点 | 动作 |
|---|---|
| `vole-core::scan` | `scan_directory_with_progress` |
| `vole-core::analyze` | `analyze_directory_with_progress` + 进度事件类型（或回调） |
| `tui/analyze_state.rs` | `Back`/`Forward`/`LiveSort`；`LiveSortMode`；freeze 光标逻辑；env 读取 |
| `tui/widgets.rs` | footer `←→` / `←` / `→` 按模式 |
| `main.rs` `cmd_analyze_tui` | 进度 channel；Child/Done；S 无需额外 effect（状态内完成）或 `None` |
| README / `docs/releases/v2.14.0.md` / Cargo / Formula | 2.14.0 |

## 4. 不做

| 项 | 说明 |
|---|---|
| mole 磁盘 / overview cache | 仍 YAGNI |
| footer 声明 `S Live` | 对齐 mole（仅状态行） |
| 扫描中 Space/⌫/T | 保持 T7 禁用 |
| bump `schema_version` | 无协议字段变化 |

## 5. 验收

1. 单测：键映射；Back/Forward 与 Esc/Enter 同效；live sort 切换与 freeze-on-move；progress 回调顺序
2. `VOLE_TEST_NO_AUTH=1 cargo test -p vole-core -p vole-cli --bin vole`
3. `./scripts/check-command-surface.sh --enforce`；`cargo fmt --all -- --check`
4. TTY：`vole analyze ~` → ← 返回、→ 进入；扫描中 S 状态行切换；列表在 continuous 下随进度重排
5. `analyze --json` 不变

## 6. 成功判据

T10 后：导航别名可用；扫描中真 live sort（非虚标）；footer 诚实；删除/JSON 零破坏；包版本 **2.14.0**。
