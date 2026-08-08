# Findings: Zed system-node npm cache（1.37.0 / W2c 续刀）

**日期**：2026-08-08  
**状态**：落地中  
**规则**：`zed-npm-system-cache`

## 动机

Filo 首刀（1.32.0）设计已标明：路径级差集上 Zed `node/cache` 与 Filo production Cache 并列伪已移植；当时择 Filo，本刀补 Zed。

`scripts/inventory-mole-rules.py` 按 label「Zed npm cache」把 Mole `app_caches.sh` 的两条路径都标为已移植，但 Vole `zed-npm-cache` 仅覆盖 `~/Library/Application Support/Zed/node/node-v*/cache/*`。Mole 另清理 system-node scratch：`…/Zed/node/cache/*`。

## 边界

- 纯 `strategy.kind = "all"`；不碰 `db/`；无 custom / 无 sudo / PrivilegeBackend
- 不合并进既有 `zed-npm-cache` id（便于单测归因）
- 保护层实测不必改动（小写 `/cache/` 路径本身未被 cleanup 拦）
