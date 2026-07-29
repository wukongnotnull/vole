# Phase 4c 续：Clean 规则扩展（Batch 4）设计

- 日期：2026-07-29
- 状态：已确认（2026-07-29）；实施计划见 `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch4.md`
- 父设计：`docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §6、§7、§8 Phase 4c
- 前置：Batch 3 已合入；约 **86** 条启用规则；custom 占比 ≈ 3.5%
- 参照上游：Mole v1.48.1（`third_party/mole-1.48.1`）
- 上一批：`docs/wukong-code/specs/2026-07-29-phase4c-rules-batch3-design.md`

## 1. 背景与目标

### 现状（Batch 3 后）

| 项 | 状态 |
|---|---|
| 已移植（库存 `ported=true`） | 80 条 |
| 启用规则合计 | ≈ **86** |
| 未移植 `all` 候选 | **376** |
| 设计 v1 目标 | Top **100–150** 条 |

Batch 3 后距 Top 100 仅差 **14** 条。本批继续净增 **40**，累计 ≈ **126**，跨过 100 里程碑并向 150 推进。

### 本设计目标

1. **Batch 4**：净增 **30–50** 条（目标 **40**），累计 ≈ **126**。
2. 覆盖 **创意/媒体**（DaVinci、Premiere、Blender、Spotify 等）与 **前端/移动 dev 缓存**（TypeScript/Vite/Expo/Android Studio 等）。
3. 本批 **0 custom**；排除 guard / 广域 user.sh / Final Cut generated（not_running）。

### 非目标

- 不移植 `Final Cut Pro generated cache`（guard/custom 组合）。
- 不移植 `user.sh` 广域 sweep。
- 不移植 symlink / sudo / not_running 规则。

## 2. 方案（延续 Batch 2–3 方案 C）

库存差集 → 选批 → TOML → fixture → 门禁。

## 3. 文件布局

- `data/rules/app-caches.toml` — Block A +18
- `data/rules/user-devtools.toml` — Block B +22
- `docs/findings/2026-07-phase4c-batch4-selection.md`
- `tests/fixtures/clean/batch4_*`

## 4. 选批概要

| 块 | 条数 | 主题 |
|---|---|---|
| A | 18 | Teams legacy 剩余、创意/媒体、阅读/笔记 app |
| B | 22 | 云 CLI、JS 构建链、移动/Swift/Expo 缓存 |

## 5. 门禁

1. `cargo test -p vole-core` + `verify-clean-candidates.sh`
2. 净增 ∈ [30, 50]
3. custom 新增 = 0；全库 custom ≤ 5%
4. README 更新（≈ 126 条）

## 6. 止损

同 Batch 3：连续 5 条需新策略 → 停批写 findings；双跑保护分歧 → 立即停。
