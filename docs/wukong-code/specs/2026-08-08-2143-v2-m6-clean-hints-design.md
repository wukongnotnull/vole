# Vole M6：`clean` 内 hints 只读提示设计

- 日期：2026-08-08 21:43
- 状态：已批准（会话默认批准；产品 v2 续篇权威规格下的专用 design）
- 包版本：本里程碑发版 **`2.1.0`**（规格 §5）
- Mole 钉版：`third_party/mole-1.48.1`
- 依据：[`2026-08-08-2030-v2-cli-complete-design.md`](2026-08-08-2030-v2-cli-complete-design.md) §3.4 / §5 / §6.3；M4 findings [`../../findings/2026-08-v2-m4-cli-complete-spike.md`](../../findings/2026-08-v2-m4-cli-complete-spike.md) §3.2
- 对照：`lib/clean/hints.sh`、`lib/clean/purge_shared.sh`、`tests/clean_hints.bats`；已落地 `purge` / `purge_paths`

## 1. 结论

交付 **`hints` 作为 `vole clean` 内只读提示模块**（非顶层命令）：在 `clean --plan`（及默认 plan 路径）结束后，追加 Mole 主路径子集的非破坏提示；超时/浅扫预算内未完成则跳过提示，**永不阻塞** clean 的候选产出与 apply。

**禁止**注册 `vole hints` / `Command::Hints`。

编排住在 `vole-core::ops`；`vole-cli` 仅渲染。不引入第二套删除路径；不改快照 / `/Library/Updates` / Install Data 禁区。

## 2. 主路径子集（本里程碑必做）

| 探针 | Mole 对照 | 行为 |
|---|---|---|
| **项目构建物快捷提示** | `probe_project_artifact_hints` / `show_project_artifact_hint_notice` | 浅扫 `purge_paths`（或默认搜索根）下一层/两层项目目录，匹配 quick-hint targets；汇总条数 + 最多 3 个路径的 `du` 抽样（每路径 ≤0.8s）；文案引导 `vole purge`（空产物抽样为 0 时提示 `--include-empty`） |
| **System Data 大路径线索** | `show_system_data_hint_notice` | 对钉死的常见大路径（DerivedData / Archives / iPhone backups / Simulator / Docker / Mail / OrbStack）做超时 `du`；≥2GiB 才展示，最多 3 条 |

### 2.1 Quick purge 对齐

- 搜索根：`~/.config/vole/purge_paths`（一行一路径；测试可 `VOLE_PURGE_PATHS` / Options 注入）→ 否则复用 purge 默认 `DEFAULT_SEARCH_REL` 中**存在**的目录（与 M5 一致；不整盘 `$HOME` 深扫）
- Targets：`PURGE_TARGETS` 减去 Mole `MOLE_PURGE_QUICK_HINT_EXCLUDED_TARGETS`（`bin`、`vendor`）
- 根本身若是项目根（indicators 子集，与 purge 一致）：直接检查 `$root/<target>`
- 否则：根下一层子目录（跳过点名）查 targets；再对每个项目目录浅列一层嵌套（跳过点名与 `node_modules|target|build|dist|DerivedData|Pods`），嵌套内再查 targets
- 预算：墙钟默认 **15s**（env `VOLE_TIMEOUT_HINT_SCAN_SEC`）；子目录列举超时默认 **1s**/次；超预算 → `scan_skipped` / truncated，有命中则标 partial，无命中则静默或单行「scan skipped · vole purge」

### 2.2 长尾（本里程碑不做）

- `show_user_launch_agent_hint_notice`（LaunchAgent / MachServices / bundle 归属）
- `show_orphan_dotdir_hint_notice`（GUI app / claude plugin 点目录）
- Mole spinner / TTY 彩色图标全量复刻（人读用简洁 ASCII 即可）

诚实记入 `coverage_note` 或 release notes「hints 长尾」。

## 3. 挂载点与输出

| 路径 | 行为 |
|---|---|
| `vole clean` / `--plan` / `--dry-run` 人读 | plan 列表与既有 coverage/notices 之后，打印 hints 行（无命中且未 skip → 不打印） |
| `--json` / `--plan-out` | plan JSON **追加可选** `hints: [{kind, summary, detail?}]`（序列化层注入；`HintNotice` 在 `vole-proto`；空则省略）；**不 bump** `schema_version` |
| `--json-stream` | 不另发明细事件；Done 后的 plan JSON（若写出）可含 hints；stream 主路径可省略 hints 以避免拖慢 |
| `--apply` | **不**跑 hints（只读提示挂在发现/plan 侧） |

`kind` 枚举本里程碑：`project_artifacts` | `system_data`。

## 4. 架构约束

1. **只读**：hints 模块禁止调用 `mole_delete*` / apply / 任何删除漏斗。
2. **失败偏跳过**：超时、权限、IO 错误 → 跳过该探针/该路径，不失败整个 clean。
3. **复用 purge 配置与 targets 表**：不复制第二套 purge 规则；可从 `purge_plan` 抽共享常量/读配置 helper（若抽共享，保持 purge 行为不变）。
4. **禁区不变**：不扫描/不暗示删除本地快照、`/Library/Updates`、`/macOS Install Data`。
5. **无顶层命令**：`scripts/check-command-surface.sh` 已有 `UNEXPECTED: top-level Hints` 负向断言；加 CLI 测 `vole hints` → 非 0 / clap 未知子命令。

## 5. 版本与文档

功能就绪后 bump **`2.1.0`**：`Cargo.toml` workspace、`Cargo.lock`、`Formula/vole.rb`、`README.md` 成熟度行、`docs/releases/v2.1.0.md`。  
权威规格 [`2030`](2026-08-08-2030-v2-cli-complete-design.md) §9 仅回链一句「M6 进行中/已交付」，不大改无关文档。

## 6. 测试与验收

- 单元（`vole-core`）：temp HOME + `purge_paths` 含 `proj/node_modules`（及被排除的 `vendor`/`bin`）→ 仅计 node_modules；预算 0 → `scan_skipped`；system_data 对小路径静默、对注入大 size stub 可展示
- CLI：`clean --plan` 人读含 `Build artifacts` / `vole purge`；`--json` 含 `hints`；`vole hints` 失败；无 `Command::Hints`
- 回归：既有 clean / purge 测仍绿
- 不得空 bump：先有上述行为再改版本号

## 7. 文件落点（预期）

```
crates/vole-core/src/ops/clean_hints.rs   # 探针 + CleanHints 汇总
crates/vole-core/src/ops/mod.rs
crates/vole-core/src/ops/purge_plan.rs    # 可选：导出共享读 purge_paths / indicators
crates/vole-proto/src/plan.rs             # 可选 hints 字段
crates/vole-cli/src/clean.rs              # plan 后收集并渲染
crates/vole-cli/tests/clean_hints_cli.rs  # CLI 测 + 禁止顶层 hints
docs/releases/v2.1.0.md
Cargo.toml / Formula/vole.rb / README.md
```

## 8. 文档阶段验收

- [x] hints = clean 内只读；禁止顶层子命令写死  
- [x] 主路径（project artifacts + system data）与长尾写死  
- [x] 超时/浅扫预算与降级语义写死  
- [x] 与 purge_paths / PURGE_TARGETS 对齐写死  
- [x] 不引入第二套删除；禁区保留  
- [x] `2.1.0` 绑定本里程碑且禁止空 bump  
