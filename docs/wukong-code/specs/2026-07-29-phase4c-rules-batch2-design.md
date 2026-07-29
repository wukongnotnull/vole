# Phase 4c 续：Clean 规则扩展（Batch 2）设计

- 日期：2026-07-29
- 状态：已确认（2026-07-29）；实施计划见 `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch2.md`
- 父设计：`docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §6、§7、§8 Phase 4c
- 前置：Phase 4–5 已合入；规则引擎 / 保护层 / plan-apply / history / 协议冻结就绪
- 参照上游：Mole v1.48.1（`third_party/mole-1.48.1`）

## 1. 背景与目标

### 现状

| 项 | 状态 |
|---|---|
| 安全闸口 / 保护清单 / 策略引擎 | 已完成 |
| 已移植规则 | 首批 AI/Codex 等（`data/rules/` 约数条） |
| Fixture | `tests/fixtures/clean/` 约 3 个 |
| 设计 v1 目标 | Top **100–150** 条（按释放空间优先；全量 547 外推不可行） |

### 本设计目标

1. 建立**可持续的分批移植流程**（库存 → 选批 → TOML → fixture → 验证）。
2. 完成本期 **Batch 2**：净增约 **30–50** 条可用规则（以纯路径 `all` + `keep_newest_*` 为主）。
3. 为后续 Batch 3+ 逼近 100–150 留下可重复的工具与验收门禁。

### 非目标（本批）

- 不冲全量 547；不保证本批结束即达 150。
- 不实现提权 / `system.sh` 需 sudo 的规则（继续 `SkipReason` / 报告跳过）。
- 不改 NDJSON 协议（已 FROZEN）；不碰 Developer ID / Homebrew 真发布。
- 不新增大量 `custom` handler（见 §4 配额）。
- 不做桌面 app。

## 2. 方案选择（已定：C）

**混合策略：**

1. **库存 + 轻量排序辅助**：从 mole `safe_clean` / bats 抽出候选清单；可选在一次性 `HOME` fixture 上估体积，作批次内优先级参考（非全局绝对排名）。
2. **按 mole 分类批量移植**：优先 `app_caches` / 开发者缓存类（与 spike 前段、bats 抽取器同构）。
3. **每批闭环**：TOML + fixture + `scripts/verify-clean-candidates.sh`；可选 `VOLE_TEST_ROOT` 双跑。

备选 A（纯体积排序）因缺稳定采集脚本且机器相关，本批不作为唯一排序依据。  
备选 B（纯分类无排序）作为目录组织方式保留，体积只做批次内 tie-break。

## 3. 架构与文件布局

不改 crate 边界。增量落在数据与测试：

```
data/rules/
  ai-agents.toml          # 已有
  codex.toml              # 已有
  example.toml            # 样例；可逐步清空或迁出
  app-caches.toml         # Batch 2 新增（建议）
  user-devtools.toml      # 视选取量拆分或合并
tests/fixtures/clean/     # 每批新增 JSON
scripts/
  extract-clean-fixtures.py   # 扩 allowlist
  inventory-mole-rules.py     # 新增：候选库存（见 §5）
  verify-clean-candidates.sh  # 沿用；去掉过时 SKIP 文案若仍存在
