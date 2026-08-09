# T4：TUI 交互 mole 级复刻收口（对照表 + 文档 + 2.8.0）

- 日期：2026-08-09 23:03
- 状态：已批准（父 design §5 T4；会话默认批准；与 v2.8.0 同 PR 发版）
- 父权威：[`2026-08-09-2136-tui-interactive-mole-parity-design.md`](2026-08-09-2136-tui-interactive-mole-parity-design.md) §5 T4
- Mole 钉版：`third_party/mole-1.48.1`
- 前置合入：T0 #117 · T1 #119 · T2 #120 · T3 #121（均已在 `main`）
- 包版本：workspace **2.7.0 → 2.8.0**（MINOR；TTY 交互面相对 2.7.0 扩至 purge/installer/whitelist + status/analyze 视觉）
- **不 bump** `schema_version`

## 1. 结论

T0–T3 功能代码已在 `main`；本波次只做 **收口**：

1. 对照表勾选 mole 交互面 vs vole 现状（诚实标出有意不做）
2. README 写清「TUI 交互对齐」与双轨用法
3. 清除仍声称「无 TTY 多选」的 coverage / findings 长尾措辞
4. 发版 **2.8.0**（release notes + Cargo.toml + Formula URL 占位 sha）

**不实现** T3 已记入长尾的能力（动画 cat、analyze 删除键等）。

## 2. 对照表（mole 交互面 → vole）

| Mole 交互面 | vole 命令 / 路径 | 状态 | 波次 |
|---|---|---|---|
| `menu_paginated` 共享分页多选 | `vole-cli::tui::{MenuState,run_paginated_select}` | ✅ | T0 |
| `uninstall` TTY 多选→确认→删 | 裸 `vole uninstall`（双轨） | ✅ | T0 / 2.7.0 |
| `purge` / project 选择器 | 裸 `vole purge`（双轨） | ✅ | T1 |
| `installer` 选择器 | 裸 `vole installer`（双轨） | ✅ | T1 |
| `manage_whitelist` 分页多选 | `vole clean --whitelist`（TTY） | ✅ | T2 |
| `status` 视觉区块/双列 | `vole status` TUI | ✅ 视觉同构（已接线键） | T3 |
| `analyze` 视觉行/viewport | `vole analyze` TUI | ✅ 视觉同构（已接线键） | T3 |
| 自动化 plan/json | `--plan` / `--apply` / `--json*` / 非 TTY | ✅ 行为不变 | 全程双轨 |

### 2.1 双轨门控（统一语义）

对 `uninstall` / `purge` / `installer`：

- **进交互**：stdin+stdout 均为 TTY，且未指定 `--plan` / `--dry-run` / `-n` / `--apply` / `--json` / `--json-stream` / `--plan-out`（uninstall 另：无 `target` 位置参数）
- **否则**：既有 plan/apply 自动化路径，零破坏
- **确认后删除**：一律经既有 `apply_*_plan`；禁止平行 `rm`

对 `clean --whitelist`：

- TTY + `--whitelist` → 分页多选保存
- `--whitelist-add` / `--whitelist-remove` / `--whitelist-list` → 自动化不变
- 非 TTY 仅 `--whitelist` → 错误提示用 flag

## 3. 有意不做（诚实长尾，本波次不实现）

来自 T3 §2.2 与父目标 D 的边界：

| 项 | 说明 |
|---|---|
| analyze Space 多选 / ⌫ 删除 / O Open / P Preview / `/` Filter / T Top | 未接线；footer **不虚标** |
| status 动画 mole cat、`k` 隐藏 cat、`c` 循环 core 数 prefs | 未接线；避免半残键位 |
| bash `menu_paginated.sh` 增量光标重绘像素级复刻 | ratatui 立即模式替代 |
| optimize `--whitelist`（mole 独立清单） | vole 尚无该 flag；另开 |
| 选择会话协议进 `vole-core` / 桌面 | UI 留在 `vole-cli` |
| bump `schema_version` | 无协议字段变化 |

purge/installer plan 仍可诚实记录**非 UI**长尾（完整 activity 分类、cloud 确认、fd 扫描分支等）——与「TTY 多选已落地」无关。

## 4. 文档与代码措辞清理

| 落点 | 动作 |
|---|---|
| `README.md` | 增补「TUI 交互对齐」；版本示例 → 2.8.0；成熟度行反映 T0–T3 |
| `docs/releases/v2.8.0.md` | 新建：汇总 T0–T3 + 本收口 |
| `docs/findings/2026-08-v2-m4-cli-complete-spike.md` | purge/installer 长尾去掉「TTY 多选 / TTY 分页全量」 |
| `docs/findings/2026-07-v2-m0-uninstall-spike.md` | 保持 T0 已落地句；可点明 T1–T4 收口版本 |
| `crates/.../coverage_note` 字符串 | 确认不再声称缺 TTY 多选（installer 已只留 fd 分支；purge 已无 UI 句） |
| 历史 release / 旧 plan 叙事 | **不改写**整段历史；仅清仍误导「当前产品」的长尾 |

## 5. 发版清单（2.8.0）

1. `Cargo.toml` workspace `version = "2.8.0"` + `Cargo.lock` 同步
2. `Formula/vole.rb`：version/URL → 2.8.0；sha256 占位 `0…0`，资产就绪后 `bash scripts/update-homebrew-formula.sh 2.8.0`
3. README 预编译示例 → `v2.8.0`
4. 合入后：annotated tag `v2.8.0` → Release workflow → Formula pin PR

## 6. 成功判据

T4 后：文档对照表勾满目标 D 的**已交付**交互面；README 读者能一眼看懂双轨；coverage/findings 不再把「TTY 多选」写成未做；`2.8.0` 可 tag 发版。有意长尾（动画/analyze 删除等）写在对照表「不做」栏，不偷偷实现。
