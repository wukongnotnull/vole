# T7：analyze 进阶键（Space / ⌫ / O / P / `/` / T）

- 日期：2026-08-10 18:59
- 状态：已批准（父 design §3 / §4.2 T7；会话指令：父批准即本窄 design 批准，不另等 OK）
- Mole 钉版：`third_party/mole-1.48.1`（`cmd/analyze/update.go` 键位、`delete.go` 废纸篓、filter 输入）
- 父权威：[`2026-08-10-1514-tui-t5-home-menu-roadmap-design.md`](2026-08-10-1514-tui-t5-home-menu-roadmap-design.md) §3 T7 / §4.2
- T3 留白：[`2026-08-09-2220-tui-t3-status-analyze-visual-parity-design.md`](2026-08-09-2220-tui-t3-status-analyze-visual-parity-design.md) §2.2（本波吃掉 analyze 未接线键）
- 现况：`crates/vole-cli/src/tui/analyze_view.rs` + `main.rs` `cmd_analyze_tui`（仅 ↑↓ / Enter / Esc / Q）
- 包版本意图：合入时 **MINOR**（相对当前 `2.10.0` → **2.11.0**）
- **不 bump** `schema_version`

## 1. 结论

为 `vole analyze` TTY 交互接线 mole 已有进阶键：

| 键 | 行为 |
|---|---|
| Space | 多选切换（路径为键；overview / 扫描中禁用） |
| ⌫ / Delete | 对多选集合或当前行发起删除确认 → Enter 确认 / Esc 取消 |
| O | `open` 打开选中项（多选上限 20） |
| P | Quick Look 预览（单文件；目录忽略） |
| `/` | 名称过滤（输入中吞键；Enter 应用；Esc 清空） |
| T | 切换「Top / Large files」全屏列表（扫描中禁用；overview 不进） |

硬约束：

1. **删除只走保护 + 废纸篓漏斗**：复用 `vole_core::delete::mole_delete`，固定 `DeleteMode::Trash`；**禁止**平行 `rm` / 第二条删除路径。
2. **footer 只声明已接线键**（延续 T3/T4 诚实 footer）。
3. JSON / `--json` / 非 TTY 路径零破坏；不 bump `schema_version`。

## 2. 键位与模式契约

### 2.1 导航态（目录列表，非 overview）

| 键 | 规则 |
|---|---|
| Space | 扫描完成后切换 `multi_selected[path]`；状态行示 `N selected, <size>` |
| ⌫ / Delete | 扫描完成后；有多选则针对集合，否则当前行；进入 `delete_confirm` |
| O | 多选优先（≤20），否则当前行；`/usr/bin/open <path>` |
| P | 仅当前行且 `!is_dir`；`qlmanage -p` 或等价 Quick Look（失败时状态行提示） |
| `/` | 进入 `entry_filtering`；对 `entries_all` 做子串过滤（大小写不敏感） |
| T | 有 `large_files` 时切换 `show_large_files`；清空双方多选/过滤焦点 |

### 2.2 overview / 扫描中

| 能力 | overview | scanning |
|---|---|---|
| Space / ⌫ / T | 禁用 | 禁用（状态行一句提示） |
| O / P | 允许当前行（mole 亦允许 open） | 允许 |
| `/` | 禁用 | 禁用 |

### 2.3 Large files（T Top）全屏

- 列表源：`out.large_files`（可再经 `large_filter`）。
- Space / ⌫ / O / P / `/` 语义同目录模式（针对 large 列表与 `large_multi_selected`）。
- Esc：有过滤先清过滤；否则退出 Top 回目录列表。
- Enter：noop（对齐 mole；不进入目录）。

### 2.4 删除确认

```
Delete: <name|N items>, <size>  Press Enter to confirm  |  ESC cancel
```

- Enter → 对路径集合调用 `mole_delete(..., DeleteMode::Trash, ...)`（更深路径优先，防父子冲突）。
- 保护拒绝 / 白名单 / 消失 / trash 失败 → 状态行报错，不 panic；成功项从当前列表移除并重算 total。
- 测试：`VOLE_TEST_NO_AUTH=1` + `MOLE_TEST_TRASH_DIR`（或仓既有 test trash 约定）；不挂真 Finder / sudo。

