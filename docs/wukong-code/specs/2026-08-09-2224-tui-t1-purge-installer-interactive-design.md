# TUI T1：`purge` / `installer` 交互多选

- 日期：2026-08-09 22:24
- 状态：已批准（父 design §5 T1；会话默认批准；镜像 uninstall 双轨）
- Mole 钉版：`third_party/mole-1.48.1`
- 父权威：[`2026-08-09-2136-tui-interactive-mole-parity-design.md`](2026-08-09-2136-tui-interactive-mole-parity-design.md) §5 T1
- 复用：`PaginatedMultiSelect` / `MenuState` / `run_paginated_select`（T0 / PR #117）
- 对照：`lib/clean/project.sh`（`select_purge_categories`）、`bin/installer.sh`（`select_installers`）
- **不 bump** workspace `Cargo.toml` 版本（并行轨由 coordinator 发版）；**不 bump** `schema_version`

## 1. 结论

在 T0 共享组件上，为 **`vole purge`** 与 **`vole installer`** 挂载与 uninstall 同构的双轨交互：

1. TTY 裸命令 → 分页多选 → 确认 → 既有 `apply_*_plan` 漏斗
2. `--plan` / `--json*` / 非 TTY / 显式自动化 flag → 行为不变

编排 / 删除 / 保护仍在 `vole-core`；UI 只在 `vole-cli`。禁止第二条删除路径。不触碰 whitelist（T2）与 status/analyze 视觉同构（T3）。

## 2. 进入交互的条件（两条命令各自独立，语义同 uninstall）

全部满足才进交互：

- `stdin` 与 `stdout` 均为 TTY
- 未指定：`--plan` / `--dry-run` / `-n` / `--apply` / `--json` / `--json-stream` / `--plan-out`

任一不满足 → **保持现有 plan/apply 自动化路径**。

| Flag | 行为 |
|---|---|
| `--permanent` | 交互路径允许单独使用（放宽现有 `requires = apply`）；确认后 apply 用永久删除 |
| `--plan` / `-n` / `--json*` / `--plan-out` / `--apply` | 永不进交互 |
| `--include-empty`（仅 purge） | 可与交互并存；影响扫描候选集合 |
| 非 TTY | 永不进交互；默认仍产出 plan（现行为） |

## 3. 候选粒度（对齐 mole，禁止路径噪音）

| 命令 | 多选条目 | 禁止 |
|---|---|---|
| `purge` | 每个 **purge plan entry**（一条构建物目录，如某项目下的 `node_modules`） | 不把 entry 再拆成内部文件；不做 residual path spam |
| `installer` | 每个 **installer plan entry**（一个安装包文件） | 不聚合为扩展名桶；不扫出符号链接候选 |

即：交互列表 = 现有 `build_*_plan` 产出的 `entries`，粒度已是 mole 选择器级别。

## 4. 交互流程（两条命令同构）

1. 复用现有扫描 / 保护逻辑，构建完整 `ProtoPlan`（与 `--plan` 同路径）。
2. 空候选 → 人类提示后退出 0，不进选择器。
3. `drain` → `run_paginated_select`（`ignore_initial_enter = true`）。
4. 取消（Q / Esc）→ 退出 0，不删。
5. 空选 → 提示后重新进入选择（对齐 mole / uninstall），不误删。
6. 有选中 → 离开 alt-screen → 打印摘要 → 确认提示：
   - purge：`Proceed with purge? [y/N]`
   - installer：`Proceed with installer cleanup? [y/N]`
7. 非 `y`/`Y` → `Aborted.`，退出 0。
8. 确认 → **内存中**将 plan 过滤为选中下标对应 entries → 调用既有 `apply_purge_plan` / `apply_installer_plan`（保护 / 废纸篓 / oplog / TOCTOU 全复用）。
9. 人类摘要走 stderr；installer 若 `report.failed > 0` 仍非零退出（现语义）。

### 4.1 预选（对齐 mole 精神，受 vole 扫描简化约束）

| 命令 | 预选 |
|---|---|
| `purge` | **全部预选**（vole 已按 `MIN_AGE_DAYS` 过滤「近期」条目，等价 mole「非 recent 默认勾选」） |
| `installer` | **无预选**（对齐 mole `select_installers` 默认全未选） |

### 4.2 MenuItem 映射

- `label`：优先 `entry.label`；必要时附加短路径尾（可读即可）
- `filter_name`：路径或 label
- `size_kb`：`entry.size / 1024`（已知时）
- `epoch`：`entry.mtime` → unix 秒（供 date 排序）

标题建议：`Select Artifacts to Purge` / `Select Installers to Remove`。

## 5. 实现落点（预期）

```
docs/wukong-code/specs/2026-08-09-2224-tui-t1-purge-installer-interactive-design.md
crates/vole-cli/src/purge.rs          # gate + run_interactive
crates/vole-cli/src/installer.rs      # gate + run_interactive
crates/vole-cli/src/main.rs           # help 文案；--permanent 放宽
crates/vole-cli/tests/purge_cli.rs    # 门控 + help 双轨
crates/vole-cli/tests/installer_cli.rs
README.md                             # 双轨说明（最小改动）
```

不改：`vole-core` apply 漏斗、Formula、workspace 版本、whitelist、status/analyze。

## 6. 测试与验收

1. 单元：`gate_interactive` 表驱动（TTY / `--plan` / `--json` / `--apply`）。
2. CLI：非 TTY / `--plan` / `--json` 断言不进交互（现 fixture 路径保持绿）。
3. Help：`--help` 含 interactive / TTY / 多选语义。
4. `VOLE_TEST_NO_AUTH=1`；不挂真 sudo / Touch ID。
5. README：`purge` / `installer` 双轨一行级说明。

## 7. 明确不做（T1）

- whitelist UI（T2）
- status / analyze 视觉同构（T3）
- mole purge 的完整 recent/cloud 分类 UI 与 category 聚合（vole 已用年龄过滤简化）
- bash 增量光标重绘
- bump `Cargo.toml` / Formula / tag / release
- bump `schema_version`

## 8. 风险

- TTY 默认从「只 plan」变为「可删」→ help / 确认文案必须醒目（同 uninstall）。
- 候选必须是 **plan entry 级**，禁止把目录内部文件展开进多选。
- 不引入平行 `rm`；确认后只走既有 apply。

## 9. 成功判据

T1 后：mole 用户在 TTY 上对 `vole purge` / `vole installer` 的心智模型与 uninstall 一致；脚本与 conformance 零破坏；T2/T3 可并行推进。
