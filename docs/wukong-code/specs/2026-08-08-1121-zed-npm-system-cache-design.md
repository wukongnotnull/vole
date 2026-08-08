# W2c 续刀：Zed system-node npm cache（`zed-npm-system-cache`）

- 日期：2026-08-08
- 状态：已批准（Condensed；默认采纳）
- 依据：Mole 对齐路线图 §2.3 W2c；`third_party/mole-1.48.1/lib/clean/app_caches.sh` Code editors 块；[`2026-08-08-0158-filo-production-cache-design.md`](2026-08-08-0158-filo-production-cache-design.md) 并列未选候选
- 包版本意图：**1.37.0**（MINOR）；规则 **534 → 535**（main 现 1.35.0；并行 W2b② 若先占 1.36 则本刀仍为 1.37，若撞车再顺延）
- 分支：`feat/clean-batch6-zed-npm-system-cache`

## 1. 结论

交付 **1** 条窄 `strategy.kind = "all"` 规则，补齐 Mole 已清理、Vole `zed-npm-cache`（`node-v*/cache`）未覆盖的 system-node scratch 路径：

| 项 | 值 |
|---|---|
| `id` | `zed-npm-system-cache` |
| Mole | `app_caches.sh` ≈133：`safe_clean ~/Library/Application Support/Zed/node/cache/* "Zed npm cache"` |
| `paths` | `~/Library/Application Support/Zed/node/cache/*` |
| 文件 | `data/rules/app-caches.toml`（紧邻既有 `zed-npm-cache`） |
| label | `Zed system-node npm cache`（区别于 `node-v*/cache` 的「Zed npm cache」） |
| handler | **无**（纯 TOML `all`；走既有 plan/apply） |
| sudo | **否** |

不扩 `user.sh` 广域；不增 custom；不改 W2a / W2b 核心（仅 coverage 句 + Cargo 版本窄改）。

## 2. 盘点：为何选这条

Filo 首刀（1.32.0 / #71）设计已标明路径级伪已移植余量二选一，当时择 Filo 家族空洞，**明确把 Zed `node/cache` 留给后续 Batch**。现状复核：

| 候选 | Mole 路径 | 现状 | 价值 / 边界 |
|---|---|---|---|
| **Zed `node/cache`（已选）** | `…/Zed/node/cache/*`（`app_caches.sh`） | Vole `zed-npm-cache` 仅 `node/node-v*/cache/*`；同 label 导致 inventory「伪已移植」 | Mole 注释称 system-node scratch；与 per-version runtime 并列；不动 `db/`；fixture `clean_code_editors_includes_zed_caches` 已置 `_cacache` 样本 |
| 其它 `dev.sh` / `app_caches` 广域 | — | — | 本刀限 1 条；禁止 `user.sh` / custom |

**排除**：合并进既有 `zed-npm-cache` paths（会模糊 id 语义且不易单测归因）；`user.sh`；custom；需 sudo 的 system 路径。

## 3. Condensed 方案（≤5 点）

1. 仅追加一条 TOML 规则；`category = "app-caches"`；`last_verified = "2026-08"`。
2. 保护：路径含 `/cache/` 子段且位于 Application Support 下 npm scratch；沿用既有 cache 保护约定，**预期零保护层改动**（实现时以 fixture / plan_verify 确认）。
3. 单测 / fixture：`tests/fixtures/clean/w2c_zed_npm_system_cache_selects_child.json`（plan 可见子项）；`verify_clean_fixtures` 绿。
4. `coverage_note`「已落地」追加短名 **Zed system-node npm cache**；启用规则数 **535**。
5. 发版文档 / Formula / README 对齐 **1.37.0**（不 bump `schema_version`）。

## 4. 非目标

- 改名或合并既有 `zed-npm-cache`（`node-v*`）
- `user.sh` 广域、盲扩 custom
- PrivilegeBackend / sudo
- 改 `protection.toml` 或 plan/apply 接线（标准 `all` 已通）

## 5. 验收

- [ ] 启用规则 535；版本 1.37.0（若 main 已被并行轨抬高则顺延 MINOR）
- [ ] plan 对 fixture 根下 `…/Zed/node/cache/*` 产出 `rule_id=zed-npm-system-cache`
- [ ] `cargo test -p vole-core`（含 fixture）绿；clippy `-D warnings`
- [ ] coverage 含本规则短名；未扩 custom / user 广域
- [ ] PR 经 review + CI 后 **merge commit** 合入；0119 另开小 PR 记「W2c 续刀完成」
