# W2c Batch 6 收口：兄弟路径 + QQ Music AS 缓存

- 日期：2026-08-08
- 状态：已批准（Condensed；用户确认取消「暂停必做」、全部命名项）
- 依据：`dev.sh` `clean_antigravity_caches` / `clean_chrome_devtools_mcp_caches`；`app_caches.sh` QQ Music 容器 AS；[`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md)
- 包版本：**1.41.0**；规则 **537 → 540**
- 分支：`feat/clean-batch6-siblings-qqmusic`

## 1. 结论

恢复 Batch 6 **必做**，本刀交付 **3** 条窄 `all` 多路径规则（无 custom / 无 sudo / 无新 guard）：

| id | 覆盖 |
|---|---|
| `antigravity-browser-siblings` | profile 下 Code/GPU/Dawn* + GraphiteDawn/crx/SW（**不含**已落地的 `Default/Cache`） |
| `chrome-devtools-mcp-siblings` | MCP profile 对称兄弟（含 `DawnCache` / `GrShaderCache`） |
| `qq-music-mac-as-caches` | 容器内 `iRRCache` / `iLog` / `iCache` / `iTemp`（**禁止** `iDownloadProxy`） |

保护层：扩展 `CACHE_SEGMENTS`；新增 QQ Music 容器 AS 形状豁免（`com.tencent.` 否则会被容器 data 保护拦下）。

## 2. 路径清单

### Antigravity（`$HOME/.gemini/antigravity-browser-profile`）

- `Default/Code Cache/*`
- `Default/GPUCache/*`
- `Default/DawnGraphiteCache/*`
- `Default/DawnWebGPUCache/*`
- `GraphiteDawnCache/*`
- `component_crx_cache/*`
- `extensions_crx_cache/*`
- `Default/Service Worker/CacheStorage/*`

### Chrome DevTools MCP（`$HOME/.cache/chrome-devtools-mcp/chrome-profile`）

- `Default/Code Cache/*` … `DawnWebGPUCache/*`（同 chromium default）
- `Default/DawnCache/*`、`Default/GrShaderCache/*`
- `GraphiteDawnCache/*`、`component_crx_cache/*`、`extensions_crx_cache/*`
- `Default/Service Worker/CacheStorage/*`

### QQ Music

根：`~/Library/Containers/com.tencent.QQMusicMac/Data/Library/Application Support/QQMusicMac/`

- `iRRCache/*`、`iLog/*`、`iCache/*`、`iTemp/*`

## 3. 非目标

- `user.sh` 广域；盲增 custom
- Mole `not_running` guard
- Filo / Claude Electron 已齐项
- W3 / optimize spotlight*

## 4. 验收

- [ ] 规则 540；版本 1.41.0
- [ ] 保护单测：segments + QQ AS 豁免；iDownloadProxy 仍保护
- [ ] fixtures 各家族至少一条
- [ ] coverage / README / Formula / releases / 0119 取消暂停必做
