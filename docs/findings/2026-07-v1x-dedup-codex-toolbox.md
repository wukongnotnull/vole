# v1.x: plan 去重 + Codex staging + JetBrains Toolbox

**日期**：2026-07-30  
**状态**：已落地（main / PR #21；随 **v1.0.0** 发布）  
**规则**：509 → **511**

## 变更

| ID | 内容 |
|---|---|
| B3 | plan 同路径去重：先成功入选的 rule 胜出 |
| B2 | `codex-desktop-stale-update-staging`（`older_than_days=30` + Codex/Sparkle guards；无 lsof） |
| B1 | `jetbrains-toolbox-old-ide-version` custom handler（`MOLE_JETBRAINS_TOOLBOX_KEEP`，默认 keep=1） |

保护层：`Application Support/JetBrains/Toolbox/apps/` 显式允许（对齐 mole toolbox 清理）。
