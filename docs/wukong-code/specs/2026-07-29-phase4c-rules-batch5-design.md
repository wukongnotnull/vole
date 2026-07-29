# Phase 4c 续：Clean 规则扩展（Batch 5 / v1 收官）设计

- 日期：2026-07-29
- 状态：已确认（2026-07-29）；实施计划见 `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch5.md`
- 父设计：`docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §8 Phase 4c
- 前置：Batch 4 已合入；约 **126** 条启用规则；Top 100 已达成
- 上一批：`docs/wukong-code/specs/2026-07-29-phase4c-rules-batch4-design.md`

## 1. 背景与目标

### 现状（Batch 4 后）

| 项 | 状态 |
|---|---|
| 启用规则合计 | ≈ **126** |
| 设计 v1 目标 | Top **100–150** |
| 距 150 上限 | **24** 条 |

### 本设计目标

1. **Batch 5（收官批）**：净增 **24** 条，累计 **≈ 150**，完成 Phase 4c v1 规则移植目标。
2. 优先 **媒体/音乐播放器**（app_caches）与 **移动/构建链剩余**（dev.sh 中仍启用的 `safe_clean`）。
3. 本批 **0 custom**；批次规模低于 usual [30,50] 区间，以 findings 说明（v1 封顶 150）。

### 非目标

- 不移植 mole 中已注释的 CocoaPods / Flutter / Dart Pub。
- 不移植 guard / user.sh 广域 / Final Cut generated。

## 2. 选批概要

| 块 | 条数 | 主题 |
|---|---|---|
| A | 12 | 音乐/视频 app（QQ Music、Bilibili、Plex、IINA/VLC、Stremio 等） |
| B | 12 | Android/Expo/Gradle/Composer/Deno/Terraform/Xcode IB |

## 3. 门禁

- 全量测试 + verify-clean-candidates
- 总规则 ≈ **150**；custom ≤ 5%
- README 标记 Phase 4c v1 **完成**

## 4. 后续（Phase 4c+）

v1 收官总结见 `docs/findings/2026-07-phase4c-v1-summary.md`。剩余 ≈310+ 未移植 `all` 候选 + guard/custom 类；v1 报告提示「继续用 Mole」或开 Batch 6+ 扩面。
