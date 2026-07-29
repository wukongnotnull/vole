# Vole NDJSON 协议（v1 定型，Phase 4 末冻结）

`schema_version` 当前为 **1**。破坏性变更递增版本号。

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
每条 entry：`id`, `path`, `label`, `size`, `rule_id`, `skip_reason`, `dev`, `ino`, `mtime`（Unix 秒）。

## Report

`succeeded`, `skipped`, `failed`, `skipped_by_reason[]`, `trashed_bytes`, `deleted_bytes`。

废纸篓语义见设计文档 5.7——不得用单一「freed」字段。

## Status 流（Phase 2）

`vole status --json-stream` 每行一个 JSON 对象，字段集与 `mo status --watch` 对齐（`StatusSnapshot`）。
取消时进程退出，不保证额外 `aborted` 行（Phase 2–3 可扩展）。

## 冻结时点

Phase 4 结束前可破坏性修改；之后只能追加字段/枚举变体。
