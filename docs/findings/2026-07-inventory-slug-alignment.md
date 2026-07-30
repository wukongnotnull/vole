# Inventory slug / path 对齐

**日期**：2026-07-30  
**状态**：完成（脚本侧；未改 vole rule id）

## 问题

`scripts/inventory-mole-rules.py` 仅用 `proposed_id`（由 mole label slugify）与 vole `id` / `label` 比对。Arc/Chrome 等路径已在 vole 用 `arc-root-*` / `arc-profile-*` 等 id 覆盖时，仍报 `unported_all`（约 22 条伪差集）。

## 做法

1. 从 `data/rules/*.toml` 收集全部 `paths`，规范化后与 mole `path_expr` 精确匹配 → `match_reason=path`
2. 显式别名：`homebrew-cache` → `homebrew-downloads-cache` → `match_reason=id_alias`
3. JSON/CSV 增加 `match_reason`；summary 增加 `ported_by_path` / `ported_by_id_alias`
4. `--self-test` 覆盖 normalize / alias / path 匹配

**不改** 现有规则 id（避免破坏 plan `rule_id`）。

## 结果（本机）

| 指标 | 之前 | 之后 |
|---|---|---|
| total | 513 | 513 |
| ported | 446 | **470** |
| unported_all | 22 | **2** |
| ported_by_path | — | 23 |
| ported_by_id_alias | — | 1 |

剩余 `unported_all`（刻意）：

- `claude-pending-uploads` — Claude bundle 保护
- `rosetta-2-cache` — `/Library/...`，需 sudo

## 验证

```bash
python3 scripts/inventory-mole-rules.py --self-test
python3 scripts/inventory-mole-rules.py
```
