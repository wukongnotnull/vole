# Vole 产品 v2 目标与整体规划

- 日期：2026-07-30
- 状态：已批准
- 依据：`2026-07-30-v1-closeout-design.md`；`2026-07-30-semver-policy-design.md`；`2026-07-29-rust-rewrite-design.md` §1 / §4.2；用户批准方案 A（顺序里程碑）

## 1. 结论

**产品 v2 的北极星是 CLI 能力补齐**：在 v1 已冻结的 `status` / `analyze` / `clean` / `history` 底座上，按顺序交付高对齐的 `uninstall` 与 `optimize`，缩短与 Mole「日常全家桶」的差距。

SwiftUI 桌面 app **不在本代际并行主路径**：等产品 v2 CLI 成熟后再开另仓。

包版本走严格 SemVer **`1.x` MINOR**（`1.1.0` → `1.2.0`）。产品话术「v2」表示能力代际，**不等于**强制发 `2.0.0`；仅破坏用户已依赖的 CLI / 协议 / 默认行为时才升 MAJOR。

## 2. 已锁定决策

| 项 | 结论 |
|---|---|
| 北极星 | CLI 补齐 Mole 差距；SwiftUI 延后 |
| 推进方式 | 方案 A：顺序里程碑 |
| 命令顺序 | `uninstall` → `optimize` |
| 深度 | 高对齐：主路径 + JSON 口径对齐 Mole；长尾诚实跳过并提示继续用 Mole |
| 范围 | 窄 v2（见 §4） |
| 交互 | 对齐 clean：`--plan` / `--apply` + 默认废纸篓（可 `--permanent`）+ `--json` / `--json-stream` |
| 版本 | 产品「v2」= 代际；包 `1.1`（uninstall）→ `1.2`（optimize） |

## 3. 成功标准（产品 v2 CLI 成熟）

1. `vole uninstall` 高对齐可用：plan→apply、保护层、oplog、Mole 兼容 JSON 子集
2. `vole optimize` 同档可用
3. 交互菜单与 shell 补全覆盖新命令
4. README 明确「产品 v2 CLI 能力已达」；SwiftUI 标为下一轨
5. 包版本至少到承载 optimize 的 **`1.2.x`**（具体 PATCH 随缺陷修复）

## 4. 范围

### 4.1 必做

- **`uninstall`**：枚举已装 app → 残留预览 plan → apply；主路径对齐 Mole；边缘 case 可 skip + coverage 提示
- **`optimize`**：任务清单 plan → apply（或等价两阶段）；主路径高对齐
- 编排住在 `vole-core::ops`；`vole-cli` 为薄前端（与现架构一致）
- 协议：优先 **追加** 事件/字段；破坏性改动须 bump `schema_version` 并按 SemVer 策略评估包版本
- 发版：Developer ID 签名公证、Homebrew Formula、`docs/releases/` 与现流水线一致

### 4.2 明确不做（本代际）

- `purge` / `installer` / `touchid` / `update` / `hints`
- 真 sudo / 系统级提权清理（遇需提权路径仍跳过并响亮告知，同 v1 §4.2 精神）
- Linux
- SwiftUI 本体
- orphaned apps（可穿插 1.x，**不阻塞** v2 宣告）

### 4.3 可穿插、不阻塞宣告

- clean 长尾规则 / guard 子集
- B4 orphaned apps（独立安全评审）
- 文档、fixture、conformance 加固

## 5. 里程碑与版本

| 里程碑 | 内容 | 包版本 |
|---|---|---|
| **M0** | Mole `uninstall` 库存与安全面 spike；划定主路径 vs 长尾 | 无发版或 docs-only |
| **M1** | 实现 `uninstall` + 测试 / fixture / 菜单 / 补全 | **1.1.0** |
| **M2** | Mole `optimize` spike；划定任务子集 | docs / 小改 |
| **M3** | 实现 `optimize` + 同档质量门禁 | **1.2.0** |
| **收口** | findings 勾选 + README「产品 v2 CLI 成熟」；再开 SwiftUI 另仓 | — |

顺序：M0 → M1 → Release 1.1.0 → M2 → M3 → Release 1.2.0 → 宣告成熟 → 桌面另仓。

## 6. 架构约束（实施时遵守）

1. **复用** clean 已有能力：路径校验、保护层、废纸篓、oplog、不可信 plan 闸口
2. **`uninstall`** 重度依赖已移植的 app protection 语义；缺口在 M0/M1 补齐，**不绕过**安全闸口
3. **提权**：本代际不实现 sudo；设计备注可预留日后 CLI sudo 与桌面 `SMAppService` 的 trait 边界，但不落地实现
4. **许可证**：GPL-3.0-only
5. **Mole 库存**：钉版本策略与 v1 一致，不盲跟上游演进

## 7. 与 v1 / SemVer 的关系

- v1 closeout 将 `uninstall` / `optimize` / 桌面标为非目标或另轨；本设计将其升为 **产品 v2 主路径**（桌面仍延后）
- 新兼容子命令按 [`2026-07-30-semver-policy-design.md`](2026-07-30-semver-policy-design.md) 递增 **MINOR**，不因「产品 v2」叙事直接发 `2.0.0`
- v1.x backlog（B4 orphaned、规则长尾）可与 M1–M3 穿插，但不替代本代际成功标准

## 8. 文档阶段验收

- 本设计无占位符、无范围自相矛盾
- 产品「v2」与包 `1.x` 关系写死
- SwiftUI 不在本代际并行主路径写死

## 9. 下一步

**M0/M1（已完成）**：[`../plans/2026-07-30-1910-v2-m0-m1-uninstall.md`](../plans/2026-07-30-1910-v2-m0-m1-uninstall.md) → 包 **`1.1.0`**；findings：[`../../findings/2026-07-v2-m0-uninstall-spike.md`](../../findings/2026-07-v2-m0-uninstall-spike.md)、[`../../findings/2026-07-v2-m1-uninstall.md`](../../findings/2026-07-v2-m1-uninstall.md)。

**M2/M3 实施计划**：[`../plans/2026-07-30-2012-v2-m2-m3-optimize.md`](../plans/2026-07-30-2012-v2-m2-m3-optimize.md)（`optimize` spike + 实现 → 包 `1.2.0`）。

**M2 spike findings**：[`../../findings/2026-07-v2-m2-optimize-spike.md`](../../findings/2026-07-v2-m2-optimize-spike.md)。
