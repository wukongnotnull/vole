# optimize `shared_file_list_repair` 设计（闸控轨 G4）

- 日期：2026-08-08 19:02
- 状态：已批准（用户明确「批准执行轨 G4」；本会话 design 落盘后直接实现）
- 依据：[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) §3.3 P4；计划 [`../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md) Task G4；Mole `opt_shared_file_list_repair`（`tasks.sh` ≈1118）；G1–G3 范例
- 包版本意图：**1.45.0**（MINOR；相对 `1.44.0`）
- **不 bump** `schema_version`

## 1. 结论

将 catalog **`shared_file_list_repair.in_m3: false` → `true`**，主路径 21 → **22**（仅剩 `disk_verify` 长尾）：

| 阶段 | 行为 |
|---|---|
| **plan** | 扫描 `~/Library/Application Support/com.apple.sharedfilelist` 下 `*.sfl2`/`*.sfl3`；`plutil -lint` 失败 → 每文件 1 条 action 候选 |
| **apply** | 再 lint；仍失败则 `remove_file`；已健康 → Ok noop |
| **失败** | lint 不可用 / `VOLE_TEST_NO_AUTH` Live → 空 plan；删失败 → Failed |

**禁止**：`sfltool`（含 `dumpbtm`，避免非特权 GUI）；碰 `*ApplicationRecentDocuments*`；本轨不做 G5/D1。

## 2. 数据损坏模型 / 备份 / skip

| 点 | 决策 |
|---|---|
| 损坏定义 | `plutil -lint` 非 0（与 Mole 一致） |
| 跳过 | 路径含 `ApplicationRecentDocuments`（用户最近文档列表；Mole 同） |
| 备份 | **不**另做拷贝；corrupt 文件本已不可用；删除后系统可重建空列表 |
| skip | 用户可不选 plan 条目；apply 幂等 |

## 3. 与 `recent-items-list` clean 规则边界

| | `clean` · `recent-items-list` | `optimize` · `shared_file_list_repair` |
|---|---|---|
| 目标 | 固定 RecentApplications/Documents/Servers/Hosts `.sfl`/`.sfl2` | **任意** corrupt `.sfl2`/`.sfl3`（除 ApplicationRecentDocuments） |
| 条件 | 规则命中即列 | 仅 `plutil -lint` 失败 |
| 目的 | 清空最近项 | 修复损坏 Finder favorites 等共享列表 |
| 重叠 | Recent* 健康文件：clean 可删、optimize **不**列 | Recent* 若损坏：optimize 可列（非 ApplicationRecentDocuments 路径） |

## 4. 采纳路径

| 点 | 决策 |
|---|---|
| catalog | `in_m3: true`；主路径 **22**；`disk_verify` 仍 false |
| 模块 | `optimize/tasks/shared_file_list.rs`：`SharedFileListDeps` + Live + Fake |
| Live list | `find` 语义：walk `sharedfilelist`，扩展名 sfl2/sfl3，跳过 ApplicationRecentDocuments |
| lint | `plutil -lint <path>` |
| 候选 | `path` = 真实文件；label `Corrupted shared file list`；`task_id=shared_file_list_repair` |
| apply | 再 lint → 失败则 `fs::remove_file`；**零** `sfltool` |
| `VOLE_TEST_NO_AUTH` | Live 空 plan / apply Skipped |
| 版本 | **1.45.0** + release / README / Formula / coverage |

## 5. Mole 对照

| Mole | Vole |
|---|---|
| find `*.sfl2|*.sfl3`，跳过 ApplicationRecentDocuments | 同 |
| `plutil -lint` 失败 → `safe_remove` | apply `remove_file` |
| dry-run 计数 | plan 候选数 |
| 无 sfltool | **禁止** sfltool |

## 6. 测试策略

- Catalog：`main.len()==22`，仅 `disk_verify` 不在主路径
- plan：Fake 注入 corrupt/healthy/recent-docs；断言候选
- apply：corrupt → 删除；healthy → noop；TestMode → Skipped
- 禁止：单测调用 `sfltool`

## 7. 非目标

- G5 `disk_verify`；改 clean `recent-items-list`；sfltool / dumpbtm；SMAppService