```

规则仍经现有 `rules::load` 嵌入/加载路径进入 `Orchestrator::build_plan`。

## 4. 规则选取与配额

### Batch 2 优先池（参考 spike 表 1–12）

优先落地：

- 纯路径 `strategy.kind = "all"`：Xcode / Simulator / VS Code logs&cache / iOS device logs 等（`app_caches.sh`）。
- `keep_newest_by_mtime`：JetBrains 扩展缓存、npm logs、brew cache 年龄类（引擎已支持则只写 TOML + fixture）。

延后到后续批次：

- 重 `not_running` / symlink 保护组合（引擎有 guard 则本批可少量纳入，不计强制）。
- `custom`（Chrome model、Launch Services、sim runtime volumes 等）。

### Custom 配额

- 全库 `custom` handler **≤ 全部已启用规则的 5%**（设计 6.1）。
- Batch 2：**默认新增 0 条 custom**；若某条无法用封闭策略表达，记入 findings 并延后，而不是先加 handler 冲进度。

### 提权

- 命中需 sudo 的 mole 规则：不移植为可删除规则；若误选入库存，标记 `deferred_privilege` 并排除。

## 5. 工具：库存脚本

新增 `scripts/inventory-mole-rules.py`（名称可微调），最低能力：

1. 扫描 `third_party/mole-1.48.1/lib/clean/*.sh` 中可识别的 `safe_clean` / 等价调用点。
2. 输出 CSV/JSON：`source_file`, `approx_label_or_comment`, `paths_hint`, `complexity_guess`（`all` / `mtime` / `guard` / `custom` / `sudo`）。
3. 与 `data/rules/**/*.toml` 的已有 `id` 做差集，列出 **未移植** 候选。

体积排序（可选）：在 `VOLE_TEST_ROOT` 或用户明示的 disposable HOME 上对候选路径 `du`，写入 findings；**失败则跳过体积列，不阻塞移植**。

## 6. Fixture 与验证

### Fixture

- 优先扩展 `extract-clean-fixtures.py` allowlist（如 `clean_app_caches.bats` 中可抽取子集）。
- 半自动结果必须人工校对 `expect_selected` / `expect_not_selected`（尤其负向）。
- 每条本批关键规则至少 1 个正向或负向断言覆盖其核心路径。

### 门禁（Batch 2 Done）

1. `cargo test -p vole-core`（含 `verify_clean_fixtures`）通过。
2. `bash scripts/verify-clean-candidates.sh` 通过（无 `VOLE_TEST_ROOT` 时允许跳过 mole 双跑，但 in-process fixture 必须绿）。
3. 本批净增规则数 ∈ **[30, 50]**（若 mole 侧可安全抽取不足 30，以 findings 说明并下调，不得灌水假规则）。
4. `custom` 占比不超 5%；本批新增 custom = 0（除非书面改本设计）。
5. README / 计划 checkbox 更新规则覆盖量说明。

## 7. 止损

| 触发 | 动作 |
|---|---|
| 单批墙钟明显超预期且阻塞（例如连续 5 条需新策略/custom） | 停批；写 `docs/findings/2026-07-phase4c-batch2.md`；开策略补强子任务 |
| fixture 抽取大面积失败 | 缩小 allowlist；改为手写少量高价值 fixture，不降低安全门禁 |
| 与 mole dry-run 在 `VOLE_TEST_ROOT` 双跑出现保护相关分歧 | **立即停**该规则；先修安全/语义再继续 |

## 8. 风险

| 风险 | 缓解 |
|---|---|
| 标签/路径与 mole 细微不一致 | fixture + 可选双跑；争议回查 bash |
| 大小写不敏感卷 | 沿用设计 6.2；fixture 覆盖已知敏感路径 |
| 规则静默过期 | `last_verified = "2026-07"`；季度复核仍适用 |
| 进度幻觉（例：复制路径但无测试） | Done 定义强制 fixture/门禁 |

## 9. 实施衔接

批准本设计并审阅本文件后：

1. 用 `writing-plans` 产出  
   `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch2.md`  
   （bite-sized tasks：库存脚本 → 选批清单 → 分组 TOML → fixture → 验证 → README）。
2. 在 feature 分支执行；每任务提交；PR 合入 `main`。

## 10. 开放问题（本设计内已默认）

| 问题 | 默认 |
|---|---|
| 本批是否必须达到 50？ | 否；目标区间 30–50，下限可 findings 下调 |
| 体积排序是否阻塞？ | 否 |
| 是否允许本批 custom？ | 默认否 |
