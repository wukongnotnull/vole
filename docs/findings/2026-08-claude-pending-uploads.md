# Claude pending-uploads（1.11.0）

落地 Mole `safe_clean …/Claude/pending-uploads/*`。

## 落点

- `is_claude_pending_uploads_path`：`…/Library/Application Support/Claude/pending-uploads/<leaf>` 单层叶形状豁免
- `data/rules/user-devtools.toml`：`claude-pending-uploads`（`strategy.kind = all`）
- apply 无旁路；无 sudo；不改 `protection.toml`

## 观察

- 现网 Claude Application Support 多数路径因 cleanup glob **整串**匹配而不被保护层拦住；形状豁免作纵深与产品意图显式化
- 单测以 helper 形状（叶 / 目录 / 嵌套 / Local Storage）为准，不假设 Local Storage 已被保护

## 安全

- 豁免面仅 pending-uploads 单层叶
- 嵌套路径与目录本身不进豁免
