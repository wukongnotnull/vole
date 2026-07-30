# Vole NDJSON 协议

**Status: FROZEN**（2026-07-29，Phase 4 结束 / Phase 5 收口）

`schema_version` 当前为 **1**。破坏性变更必须递增版本号，并单独立项；冻结后不得静默新增或重命名 NDJSON `StreamEvent` 字段。允许的演进：仅追加可选字段，或追加 `SkipReason` 枚举变体（不得改名既有字符串）。

权威说明见设计文档 [5.6](wukong-code/specs/2026-07-29-rust-rewrite-design.md#56-前端边界协议与两阶段模型)。

## 传输

- stdout：NDJSON，一行一个事件；**仅协议**。
- stderr：人类日志与诊断。

## 事件类型

| type | 字段 | 说明 |
|---|---|---|
| `progress` | `scanned`, `current` | 扫描进度 |
| `candidate` | `id`, `path`, `label`, `size`, `rule_id` | plan 阶段候选 |
| `skipped` | `rule_id`, `reason` | 规则级跳过 |
| `done` | `report` | 结束汇总 |
| `aborted` | `reason` | 取消或异常中止 |

`reason` 取值见 `vole_proto::SkipReason`（snake_case）。

## Plan 文件

JSON 对象：`schema_version`, `created_at`, `ttl_secs`（默认 900）, `entries[]`。
可选 `coverage_note`（字符串）：plan 阶段规则覆盖说明（未移植 mole 类别提示）。
每条 entry：`id`, `path`, `label`, `size`, `rule_id`, `skip_reason`, `dev`, `ino`, `mtime`（Unix 秒）。

`vole uninstall` 复用同一 Plan / Report / 事件形状；`rule_id` 使用前缀 `uninstall:`（应用本体）或 `uninstall:leftover:`（用户域残留）。勿用 `vole clean --apply` 执行 uninstall plan（apply 路径独立）。

`vole optimize` 同样复用 Plan / Report；`rule_id` 约定：
- `optimize:delete:<task_id>` — 删除类（默认废纸篓）
- `optimize:action:<task_id>` — 副作用类（VACUUM / defaults / Dock / LaunchServices 等）
勿用 clean/uninstall apply 执行 optimize plan。

## Report

`succeeded`, `skipped`, `failed`, `skipped_by_reason[]`, `trashed_bytes`, `deleted_bytes`。
可选 `coverage_note`（字符串）：plan 阶段 `--json-stream` 的 `done.report` 可携带与 Plan 相同的覆盖说明；apply 阶段通常省略。

废纸篓语义见设计文档 5.7——不得用单一「freed」字段。

## Status 流（Phase 2）

`vole status --json-stream` 每行一个 JSON 对象，字段集与 `mo status --watch` 对齐（`StatusSnapshot`）。
取消时进程退出，不保证额外 `aborted` 行（Phase 2–3 可扩展）。

## 冻结时点

**已冻结。** Phase 4 结束后，NDJSON / Plan / Report / SkipReason 字符串进入兼容性承诺。变更流程：新计划 + `schema_version` bump（若破坏性）。

---

## 附录：History JSON（非 NDJSON）

`vole history --json` 输出**单个** JSON 对象（非事件流），对齐 mole `history --json`。它**不属于** `StreamEvent`，不走 `schema_version`，但字段名与 mole 契约对齐，变更需单独说明。

| 字段 | 说明 |
|---|---|
| `logs.operations` | operations.log 路径 |
| `logs.deletions` | deletions.log 路径 |
| `limit` | 实际使用的 limit（夹紧后 1..=200，默认 20） |
| `sessions[]` | 最新在前；见下表 |
| `deletions[]` | 最新在前；见下表 |

### `sessions[]` 元素

`command`, `started_at`, `ended_at`, `items`, `size`, `operation_count`, `actions.{removed,trashed,skipped,failed,rebuilt,other}`

### `deletions[]` 元素

`timestamp`, `mode`, `status`, `size_kb`（number 或 `null`）, `path`
