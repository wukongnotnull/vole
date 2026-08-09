# TUI T2：whitelist 交互切到共享分页多选

- 日期：2026-08-09 22:23
- 状态：已批准（父 design §5 T2；本会话 condensed brainstorming 默认采纳）
- 父设计：[`2026-08-09-2136-tui-interactive-mole-parity-design.md`](./2026-08-09-2136-tui-interactive-mole-parity-design.md)
- Mole 钉版：`third_party/mole-1.48.1`
- 对照：`lib/manage/whitelist.sh`（`manage_whitelist_categories`）+ `lib/ui/menu_paginated.sh`
- 复用：`run_paginated_select` / `MenuState` / `MenuConfig.preselected`（PR #117）
- **不 bump** `Cargo.toml` / Formula / tag / `schema_version`

## 1. 结论

将 `vole clean --whitelist` 的 TTY 交互从 stdin `[a]/[r]/[q]` 简易菜单，改为与 mole `manage_whitelist` 同构的**预定义缓存目录分页多选**：

1. 展示 mole 对齐的 clean 白名单目录（display name）
2. 已在白名单中的项预选并置顶
3. Enter 保存选中预定义项 + 保留自定义 pattern；Q/Esc 取消且不写盘
4. `--whitelist-add` / `--whitelist-remove` / `--whitelist-list` 自动化路径行为不变

本波次**不**做：`purge`/`installer`（T1）、`status`/`analyze`（T3）、`optimize --whitelist`（vole 尚无该 flag；mole 有独立 optimize 清单，另开）。

## 2. 目标与非目标

| 做 | 不做 |
|---|---|
| clean `--whitelist` TTY → `run_paginated_select` | 碰 T1/T3 代码路径 |
| mole 级 catalog + 预选置顶 + 自定义 pattern 保留 | 改 clean 扫描时 `load_clean()` 缺省文件语义（见 §5） |
| 取消不写盘；确认整表 `save_clean` | bump 版本 / Formula / tag |
| flag 自动化与非 TTY 错误提示保持 | bash 增量光标重绘像素级复刻 |

## 3. 架构

```text
vole clean --whitelist
  ├─ --whitelist-add/remove/list  → 现有 vole-core::whitelist I/O（不变）
  ├─ --whitelist 且非 TTY         → InvalidInput（提示用 flag）
  └─ --whitelist 且 TTY
        → load 当前 patterns（缺文件时菜单侧用 mole defaults 作预选种子）
        → build menu（catalog；已选置顶；preselected 下标）
        → run_paginated_select("Whitelist Manager…", …)
        → Cancelled → 提示，exit 0，不写盘
        → Confirmed → merge(selected predefined + custom) → save_clean
```

### 3.1 模块边界

| 位置 | 职责 |
|---|---|
| `vole-core::whitelist` | 既有 load/save/add/remove；新增 catalog 常量、`patterns_equivalent`、`default_clean_patterns`、纯函数 `build_clean_whitelist_menu` / `merge_whitelist_selection`（可单测） |
| `vole-cli::clean` | TTY 接线：组 `MenuItem`、调 `run_paginated_select`、打印摘要；删除 `a/r/q` 循环 |
| `vole-cli::tui` | **不改契约**；复用 `MenuConfig.preselected`、`SortMode::Name`（catalog 无 epoch/size） |

### 3.2 Catalog

- 源：mole `get_all_cache_items`（`display_name|pattern|category`）；vole 存 `(display_name, pattern)`，category 仅文档用途可省略。
- pattern 字面量使用 `$HOME/...` 或 mole 等价；运行时按 `HOME` 展开比较，写入时压成 `~/...`（对齐 mole `# Convert back to portable format with ~`）。
- `FINDER_METADATA` sentinel 原样保留（与 mole 一致，不展开为路径）。

### 3.3 菜单构建（对齐 mole）

1. `current` = 配置文件存在则 `load_clean()`；**文件不存在**则种子为 `default_clean_patterns()`（仅管理会话；见 §5）。
2. 遍历 catalog：展开后与 `current` 做 `patterns_equivalent`；命中者进入 selected 桶，否则 remaining。
3. 菜单顺序 = selected ++ remaining；`preselected = 0..selected.len()`。
4. `custom_patterns` = `current` 中无法等价匹配任何 catalog pattern 的项（确认时原样并回）。

### 3.4 选择结果

| 结果 | 行为 |
|---|---|
| `Cancelled` | stderr/stdout：「Cancelled, no changes saved」（或中英一致短句）；**不**调用 `save_clean` |
| `Confirmed(idxs)` | 允许空集；`merge` = 选中 catalog patterns（portable `~`）+ `custom_patterns`；`save_clean`；打印 Protected N + Config 路径摘要 |
| 空 catalog | 不应发生；若发生 → 错误退出（对齐 menu「No items provided」） |

不要求 uninstall 式二次 `y/N`（mole whitelist 无此步）。

### 3.5 MenuConfig

```text
sort_mode = Name（无 metadata）
ignore_initial_enter = true
preselected = 构建结果
term_height = crossterm size rows（失败则默认）
```

其余 `VOLE_MENU_*` / `MOLE_MENU_*` 仍经 `MenuState::config_from_env`。

## 4. CLI 与文档

| 面 | 要求 |
|---|---|
| `--whitelist` help | 写明 TTY 分页多选；自动化用 `--whitelist-add/remove/list` |
| README「白名单」提示 | 从「简易交互」改为「TTY 分页多选保护缓存；flag 供脚本」 |
| 非 TTY `--whitelist` | 保持现错误，不进入 ratatui |

## 5. 有意不做的行为对齐

Mole 在配置文件缺失时，`load_whitelist` 把 `DEFAULT_WHITELIST_PATTERNS` 注入 `CURRENT_WHITELIST_PATTERNS`，**clean 扫描也会吃到 defaults**。Vole 今日 `load_clean()` 缺文件返回 `[]`，扫描不套 defaults。

**T2 不改变扫描语义**：仅在 `--whitelist` 管理会话用 defaults 作预选种子。把 defaults 并入 `load_clean()` 会影响全量 clean 跳过面，另开 design。

## 6. 测试与验收

1. **单元（vole-core）**：`patterns_equivalent`（`~` vs `$HOME`）；缺文件种子 defaults；已选项置顶 + preselected；自定义 pattern 在清空预定义后仍保留；确认空选 → 仅 custom 或空表。
2. **CLI**：`--whitelist-list` / `--whitelist-add` / `--whitelist-remove` 非 TTY 回归；`--whitelist` 非 TTY 仍报 InvalidInput；help 含分页/交互或 whitelist-add 指引。
3. **环境**：`VOLE_TEST_NO_AUTH=1`；不挂真授权。
4. **范围守卫**：diff 不含 purge/installer 交互与 status/analyze 视觉改动。

## 7. 风险

- Catalog 体积大（~70 项）：必须分页组件，禁止退回整表 stdin 编号。
- `~` / `$HOME` / 字面 HOME 三者等价比较出错会导致预选漂移 → 单测钉死。
- 自定义 pattern 若误判为预定义会被「保存时丢掉」→ merge 必须显式保留 custom 桶。

## 8. 成功判据

TTY 上 `vole clean --whitelist` 的心智与 mole `mo clean --whitelist` 一致（多选保护缓存、预选置顶、取消不写、自定义保留）；脚本 flag 零破坏；共享 `PaginatedMultiSelect` 再挂一处交互面。
