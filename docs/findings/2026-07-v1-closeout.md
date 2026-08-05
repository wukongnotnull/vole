# Vole v1 Closeout 勾选

**日期**：2026-07-30  
**状态**：完成（产品目标于 v0.0.11；包版本于 **v1.0.0** 对齐 SemVer；B1–B3 见 `2026-07-v1x-dedup-codex-toolbox.md`）  
**设计**：[`docs/wukong-code/specs/2026-07-30-v1-closeout-design.md`](../wukong-code/specs/2026-07-30-v1-closeout-design.md)  
**计划**：[`docs/wukong-code/plans/2026-07-30-v1-closeout.md`](../wukong-code/plans/2026-07-30-v1-closeout.md)

## 设计 §1 / Phase 对照

| 项 | 状态 |
|---|---|
| 单一二进制、无第三方 CLI | ✅ |
| `status` | ✅ |
| `analyze` | ✅ |
| `clean` plan/apply + 废纸篓口径 | ✅ |
| 保护层 / 白名单 / oplog | ✅ |
| 规则 Top 150+（现 **511**） | ✅ |
| `history` + 菜单 + 补全 | ✅ |
| 协议 FROZEN | ✅ |
| Developer ID + 公证 | ✅（v0.0.9+） |
| Homebrew Formula | ✅（1.0.0） |
| v0.0.11 发版（509） | ✅（历史；前 SemVer 纪律） |
| **v1.0.0** 包版本对齐（511） | ✅ |
| 桌面 app | ❌ 非目标 |
| uninstall / optimize / purge / … | ❌ 非目标 |

## Inventory 剩余（刻意 / 可选）

| 类 | 条数 | 处理 |
|---|---|---|
| `all` 刻意跳过 | 2 | pending-uploads / Rosetta `/Library` |
| custom 可选 | ~5 | orphaned / 动态 label → 延后 |

## v1.x backlog

| ID | 内容 | 状态 |
|---|---|---|
| B1 | JetBrains Toolbox 旧 IDE keep-N | ✅ |
| B2 | Codex Desktop stale staging（无 lsof） | ✅ |
| B3 | plan 同路径去重 | ✅ |
| B4 | orphaned apps | 延后（≥1w + 安全评审） |
| C | SwiftUI 桌面 | 另仓：`vole-macos` Clean MVP 已开（见 `2026-08-cli-honesty-pass.md`） |

## 下一轨：产品 v2

**设计**：[`docs/wukong-code/specs/2026-07-30-1900-v2-product-goals-design.md`](../wukong-code/specs/2026-07-30-1900-v2-product-goals-design.md)

窄 v2：顺序里程碑 `uninstall`（包 **1.1.0**）→ `optimize`（包 **1.2.0**）；SwiftUI 延后；不做 purge / installer / touchid / update / 真 sudo / Linux。
