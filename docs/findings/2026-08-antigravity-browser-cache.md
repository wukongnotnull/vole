# Findings: Antigravity browser Cache（1.39.0 / W2c Batch 6+）

**日期**：2026-08-08  
**状态**：落地中  
**规则**：`antigravity-browser-cache`

## 动机

Filo / Zed 续刀之后，字面 `~/` `safe_clean` 已齐；变量展开后的路径级差集仍暴露 `~/.gemini/antigravity-browser-profile` 整簇空洞。Vole 已有 `Application Support/Antigravity/{Cache,Code Cache,GPUCache,Dawn*}`，同 Filo 叙事：Electron AS 家族已齐，只差 Gemini browser profile 主 Chromium HTTP 缓存。

Mole：`clean_antigravity_caches` → `clean_chromium_default_caches` → `…/Default/Cache/*`（label「Antigravity browser cache」）。

## 边界

- 纯 `strategy.kind = "all"`；无 custom / 无 sudo / PrivilegeBackend
- 不合并进既有 `antigravity-cache` id（便于单测归因）
- 不同刀植入 profile 的 Code/GPU/Dawn/crx 兄弟路径（留给续刀）
- 未加 `not_running`（与既有 Antigravity AS 规则一致；Mole 运行时跳过属体验差）
