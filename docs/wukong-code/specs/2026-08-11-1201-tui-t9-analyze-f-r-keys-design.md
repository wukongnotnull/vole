# T9：analyze `F` Finder reveal + `R` Refresh

- 日期：2026-08-11 12:01
- 状态：已批准（会话：默认 T9 · 「执行默认」）
- Mole 钉版：`third_party/mole-1.48.1`（`cmd/analyze/update.go` `f`/`F`/`r`/`R`；`safeOpen(path, reveal)`）
- 父权威：[`2026-08-10-1514-tui-t5-home-menu-roadmap-design.md`](2026-08-10-1514-tui-t5-home-menu-roadmap-design.md) §4.2 T7 留白；[`2026-08-10-1859-tui-t7-analyze-advanced-keys-design.md`](2026-08-10-1859-tui-t7-analyze-advanced-keys-design.md) §3「不做」表（F/R 留给后续）
- T8 现况：[`docs/releases/v2.12.0.md`](../../releases/v2.12.0.md)（包线 **2.12.0**）
- 包版本意图：合入时 **MINOR**（相对当前 `2.12.0` → **2.13.0**）
- **不 bump** `schema_version`

## 1. 结论

T7 已接线 Space/⌫/O/P/`/`/T；mole footer 仍声明、vole 诚实省略的两键为本波交付：

1. **`F` / `f`：Finder reveal** — `open -R <path>`；多选同 Open（上限 20）
2. **`R` / `r`：Refresh** — 清空多选/过滤焦点，对**当前 stack 顶路径**重新扫描（vole 无 mole 磁盘 cache，语义为「强制重扫」）

硬约束：

- 删除漏斗零变更；F/R **不**触发删除
- footer **只声明已接线**键（接线后出现 `F File` / `R Refresh`）
- `analyze --json` / 非 TTY 不变
- 不 bump `schema_version`

## 2. 键位契约

### 2.1 `F` Finder reveal

| 项 | 约定 |
|---|---|
| 触发 | `f` / `F`（过滤输入态除外，字面字符进 filter） |
| 目标 | `paths_for_action`（多选非空用多选，否则当前行）；Large files / 目录列表同形 |
| argv | `/usr/bin/open` `-R` `<path>`（对齐 mole `safeOpen(path, true)`） |
| 批量上限 | `MAX_BATCH_OPEN`（20）；超出 → 状态行提示，不 spawn |
| overview / scanning | **允许**（对齐 mole：open/reveal 在扫描中可用） |
| 空列表 | noop |
| 失败 | 状态行 `Reveal failed: …`；不 panic |

### 2.2 `R` Refresh

| 项 | 约定 |
|---|---|
| 触发 | `r` / `R`（过滤输入态除外） |
| 行为 | 发出 `AnalyzeEffect::Refresh`；`main`：`multi`/`filter`/`Top` 状态清空（或整表 `AnalyzeState::default()`）、`scanning = true`、丢弃进行中的 `scan_rx`、**stack 不变** |
| 状态行 | `Refreshing...`（进入扫描；扫描结束后既有逻辑清状态） |
| overview | 同目录：重扫当前 overview 根（vole 无独立 overview cache 失效 API） |
| Top 全屏 | 退出 Top 语义随 state reset；重扫后回目录列表 |
| delete 确认中 | 忽略 R（或先当 cancel——**选忽略**，与未映射键一致） |
| JSON / 非 TTY | 无此键 |

说明：mole 的 `invalidateCacheTree` / overview cache 在 vole 侧无对应物；本波 **不**引入磁盘 cache，仅重跑 `analyze_directory`。

### 2.3 Footer（接线后）

| 模式 | 须含 |
|---|---|
| 目录 | `F File`、`R Refresh`（与既有 ↑↓ Space Enter / Filter O P ⌫ T Esc Q 并列） |
| Top | `F File`、`R Refresh`（无 Enter/T 时保持 T7 Top 形态，仅追加 F/R） |
| filter / delete confirm | 不变（无 F/R 声明） |

既有单测 `assert!(!f.contains("F File"))` **改为**断言**出现**；并保留「未做键」（如 `S Live` / `←→` 别名）不出现。

## 3. 实现落点

| 落点 | 动作 |
|---|---|
| `tui/analyze_actions.rs` | `reveal_argv(path) -> Vec<String>`；单测形状含 `-R` |
| `tui/analyze_state.rs` | `AnalyzeKey::{Reveal,Refresh}`；`map_analyze_key`；`begin_reveal`；`Refresh` → `AnalyzeEffect::Refresh`；清空多选在 effect 前或由 main 重置 |
| `tui/widgets.rs` | `analyze_footer` 追加 `F File` / `R Refresh` |
| `main.rs` `cmd_analyze_tui` | `Reveal` → `spawn_detached(reveal_argv)`；`Refresh` → 重扫 |
| README / `docs/releases/v2.13.0.md` | 勾 T9；成熟度 **2.13.0** |
| `Cargo.toml` / Formula | MINOR 2.13.0（Formula sha 发版后 pin） |

## 4. 不做（本波）

| 项 | 说明 |
|---|---|
| `←`/`→`/`h`/`b` 导航别名 | 可选增强，另开 |
| 扫描中 `S` live sort | 可选增强，另开 |
| mole 磁盘 / overview cache 复刻 | YAGNI；Refresh=重扫即可 |
| bash 像素级重绘 | 仍用 ratatui |
| bump `schema_version` | 无协议变化 |

## 5. 验收

1. 单元：`reveal_argv`；`map_analyze_key` 含 f/F/r/R；`begin_reveal` 批量上限；footer 含 F/R、不含未做键
2. `VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole`
3. `./scripts/check-command-surface.sh --enforce`；`cargo fmt --all -- --check`
4. TTY 手工：`vole analyze ~` → `F` 开 Finder；改目录内容后 `R` 重扫列表更新
5. `analyze --json` 行为不变

## 6. 成功判据

T9 后：TTY `vole analyze` 可用 `F`/`R`；footer 诚实声明；删除/JSON 零破坏；包版本 **2.13.0**。
