# Phase 4c 续：Clean 规则扩展（Batch 6）设计

- 日期：2026-07-30
- 状态：已确认；实施计划见 `docs/wukong-code/plans/2026-07-30-phase4c-rules-batch6.md`
- 父设计：`docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §8 Phase 4c+
- 前置：Phase 4c v1 收官（**150** 条）；v0.0.1 Release
- 上一批：`docs/wukong-code/specs/2026-07-29-phase4c-rules-batch5-design.md`

## 1. 背景与目标

| 项 | Batch 5 后 | Batch 6 目标 |
|---|---|---|
| 启用规则 | **150** | **190** (+40) |
| 库存 `ported` | 144 | **184** |
| 未移植 `all` | ≈312 | ≈272 |

### 本批目标

1. Phase 4c **v1 后首批**扩面：净增 **40** 条纯 `all` 规则。
2. **Block A**：流媒体/下载/播放器长尾（iQIYI、斗鱼/虎牙、Transmission 等）。
3. **Block B**：DB/API 客户端 + 构建/CI 缓存（Sequel、Postman、Bazel、Jenkins 等）。
4. **0 custom**；不碰 `user.sh` 广域。

### 非目标

- mole 注释掉的 CocoaPods / Flutter / Dart Pub
- guard / not_running / Final Cut generated
- `user.sh` 130 条广域规则

## 2. 门禁

- `cargo test -p vole-core verify_clean_fixtures`
- `bash scripts/verify-clean-candidates.sh`
- 5× `batch6_*` fixture + 可选 VOLE_TEST_ROOT 双跑

## 3. 文档

- 选批：`docs/findings/2026-07-phase4c-batch6-selection.md`
- README 规则数 → **190**
