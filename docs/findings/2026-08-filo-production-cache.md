# Findings: Filo production Cache（1.32.0 / W2c）

**日期**：2026-08-08  
**状态**：已落地（PR 待合）  
**规则**：`filo-production-cache`

## 动机

`scripts/inventory-mole-rules.py` 按 label/id 将「Filo cache」标为已移植，但 Vole 仅覆盖 `~/Library/Caches/com.filo.client/*`。Mole `dev.sh` 另有 Electron 主路径 `~/Library/Application Support/Filo/production/Cache/*`；邻接的 Code Cache / GPUCache / Dawn* 已在 Vole，唯缺本条。

路径级差集（`app_caches.sh` + `dev.sh` 字面 `safe_clean`）亦列出 Zed `node/cache`（同 label 伪已移植）；本刀择 Filo 以补齐家族空洞。

## 边界

- 纯 `strategy.kind = "all"`；保护层已认 `"/Cache/"` 段，零改 protection
- 不碰 `user.sh`、不增 custom、无 sudo / PrivilegeBackend
