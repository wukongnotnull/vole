# CLI 文档 / coverage 诚实性收口

**日期**：2026-08-05  
**状态**：完成  
**动机**：产品 v2 CLI（`1.2.0`：uninstall + optimize）与 B1–B3（Toolbox / Codex staging / plan 去重）已落地，但 `coverage_note` 仍声称 Toolbox keep-N「未移植」；README 桌面端表述落后于 [vole-macos](https://github.com/wukongnotnull/vole-macos) Clean MVP。

## 变更

| 项 | 修正 |
|---|---|
| `crates/vole-core/src/ops/coverage.rs` | 标明 v2 CLI 已达与已落地 handler；诚实列出仍未移植：orphaned、sudo/系统路径 |
| `README.md` | Mole 缺口举例含 orphaned / sudo；对比表与「相关项目」对齐 Clean MVP（非夸大） |
| 本 findings | 记录收口依据 |

## 仍延后（刻意）

| ID | 内容 | 备注 |
|---|---|---|
| B4 | orphaned apps | ≥1w + 独立安全评审 |
| — | optimize/uninstall 需 sudo 长尾 | 产品边界不变 |
| — | Homebrew Core | notability；短期继续自建 tap |

## 验收

- `cargo test -p vole-core coverage_note_mentions_mole_and_count --lib` 通过
- plan 产出的 `coverage_note` 不再把 Toolbox 标为未移植