### 2.5 Filter

- `/` 进入输入；字符追加、⌫ 删字、Space 为字面空格。
- 实时过滤可见列表；**改查询清空多选**（防隐藏行被操作）。
- Enter：退出输入保留过滤；Esc：清空过滤并退出输入。

## 3. 实现落点（预期）

```
docs/wukong-code/specs/2026-08-10-1859-tui-t7-analyze-advanced-keys-design.md
crates/vole-cli/src/tui/analyze_state.rs   # 新建：AnalyzeState + 键处理纯逻辑
crates/vole-cli/src/tui/analyze_view.rs    # 行 ○/●、Top 全屏、filter/confirm UI、footer 入参
crates/vole-cli/src/tui/widgets.rs         # analyze_footer(mode) 诚实声明
crates/vole-cli/src/tui/mod.rs             # re-export
crates/vole-cli/src/main.rs               # cmd_analyze_tui 接 AnalyzeState；删除/open/preview 副作用
crates/vole-core/...                      # 仅在必要时暴露薄封装；优先复用 mole_delete + AppProtection
README.md / docs/releases/v2.11.0.md      # 勾 T7；去掉「有意未接线 analyze 删除/多选…」
Cargo.toml / Formula/vole.rb              # MINOR 2.11.0（发版 Task）
```

**不做（T7）：**

| 项 | 说明 |
|---|---|
| F File（Finder reveal） | 父 T7 未列；留给后续 |
| R Refresh | 父 T7 未列 |
| j/k / ←→ 额外导航别名 | 可选增强，非本波必做 |
| status cat / optimize --whitelist | T8 |
| bump `schema_version` | 无协议变化 |
| 平行 `rm` | 硬禁止 |

## 4. Footer 诚实规则

`analyze_footer` 改为按模式拼装，**只含已接线**：

| 模式 | 示例片段 |
|---|---|
| 目录（可回退） | `↑↓ \| Space \| Enter \| / Filter \| O Open \| P Preview \| ⌫ Del \| T Top \| Esc Back \| Q/Ctrl+C Quit` |
| 目录（根） | 同上，`Esc/Q Quit` |
| Top | `↑↓ \| Space \| / Filter \| O Open \| P Preview \| ⌫ Del \| Esc Back \| Q/Ctrl+C Quit` |
| filter 输入 | `Filter: type… Enter apply \| Esc clear` |
| delete 确认 | `Enter confirm \| Esc cancel` |

更新既有断言：`footer_omits_unwired_actions` / `analyze_footer_modes` → 改为断言**已接线键出现**、**未做键（如 `F File` / `R Refresh`）不出现**。

行渲染：多选时用 `●`/`○` 前缀（对齐 mole）；无多选时保持 `▶` 选中标记。

## 5. 测试与验收

1. **单元（优先）**：`AnalyzeState` 表驱动——Space 切换、filter 应用/清空并清多选、delete 确认态进入/取消、overview/scanning 禁用、Top 切换、footer 文案。
2. **删除安全**：fixture 路径经 `mole_delete` + `MOLE_TEST_TRASH_DIR`；保护路径拒绝；无 `std::fs::remove_*` 旁路。
3. **Open/Preview**：可测纯函数「构建 argv / 跳过目录 preview」；真 `open`/`qlmanage` 不在 CI 强制成功。
4. **回归**：`analyze --json` / 非 TTY；既有 layout 单测；`VOLE_TEST_NO_AUTH=1`。
5. **版本**：`2.11.0`；不 bump `schema_version`。

## 6. 风险

| 风险 | 缓解 |
|---|---|
| analyze 删除误伤系统/用户关键路径 | 强制 `validate_path_for_deletion` + `AppProtection`；确认默认需 Enter |
| 测试触发真 Trash / auth | `VOLE_TEST_NO_AUTH` + test trash dir |
| footer 虚标 | 单测钉死未接线键缺席 |
| Top 与底部 Large files 摘要重复 | Top 全屏时隐藏底部只读摘要 |

## 7. 成功判据

T7 后：TTY `vole analyze` 可用 Space/⌫/O/P/`/`/T；删除只进保护+废纸篓；footer 诚实；JSON 自动化零破坏；包版本 **2.11.0**。
