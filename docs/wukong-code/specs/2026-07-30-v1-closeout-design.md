# Vole v1 Closeout 设计

- 日期：2026-07-30
- 状态：已批准
- 依据：`2026-07-29-rust-rewrite-design.md` §1；用户认可收口方案

## 1. 结论

**设计文档中的 v1 CLI 产品目标已达成。**  
本收口不新增 Mole 子命令；重点是范围冻结、文档诚实、发版追上 main（509 规则）。

## 2. v1 已完成（对照设计）

| 目标 | 交付 |
|---|---|
| `status` / `analyze` / `clean` | 可用；plan→apply；默认废纸篓 |
| 协议为桌面预留 | `docs/protocol.md` FROZEN |
| 保护路径 / JSON 子集 / oplog | 已对齐 |
| 规则 | **509**（远超原 Top 150） |
| Phase 5 | history、菜单、补全、Developer ID、公证、Formula、install.sh |

## 3. v1 非目标（不做）

`uninstall` / `optimize` / `purge` / `installer` / `touchid` / `update` / `hints` / 真 sudo / Linux / SwiftUI 本体。

刻意不移植规则：`claude-pending-uploads`、`rosetta-2-cache`（`/Library` sudo）。

## 4. 本收口必做（A）

1. 文档：本 design + findings 勾选 + README「v1 完成」
2. Release **v0.0.11**（509 规则）
3. Formula / README 安装版本对齐

## 5. 可选 v1.x（B，不阻塞发版）

- JetBrains Toolbox keep-N
- Codex Desktop stale staging（无 lsof）
- plan 同路径去重（`user-app-cache` 与具名规则双报）
- orphaned apps（高风险，独立里程碑）

## 6. 另轨（C）

SwiftUI 桌面 app；Mole 其余命令。

## 7. 验收

- Release v0.0.11 双架构资产 + 签名公证流水线绿
- Formula `0.0.11` sha 正确
- README / findings 明确「v1 CLI 功能完成」
