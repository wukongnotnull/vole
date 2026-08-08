# Findings: Chrome DevTools MCP Cache（1.40.0 / W2c Batch 6+）

**日期**：2026-08-08  
**状态**：落地中  
**规则**：`chrome-devtools-mcp-cache`

## 动机

Antigravity browser Cache 落地后，路径级差集仍暴露整族未移植的 Chrome DevTools MCP Chromium profile（`~/.cache/chrome-devtools-mcp/chrome-profile`）。与 Antigravity browser Cache 同构：优先开家族主 `Default/Cache`，兄弟 Dawn/crx/Service Worker 留给续刀。

Mole：`clean_chrome_devtools_mcp_caches` → `clean_chromium_default_caches` → `…/Default/Cache/*`（label「Chrome DevTools MCP browser cache」）。

## 边界

- 纯 `strategy.kind = "all"`；无 custom / 无 sudo / PrivilegeBackend
- 不同刀植入 Code/GPU/Dawn/GrShader/Graphite/crx / Service Worker
- 未加 `not_running`（与 `antigravity-browser-cache` 一致；Mole 运行时跳过属体验差）
- 禁止 `user.sh` 广域、盲增 custom
