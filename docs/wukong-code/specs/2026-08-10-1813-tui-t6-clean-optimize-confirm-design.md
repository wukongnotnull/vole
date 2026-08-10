# T6：`clean` / `optimize` TTY 确认双轨

- 日期：2026-08-10 18:13
- 状态：已批准（父 design §3 T6；会话指令：父批准即本窄 design 批准，不另等 OK）
- Mole 钉版：`third_party/mole-1.48.1`（确认文案参照；mole optimize 已去确认，vole 仍按父权威保留 `[y/N]`）
- 父权威：[`2026-08-10-1514-tui-t5-home-menu-roadmap-design.md`](2026-08-10-1514-tui-t5-home-menu-roadmap-design.md) §3 / §4.1 T6
- 模式参照：`uninstall.rs` / `purge.rs` / `installer.rs` 的 `gate_interactive` + 确认后既有 `apply_*`
- 包版本意图：合入时 **MINOR**（相对当前 `2.9.0` → **2.10.0**）
- **不 bump** `schema_version`

## 1. 结论

为 **`vole clean`** 与 **`vole optimize`** 挂载 TTY 确认双轨（**非**分页多选）：

1. 裸 TTY → 现有扫 plan → 人类摘要 → `Proceed? [y/N]`（默认 N）→ 内存 plan → 既有 apply 漏斗
2. `--plan` / `-n` / `--dry-run` / `--apply` / `--json*` / `--plan-out` / 非 TTY → 行为不变
3. `clean` 另：任一 whitelist 系 flag → 不进本确认轨（走既有 whitelist 路径）

有意行为变化：TTY 裸 `vole clean` / `vole optimize` 从「只 plan」变为「可确认后执行」。自动化与删除漏斗零破坏；禁止平行 `rm`；不做 `optimize --whitelist`（T8）；不 bump `schema_version`。

## 2. 进入交互的条件

### 2.1 `clean`

全部满足才进确认轨：

- `stdin` 与 `stdout` 均为 TTY
- 未指定：`--plan` / `--dry-run` / `-n` / `--apply` / `--json` / `--json-stream` / `--plan-out`
- 非 whitelist 系：`--whitelist` / `--whitelist-add` / `--whitelist-remove` / `--whitelist-list`

任一不满足 → **保持现有 plan / apply / whitelist 路径**。

### 2.2 `optimize`

全部满足才进确认轨：

- `stdin` 与 `stdout` 均为 TTY
- 未指定：`--plan` / `--dry-run` / `-n` / `--apply` / `--json` / `--json-stream` / `--plan-out`

`--task` **可与交互并存**（只缩窄扫出的 plan，仍走确认→apply）。

### 2.3 Flag 表

| Flag | 行为 |
|---|---|
| `--permanent` | 交互路径允许单独使用（放宽现有 `requires = apply`）；确认后 apply 用永久删除 |
| `--plan` / `-n` / `--json*` / `--plan-out` / `--apply` | 永不进确认轨 |
| whitelist 系（仅 clean） | 永不进本确认轨；走既有 whitelist 接线 |
| 非 TTY | 永不进确认轨；默认仍产出 plan（现行为） |

## 3. 交互流程（两条命令同构，无分页多选）

1. 复用现有扫描 / 保护 / whitelist（clean）逻辑，构建完整 plan（与 `--plan` 同路径）。
2. 空候选 → 人类提示后退出 0，**不**问 Proceed。
3. 打印既有人类 plan 摘要（复用 `print_human_plan` / hints；走 stdout/stderr 现约定）。
4. 确认提示（stderr）：
   - clean：`Proceed with clean? [y/N] `
   - optimize：`Proceed with optimize? [y/N] `
5. 非 `y`/`Y`（含空 Enter）→ `Aborted.`，退出 0。
6. 确认 → **内存中**调用既有 `apply_proto_plan`（clean）/ `apply_optimize_plan`（optimize）；保护 / 废纸篓 / oplog / TOCTOU 全复用。
7. 人类 apply 摘要走现有 `print_human_report`；退出码与现 apply 路径一致。

**不做：** clean/optimize 分页多选单条（父权威明确留给 mole 亦非此模型 / 本波不做）。

## 4. 实现落点（预期）

```
docs/wukong-code/specs/2026-08-10-1813-tui-t6-clean-optimize-confirm-design.md
crates/vole-cli/src/clean.rs          # explicit_plan + gate_interactive + run_interactive
crates/vole-cli/src/optimize.rs       # 同上
crates/vole-cli/src/main.rs           # help 双轨文案；--permanent 放宽；传 explicit_plan
crates/vole-cli/tests/…               # 门控单测 + CLI 非 TTY / --plan 回归；help 断言
README.md                             # 去掉「Clean/Optimize still plan-only until T6」
docs/releases/v2.10.0.md              # 发版说明
Cargo.toml / Formula/vole.rb          # MINOR 2.10.0（发版 Task）
```

不改：`vole-core` apply 漏斗语义、`schema_version`、`optimize --whitelist`（T8）、analyze 进阶键（T7）。

## 5. 测试与验收

1. 单元：`gate_interactive` 表驱动（TTY / `--plan` / `--json` / `--apply` / whitelist 系）。
2. CLI：非 TTY / `--plan` / `--json` 断言仍只 plan、不进确认（现 fixture 保持绿）。
3. Help / `after_help`：去掉「plan-only until T6」；写明 TTY 裸调用确认后执行。
4. README：替换 T5 诚实句为 T6 已交付语义。
5. `VOLE_TEST_NO_AUTH=1`；不挂真 sudo / Touch ID。
6. 版本：**2.10.0**；不 bump `schema_version`。

## 6. 明确不做（T6）

- `optimize --whitelist`（T8）
- clean 分页多选单条
- analyze Space/⌫/O/P/`/`/T（T7）
- status 动画 cat / prefs（T8）
- bump `schema_version`
- 平行 `rm` 或第二条删除路径

## 7. 风险

| 风险 | 缓解 |
|---|---|
| TTY 默认可删 | `[y/N]` 默认 N；双轨门控单测；help/README 醒目 |
| 与自动化脚本撞车 | 非 TTY / 显式 `--plan`/`--json*` 永不进交互 |
| 空 plan 误确认 | 空候选直接退出，不提示 Proceed |

## 8. 成功判据

T6 后：首页 Enter 进 Clean/Optimize 即真确认流；脚本与 conformance 零破坏；删除只走既有 apply；README/help 不再声称 plan-only until T6。
