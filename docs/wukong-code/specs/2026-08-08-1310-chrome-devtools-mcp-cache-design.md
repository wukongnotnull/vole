# W2c Batch 6+：Chrome DevTools MCP browser Cache（`chrome-devtools-mcp-cache`）

- 日期：2026-08-08
- 状态：已批准（Condensed；默认采纳）
- 依据：Mole 对齐路线图 §2.3 W2c；`third_party/mole-1.48.1/lib/clean/dev.sh` `clean_chrome_devtools_mcp_caches` / `clean_chromium_default_caches`；[`2026-08-08-1230-antigravity-browser-cache-design.md`](2026-08-08-1230-antigravity-browser-cache-design.md) 之后续挑
- 包版本意图：**1.40.0**（MINOR）；规则 **536 → 537**
- 分支：`feat/clean-batch6-chrome-devtools-mcp-cache`

## 1. 结论

交付 **1** 条窄 `strategy.kind = "all"` 规则，补齐 Mole `dev.sh` 已清理、Vole 尚未覆盖的 Chrome DevTools MCP Chromium profile 主 HTTP 缓存：

| 项 | 值 |
|---|---|
| `id` | `chrome-devtools-mcp-cache` |
| Mole | `dev.sh` `clean_chrome_devtools_mcp_caches`：`clean_chromium_default_caches "$HOME/.cache/chrome-devtools-mcp/chrome-profile" "Chrome DevTools MCP"` → `…/Default/Cache/*`（label「Chrome DevTools MCP browser cache」） |
| `paths` | `~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/*` |
| 文件 | `data/rules/user-devtools.toml`（独立 MCP 家族；可紧邻 `antigravity-browser-cache` 或其后） |
| handler | **无**（纯 TOML `all`；走既有 plan/apply） |
| sudo | **否** |
| guards | **无**（与 `antigravity-browser-cache` 一致；Mole 运行时 `chrome_devtools_mcp_running` 跳过属体验差，本刀不扩 guard 面） |

不扩 `user.sh` 广域；不增 custom；不改 W2a / W2b 核心（仅 coverage 句 + Cargo 版本窄改）。

## 2. 盘点：为何选这条

`scripts/inventory-mole-rules.py` 仍报 `unported_all = 0`（按 label/id）。路径级 / 变量展开差集候选：

| 候选 | Mole 路径 | 现状 | 价值 / 边界 |
|---|---|---|---|
| **Chrome DevTools MCP `Default/Cache`（已选）** | `~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/*`（`dev.sh`） | **整族未移植** | 与 Antigravity browser Cache 同构；开新家族；体量常为 Chromium HTTP 主缓存；`/Cache/` 保护豁免已通；无 sudo |
| Antigravity profile 兄弟（未选） | `…/Default/Code Cache` / `GPUCache` / Dawn* / Graphite / crx | profile 仅落地 `Default/Cache` | 窄且安全；价值为补齐已开家族，略低于开新 MCP 家族 |
| QQ Music `iRRCache`（未选） | Containers `…/QQMusicMac/iRRCache/*`（`app_caches.sh`） | Mac Caches + container Caches 已齐，AS `iRRCache` 等未齐 | 纯 `all`；保护边界清晰（勿碰 `iDownloadProxy`）；价值并列，留给后续 |

**排除**：`user.sh`；盲扩 custom；Service Worker / Dawn/GrShader / component_crx / extensions_crx（可续刀）；需 sudo 的 system 路径；本刀一次只 1 条。

## 3. Condensed 方案（≤5 点）

1. 仅追加一条 TOML 规则；`category = "user-devtools"`；`last_verified = "2026-08"`。
2. 保护：`is_explicit_clean_cache_path` 已含 `"/Cache/"` → **零保护层改动**。
3. 单测 / fixture：`tests/fixtures/clean/w2c_chrome_devtools_mcp_cache_selects_child.json`；`verify_clean_fixtures` 绿。
4. `coverage_note`「已落地」追加短名 **Chrome DevTools MCP Cache**；启用规则数 **537**。
5. 发版文档 / Formula / README 对齐 **1.40.0**（不 bump `schema_version`）。

## 4. 非目标

- 同刀植入 MCP profile 的 Code/GPU/Dawn/GrShader/Graphite/crx / Service Worker
- Antigravity profile 兄弟路径 / QQ Music `iRRCache`
- PrivilegeBackend / sudo；`user.sh`；custom
- 为对齐 Mole 而新建 `not_running`（与 `antigravity-browser-cache` 保持一致）

## 5. 验收

- [ ] 启用规则 537；版本 1.40.0
- [ ] plan 对 fixture 根下 `…/chrome-devtools-mcp/chrome-profile/Default/Cache/*` 产出 `rule_id=chrome-devtools-mcp-cache`
- [ ] `cargo test -p vole-core`（含 fixture）绿；clippy `-D warnings`
- [ ] coverage 含本规则短名；未扩 custom / user 广域
- [ ] PR 经 review + CI 后 **merge commit** 合入；0119 另开小 PR：本刀完成；下一刀——若差集仍大可续 W2c，否则明确写「暂停 Batch6 必做；optimize 后置长尾保持 coverage；W3 不开发」
