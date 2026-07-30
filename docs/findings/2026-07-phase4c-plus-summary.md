# Phase 4c+ 收官总结（Batch 6–13）

**日期**：2026-07-30  
**状态**：Phase 4c+ **已完成**（`all` 策略 mole 规则移植收尾）  
**Release**：**v0.0.7**（470 规则）

---

## 1. 目标与结果

| 项 | v0.0.1 (Batch 5) | v0.0.7 (Batch 13) |
|---|---|---|
| 启用规则 | **150** | **470** |
| 库存 `ported` | 144/513 (28%) | **442/513 (86%)** |
| 未移植 `all` | 312 | **22**（多为 id  slug 重复，路径已覆盖） |
| Fixture JSON | 46 | **86** |
| Release | v0.0.1 | v0.0.1 → v0.0.7 |

**结论**：mole `safe_clean` 中 **`strategy.kind = all`** 类规则已基本移植完毕；剩余 22 条 inventory 差集主要为 **同路径不同 proposed_id**（Arc/Chrome 变体）及 **刻意跳过** 项。

---

## 2. 分批轨迹（Batch 6–13）

| 批次 | 净增 | 累计 | 主题 | Release |
|---|---|---|---|---|
| Batch 6 | +40 | 190 | 流媒体/游戏/dev CI | — |
| Batch 7 | +40 | 230 | 游戏/download/Figma 等 | — |
| Batch 8 | +40 | 270 | 保护层 refine + AI/DB | **v0.0.2** |
| Batch 9 | +40 | 310 | Email/shell/Warp/pre-commit | **v0.0.3** |
| Batch 10 | +40 | 350 | Notion/remote/Apple system | **v0.0.4** |
| Batch 11 | +40 | 390 | macOS system + 浏览器主干 | **v0.0.5** |
| Batch 12 | +40 | 430 | Chrome/Arc/Dia/Helium 剩余 | **v0.0.6** |
| Batch 13 | +40 | **470** | Office/VM/user.sh 广域收尾 | **v0.0.7** |

选批文档：`docs/findings/2026-07-phase4c-batch{6..13}-selection.md`

---

## 3. 规则分布（main @ 2026-07-30）

| 文件 | `[[rule]]` 数 |
|---|---|
| `app-caches.toml` | 243 |
| `user-devtools.toml` | 221 |
| `ai-agents.toml` | 3 |
| `codex.toml` | 1 |
| `example.toml` | 2 |
| **合计** | **470** |

---

## 4. 关键工程变更

| 变更 | 批次 | 说明 |
|---|---|---|
| 保护层 refine | Batch 8 | `is_explicit_clean_cache_path()` — Library/Caches 下 explicit 规则跳过 bundle guard |
| VOLE_TEST_ROOT 双跑 | PR #14 | mole↔vole plan 对比 + sentinel 标记 |
| CI fix | v0.0.3 | clippy collapsible_if + rustfmt |
| Release 流水线 | v0.0.1–v0.0.7 | tag 触发 + 手动 tarball 补传兜底 |

---

## 5. 剩余未移植（inventory @ v0.0.7）

```bash
python3 scripts/inventory-mole-rules.py
# ported: 442, unported_all: 22
```

### 5.1 伪差集（路径已覆盖，slug 不匹配）

Arc/Chrome/Brave/Dia 变体 — vole 使用 `arc-root-*` / `arc-profile-*` / `arc-user-data-*` 等 id，inventory `proposed_id` 仍为 mole 原始 slug。

`homebrew-cache` — 同路径已在 `homebrew-downloads-cache`（Batch 10）。

### 5.2 刻意排除

| 项 | 原因 |
|---|---|
| `claude-pending-uploads` | Claude bundle 保护，非 explicit cache |
| `rosetta-2-cache` (`/Library/...`) | 系统路径，需 sudo |
| **guard** (44) | `pgrep` / `not_running` — 引擎未扩 |
| **custom** (14) | symlink/动态 label — v1 配额 3 条 legacy |
| **sudo** (2) | 设计 SkipReason |
| **mtime** (1) | Pacifist keep-by-age |

---

## 6. 验证

```bash
cargo test -p vole-core verify_clean_fixtures
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
bash scripts/verify-clean-candidates.sh   # VOLE_TEST_ROOT 双跑
```

---

## 7. 建议后续

1. ~~**guard 子集**~~ — 已落地（见 `2026-07-guard-not-running.md`）
2. ~~**inventory slug 对齐**~~ — 已落地（见 `2026-07-inventory-slug-alignment.md`；`unported_all` 22→2）
3. ~~**Developer ID 下 TCC 完整矩阵**~~ — 已测（`2026-07-phase1-tcc-devid-matrix.md`）
4. （可选）cmdline/`pgrep -f`、FCP generated 等重 guard
5. （可选）SwiftUI 桌面 app（协议已冻结）
6. （可选）Raycast / `open -a` 手测补全 TCC 矩阵

---

**Phase 4c+ Done.** 父文档：[`2026-07-phase4c-v1-summary.md`](2026-07-phase4c-v1-summary.md)
