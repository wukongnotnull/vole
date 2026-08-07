# W2c：Filo production Chromium Cache（`filo-production-cache`）

- 日期：2026-08-08
- 状态：已批准（Condensed；默认采纳）
- 依据：Mole 对齐路线图 §2.3 W2c；`third_party/mole-1.48.1/lib/clean/dev.sh` Filo Electron 块；`docs/findings/2026-07-phase4c-v1-summary.md` Batch 6+ 方向
- 包版本意图：**1.32.0**（MINOR）；规则 **533 → 534**
- 分支：`feat/clean-batch6-filo-production-cache`

## 1. 结论

交付 **1** 条窄 `strategy.kind = "all"` 规则，补齐 Mole `dev.sh` 已清理、Vole Filo 家族尚未覆盖的主 Chromium HTTP 缓存：

| 项 | 值 |
|---|---|
| `id` | `filo-production-cache` |
| Mole | `dev.sh` ≈2054：`safe_clean ~/Library/Application Support/Filo/production/Cache/* "Filo cache"` |
| `paths` | `~/Library/Application Support/Filo/production/Cache/*` |
| 文件 | `data/rules/user-devtools.toml`（与既有 `filo-code-cache` / `filo-gpu-cache` / Dawn 同簇） |
| handler | **无**（纯 TOML `all`；走既有 plan/apply） |
| sudo | **否** |

不扩 `user.sh` 广域；不增 custom；不改 W1 / W2a / W2b 核心（仅 coverage 句 + Cargo 版本窄改）。

## 2. 盘点：为何选这条

`scripts/inventory-mole-rules.py` 报告 `unported_all = 0`（`all` 按 **label/id** 已对上）。路径级对照（`app_caches.sh` + `dev.sh` 字面 `safe_clean` vs `data/rules`）仍暴露 **2** 条伪已移植：

| 候选 | Mole 路径 | 为何伪已移植 | 价值 / 边界 |
|---|---|---|---|
| **Filo production Cache（已选）** | `…/Filo/production/Cache/*`（`dev.sh`） | Vole 已有同 label「Filo cache」→ `~/Library/Caches/com.filo.client/*`，**路径不同** | Electron 主 Cache，体量大；邻接 `Code Cache`/`GPUCache`/`Dawn*` **已移植**，只差本条；保护层已认 `/Cache/` 段 |
| Zed `node/cache`（未选） | `…/Zed/node/cache/*`（`app_caches.sh`） | 同 label「Zed npm cache」已覆盖 `node-v*/cache` | 亦窄且安全；价值略低于补齐 Filo 家族空洞；留给后续 Batch |

**排除**：`user.sh`；注释掉的 CocoaPods/Flutter/Pub；custom（obsolete extensions 等）；需 sudo 的 system 路径。

## 3. Condensed 方案（≤5 点）

1. 仅追加一条 TOML 规则；`category = "user-devtools"`；`last_verified = "2026-08"`。
2. 保护：`is_explicit_clean_cache_path` 已含 `"/Cache/"` → **零保护层改动**。
3. 单测 / fixture：`tests/fixtures/clean/` 一条（计划可见候选）；`verify_clean_fixtures` + 可选 `verify-clean-candidates`。
4. `coverage_note`「已落地」追加短名 **Filo production Cache**；规则启用数 534。
5. 发版文档 / Formula / README 对齐 **1.32.0**（不 bump `schema_version`）。

## 4. 非目标

- 合并或重命名既有 `filo-cache`（Library/Caches）
- Zed system-node npm cache（并列下一批）
- PrivilegeBackend / sudo
- 改 `protection.toml` 或 plan/apply 接线（标准 `all` 已通）

## 5. 验收

- [ ] 启用规则 534；版本 1.32.0
- [ ] plan 对 fixture 根下 `…/Filo/production/Cache/*` 产出 `rule_id=filo-production-cache`
- [ ] `cargo test -p vole-core`（含 fixture）绿；clippy `-D warnings`
- [ ] coverage 含本规则名；未扩 custom / user 广域
- [ ] PR 经 review + CI 后 **merge commit** 合入
