# clean plan 广域目录体积对齐 Mole

- 日期：2026-08-09 13:22
- 状态：已批准（plan「广域清理体积修复」）
- 依据：Mole `user.sh` `safe_clean ~/Library/Caches/*` / `~/Library/Logs/*`；[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) §1.2 A；实测 Vole 已启用 `user-app-cache` / `user-app-logs` 但 plan 合计约 2.3G
- 包版本意图：**2.6.0**（MINOR；相对 `2.5.0`；用户可见 plan 体积修正）
- **不 bump** `schema_version`

## 1. 结论

广域规则**已在** `data/rules/user-devtools.toml`（`user-app-cache` / `user-app-logs`）。缺口是 plan 对目录用 `metadata().len()`（inode），未递归 `du`，导致桌面/CLI 合计严重偏低。

本变更：

| 点 | 决策 |
|---|---|
| 测体积 | 文件/symlink → metadata；目录 → `measure_path_size_bytes`（`du -skPx`，既有 30s 超时） |
| 重叠 | 目录 entry 展示体积 = `max(0, raw_du - sum(raw 直接 plan 子孙))`，合计不双计 |
| 并行 | 目录 `du` 有界并行（固定小并发） |
| 保护 | 不放宽；`ms-playwright` 等仍跳过 |
| 不做 | 不整树 `XCTestDevices` / `CoreSimulator`；不改 apply 删除语义 |

## 2. 验收

1. 单测：目录 entry 体积 ≥ 内容字节；父+子同 plan 时父体积不含子已计部分，合计 ≈ 父树 union
2. 本机 `vole clean --plan --json` 合计从约 2.3G 升到 Caches 广域真实量级（十余 GB 级）
3. `coverage_note` / 路线图：Caches/Logs 广域已落地；不再把该项写成「继续用 Mole」

## 3. 非目标

- 盲扩 bash custom 循环
- Developer 大户整树清理
- 放宽 path protection
