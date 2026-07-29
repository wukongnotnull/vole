# Phase 4c v1 收官总结

**日期**：2026-07-29  
**状态**：Phase 4c v1 **已完成**（Top 100–150 规则目标达成）  
**父设计**：`docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §8 Phase 4c  
**前置 spike**：`docs/findings/2026-07-spike-summary.md`（外推全量 547 条不可行 → 收缩至 Top 100–150）

---

## 1. 目标与结果

| 项 | 设计目标 | v1 实际 |
|---|---|---|
| 启用规则总数 | Top **100–150** | **150** |
| 净增（Batch 2–5） | 分 4–5 批，每批 30–50（收官批例外） | **+144**（自 Batch 2 前 ≈6 条 → 150） |
| 库存 `ported` | 尽可能覆盖高价值 `all` | **144 / 513**（28%） |
| 新增 `custom` | 全库 ≤5%；分批默认 0 | Batch 2–5 **+0**；全库 **3/150 ≈ 2%** |
| Fixture 覆盖 | 关键规则 ≥1 | **46** JSON（含 batch2–5 与 bats 抽取） |
| 协议 | 不改 NDJSON（FROZEN） | 未改 |

**结论**：Phase 0.5 spike 的「收缩方案」已落地；v1 `vole clean` 具备 **150 条**可计划/可应用的 macOS 清理规则，其余 mole 能力在报告中提示用户继续用 Mole。

---

## 2. 分批轨迹

| 批次 | PR | 净增 | 累计 | 主题 | 文档 |
|---|---|---|---|---|---|
| Batch 2 | [#3](https://github.com/wukongnotnull/vole/pull/3) | +40 | ≈46 | Xcode/VS Code/通讯/npm | `docs/findings/2026-07-phase4c-batch2-selection.md` |
| Batch 3 | [#6](https://github.com/wukongnotnull/vole/pull/6) | +40 | ≈86 | 通讯/AI 桌面/dev 缓存 | `docs/findings/2026-07-phase4c-batch3-selection.md` |
| Batch 4 | [#7](https://github.com/wukongnotnull/vole/pull/7) | +40 | ≈126 | 创意/媒体 + 前端/移动 dev | `docs/findings/2026-07-phase4c-batch4-selection.md` |
| Batch 5 | [#8](https://github.com/wukongnotnull/vole/pull/8) | +24 | **150** | 音乐/视频 + 构建链收官 | `docs/findings/2026-07-phase4c-batch5-selection.md` |

**工具链（Batch 2 建立，后续复用）**：

- `scripts/inventory-mole-rules.py` — mole `safe_clean` 库存 vs TOML 差集
- `scripts/extract-clean-fixtures.py` — bats → JSON fixture 半自动抽取
- `scripts/verify-clean-candidates.sh` — fixture 计划校验 + 可选 `VOLE_TEST_ROOT` 双跑
- `tests/fixtures/clean/` + `verify_clean_fixtures` — 表驱动断言

---

## 3. 规则数据分布（`main` @ 2026-07-29）

| 文件 | `[[rule]]` 数 | 说明 |
|---|---|---|
| `data/rules/app-caches.toml` | 83 | mole `app_caches.sh` 为主 |
| `data/rules/user-devtools.toml` | 61 | mole `dev.sh` 为主 |
| `data/rules/ai-agents.toml` | 3 | 含 2× custom |
| `data/rules/codex.toml` | 1 | custom |
| `data/rules/example.toml` | 2 | 样例 |
| **合计** | **150** | |

**策略分布（Batch 2–5 新增）**：几乎全部为 `strategy.kind = "all"`；例外为 Batch 2 的 `npm-logs-keep-newest`（`keep_newest_by_mtime keep=5`）。

**Custom handler（3 条，未增）**：

- `ai-agents.toml` ×2
- `codex.toml` ×1

---

## 4. 库存余量（未移植）

`python3 scripts/inventory-mole-rules.py` @ v1 收官：

| 复杂度 | 未移植 |
|---|---|
| `all` | **312** |
| `guard` | 42 |
| `custom` | 13 |
| `mtime` | 1 |
| `sudo` | 1 |
| **合计未移植** | **369** |

**按 mole 源文件（未移植 `all`）**：

| 源文件 | 约条数 | v1 处理 |
|---|---|---|
| `user.sh` | 171 | **刻意排除**（广域 sweep，如 `~/Library/Caches/*`） |
| `app_caches.sh` | 104 | 部分已移植；余下多为长尾 app / 容器路径 |
| `dev.sh` | 91 | 部分已移植；余下含注释掉或 guard 邻近项 |

---

## 5. 验证与门禁

每批合入前均通过：

```bash
cargo test -p vole-core
cargo test -p vole-core verify_clean_fixtures -- --nocapture
bash scripts/verify-clean-candidates.sh
cargo clippy -p vole-core -- -D warnings
```

- CI：`check` + `conformance-plan-only` 全绿（Batch 2–5 PR）
- `VOLE_TEST_ROOT` mole↔vole 双跑：开发机未设；脚本在无 env 时 **SKIP**（可接受，见 Batch 2 计划）

---

## 6. 明确排除（v1 范围外）

| 类别 | 原因 | 用户提示 |
|---|---|---|
| `user.sh` 广域规则 | 路径过宽，保护层边界难保证 | 继续用 Mole |
| `not_running` / `pgrep` guard | 本阶段未扩 guard 规则量 | 继续用 Mole |
| symlink / custom 逃逸 | 配额与工期；v1 仅 3 条 legacy custom | 继续用 Mole |
| sudo / `system.sh` | 设计 SkipReason | 报告跳过 |
| mole 注释掉的 `safe_clean` | 上游未启用（如 CocoaPods、Flutter） | 不移植 |

---

## 7. 风险与遗留

| 项 | 状态 |
|---|---|
| 标签/路径与 mole 细微漂移 | 靠 fixture；争议查 bash / 可选双跑 |
| custom 占比 | v1 收官 **2%**，低于 5% 硬顶 |
| 规则静默过期 | `last_verified = "2026-07"`；季度复核机制仍适用（设计 6.3） |
| 体积排序脚本 | 未做；非 v1 阻塞 |
| 全量 547 条外推 ~19.5 周 | 已通过 Top 150 收缩关闭 |

---

## 8. 建议后续方向

按优先级（与设计 §8 / Phase 5 findings 对齐）：

### A. 发布与签名（Phase 5 遗留）

- Developer ID + notarization（Cask 路径）
- Homebrew **Formula** 真发布（当前占位见 `docs/findings/2026-07-phase5-signing.md`）
- TCC 完整矩阵（deferred：`docs/findings/2026-07-phase1-tcc-deferred.md`）

### B. Phase 4c+（可选扩面）

- 新开 **Batch 6+**：长尾 `app_caches` / `dev.sh` 纯 `all` 规则
- 或按需启用 **guard** 子集（Final Cut / Docker daemon 等）— 需引擎与 fixture 增量
- **不**建议再扩 `custom` 直至有明确策略补强子任务

### C. 产品化 clean 报告

- plan 输出中对未覆盖 mole 类别给出「继续用 Mole」提示（**已实现**：`Plan.coverage_note` / `done.report.coverage_note`；人类 plan  footer 见 stderr）
- 可选：`VOLE_TEST_ROOT` 双跑纳入 CI disposable job（非阻塞）

### D. 其他 Phase

- Phase 4 主体（4a 保护层 / 4d plan 威胁模型）已在 `phase4-clean` 计划落地；无新增 4d 大项
- 桌面 app / sidecar：协议已 FROZEN，待 app 项目启动

---

## 9. 相关文档索引

| 文档 | 用途 |
|---|---|
| `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` | 总设计 |
| `docs/wukong-code/plans/2026-07-29-phase4-clean.md` | Phase 4 clean 实施 |
| `docs/wukong-code/specs/2026-07-29-phase4c-rules-batch{2,3,4,5}-design.md` | 各批设计 |
| `docs/findings/2026-07-spike-rule-throughput.md` | 20 条速率 spike |
| `docs/findings/2026-07-phase4c-batch*-selection.md` | 各批选批清单 |

---

**Phase 4c v1 Done.** 下一建议动作：**Homebrew Formula / 签名**（Phase 5 发布线）或 **clean 报告未覆盖提示**（小增量产品化）。
