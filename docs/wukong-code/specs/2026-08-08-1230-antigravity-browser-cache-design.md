# W2c Batch 6+：Antigravity browser profile Cache（`antigravity-browser-cache`）

- 日期：2026-08-08
- 状态：已批准（Condensed；默认采纳）
- 依据：Mole 对齐路线图 §2.3 W2c；`third_party/mole-1.48.1/lib/clean/dev.sh` `clean_antigravity_caches` / `clean_chromium_default_caches`；[`2026-08-08-1121-zed-npm-system-cache-design.md`](2026-08-08-1121-zed-npm-system-cache-design.md) 续刀之后再挑
- 包版本意图：**1.39.0**（MINOR）；规则 **535 → 536**
- 分支：`feat/clean-batch6-antigravity-browser-cache`

## 1. 结论

交付 **1** 条窄 `strategy.kind = "all"` 规则，补齐 Mole `dev.sh` 已清理、Vole Antigravity Electron（`Application Support/Antigravity/*`）未覆盖的 Gemini browser profile 主 Chromium HTTP 缓存：

| 项 | 值 |
|---|---|
| `id` | `antigravity-browser-cache` |
| Mole | `dev.sh` `clean_antigravity_caches`：`clean_chromium_default_caches "$HOME/.gemini/antigravity-browser-profile" "Antigravity"` → `…/Default/Cache/*`（label「Antigravity browser cache」） |
| `paths` | `~/.gemini/antigravity-browser-profile/Default/Cache/*` |
| 文件 | `data/rules/user-devtools.toml`（紧邻既有 `antigravity-cache` / code / GPU / Dawn） |
| handler | **无**（纯 TOML `all`；走既有 plan/apply） |
| sudo | **否** |
| guards | **无**（与既有 Antigravity AS 规则一致；Mole 运行时 `antigravity_or_gemini_running` 跳过属体验差，本刀不扩 guard 面） |

不扩 `user.sh` 广域；不增 custom；不改 W2a / W2b 核心（仅 coverage 句 + Cargo 版本窄改）。

## 2. 盘点：为何选这条

`scripts/inventory-mole-rules.py` 仍报 `unported_all = 0`（按 label/id）。`app_caches.sh` + `dev.sh` 字面 `~/` `safe_clean` 已齐；**变量展开后的路径级差集**仍暴露整簇空洞：

| 候选 | Mole 路径 | 现状 | 价值 / 边界 |
|---|---|---|---|
| **Antigravity browser `Default/Cache`（已选）** | `~/.gemini/antigravity-browser-profile/Default/Cache/*`（`dev.sh`） | Vole 仅有 `~/Library/Application Support/Antigravity/{Cache,Code Cache,GPUCache,Dawn*}`；**profile 树整簇未移植** | 同 Filo 叙事：Electron AS 家族已齐，只差 browser profile 主 Cache；`/Cache/` 已在保护豁免；无 sudo |
| Chrome DevTools MCP `Default/Cache`（未选） | `~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/*` | 整族未移植 | 亦窄且安全；留给后续 Batch 开新家族 |
| QQ Music `iRRCache` 等（未选） | Containers `…/QQMusicMac/iRRCache/*` 等 | Mac Caches 已齐，AS 子树未齐 | 纯 `all`；价值并列，本刀优先 `dev.sh` 与 Filo 同源块 |

**排除**：`user.sh`；盲扩 custom；Service Worker / component_crx / extensions_crx（可续刀）；需 sudo 的 system 路径；本刀一次只 1 条。

## 3. Condensed 方案（≤5 点）

1. 仅追加一条 TOML 规则；`category = "user-devtools"`；`last_verified = "2026-08"`。
2. 保护：`is_explicit_clean_cache_path` 已含 `"/Cache/"` → **零保护层改动**。
3. 单测 / fixture：`tests/fixtures/clean/w2c_antigravity_browser_cache_selects_child.json`；`verify_clean_fixtures` 绿。
4. `coverage_note`「已落地」追加短名 **Antigravity browser Cache**；启用规则数 **536**。
5. 发版文档 / Formula / README 对齐 **1.39.0**（不 bump `schema_version`）。

## 4. 非目标

- 合并进既有 `antigravity-cache`（`Application Support`）
- 同刀植入 profile 的 Code/GPU/Dawn/Graphite/crx 兄弟路径（留给续刀）
- Chrome DevTools MCP / QQ Music iRRCache（并列下一批）
- PrivilegeBackend / sudo；`user.sh`；custom
- 为对齐 Mole 而新建 `not_running`（与现有 Antigravity AS 规则保持一致）

## 5. 验收

- [ ] 启用规则 536；版本 1.39.0
- [ ] plan 对 fixture 根下 `…/antigravity-browser-profile/Default/Cache/*` 产出 `rule_id=antigravity-browser-cache`
- [ ] `cargo test -p vole-core`（含 fixture）绿；clippy `-D warnings`
- [ ] coverage 含本规则短名；未扩 custom / user 广域
- [ ] PR 经 review + CI 后 **merge commit** 合入；0119 另开小 PR 记本续刀完成与下一刀
