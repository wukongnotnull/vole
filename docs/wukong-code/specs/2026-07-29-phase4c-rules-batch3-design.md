# Phase 4c 续：Clean 规则扩展（Batch 3）设计

- 日期：2026-07-29
- 状态：已确认（2026-07-29）；实施计划见 `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch3.md`
- 父设计：`docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §6、§7、§8 Phase 4c
- 前置：Batch 2 已合入（`main` @ 2026-07-29）；约 **46** 条启用规则；库存脚本与 fixture 门禁就绪
- 参照上游：Mole v1.48.1（`third_party/mole-1.48.1`）
- 上一批：`docs/wukong-code/specs/2026-07-29-phase4c-rules-batch2-design.md`

## 1. 背景与目标

### 现状（Batch 2 后）

| 项 | 状态 |
|---|---|
| 已移植（库存 `ported=true`） | 40 条 |
| 启用规则合计（含 AI/Codex/example） | ≈ **46** |
| 未移植 `all` 候选 | **416** |
| 未移植 `guard` / `custom` | 42 + 13 |
| `custom` 占比 | ≈ **6.5%**（3/46，略高于 5% 硬顶） |
| 设计 v1 目标 | Top **100–150** 条 |

Batch 2 覆盖了 Xcode/VS Code/Zed/通讯类（Discord/Slack/Zoom 等）与 npm/tnpm 子集。库存显示下一批高价值、低复杂度候选集中在：

1. **`app_caches.sh` 剩余**（≈145 条未移植 `all`）：WhatsApp/Skype/QQ/钉钉/ChatGPT/Claude/Adobe/Sketch 等
2. **`dev.sh` 开发者缓存**（≈141 条未移植 `all`）：Yarn/Poetry/cargo/Hugging Face/Docker buildx 等
3. **`user.sh` 广域规则**（≈130 条未移植 `all`）：**本批明确排除**（见 §4）

### 本设计目标

1. 完成 **Batch 3**：净增 **30–50** 条规则，累计约 **76–96** 条，向 Top 100 迈进。
2. **压低 custom 占比**：本批新增 **0** 条 custom；仅 `all` / `keep_newest_*` / `older_than_days`（引擎已支持者）。
3. 延续 Batch 2 流程：库存 → 选批 → TOML → fixture → 门禁；可选 `VOLE_TEST_ROOT` 双跑抽检。

### 非目标（本批）

- 不移植 `user.sh` 广域 sweep（如 `~/Library/Caches/*`、`~/Library/Logs/*`）——路径过宽，与保护层/误删风险冲突。
- 不移植 `not_running` / `pgrep` guard 规则（Final Cut、Docker daemon、Simulator running 等）。
- 不移植 symlink / custom / sudo 规则。
- 不改 NDJSON 协议（FROZEN）；不做 Developer ID / Homebrew 真发布。

## 2. 方案选择（延续 Batch 2 方案 C）

**混合策略：**

1. **库存差集 + 分类优先**：`inventory-mole-rules.py` 列出未移植候选；按 mole 源文件分块选批。
2. **批次内 tie-break**：同块内优先常见开发者/通讯/创意工具缓存（与 spike 表 1–8 同构的纯路径项）。
3. **每批闭环**：TOML + fixture + `verify-clean-candidates.sh`；高信心子集可选双跑。

体积排序仍为**可选增强**（`VOLE_TEST_ROOT` + `du`），失败不阻塞。

## 3. 架构与文件布局

不改 crate 边界。增量落在数据与测试：

```
data/rules/
  app-caches.toml          # 追加 Block A（通讯/AI 桌面/创意工具）
  user-devtools.toml       # 追加 Block B（dev.sh 开发者缓存）
docs/findings/
  2026-07-phase4c-batch3-selection.md   # 冻结选批（实施 Task 2）
tests/fixtures/clean/      # 新增 batch3_* JSON
scripts/
  inventory-mole-rules.py  # 沿用
  extract-clean-fixtures.py  # 扩 allowlist（clean_dev_caches.bats 等）
  verify-clean-candidates.sh
```

## 4. 规则选取与配额

### Batch 3 优先池（建议 40 条）

| 块 | 源文件 | 目标条数 | TOML 文件 | 策略 |
|---|---|---|---|---|
| **A** | `app_caches.sh` 剩余 | ~18 | `app-caches.toml` | `all` |
| **B** | `dev.sh` 纯路径 | ~22 | `user-devtools.toml` | `all` |
| **C** | `dev.sh` / `user.sh` mtime 类 | 0–5（可选） | `user-devtools.toml` | `keep_newest_*` / `older_than_days` |

**Block A 示例**（自库存 preview，实施时以选批清单为准）：

- 通讯：WhatsApp、Skype、QQ、WeCom、Feishu、Tencent Meeting
- 协作：Microsoft Teams legacy（Cache / logs / tmp 等子路径，按 mole 拆分）
- AI 桌面：ChatGPT cache、Claude desktop cache、Claude logs、LM Studio cache
- 创意：Sketch、Adobe、ScreenFlow

**Block B 示例**：

- JS：Yarn cache、Yarn v1 cache、tnpm cacache
- Python：pyenv、Poetry、Ruff、MyPy、pytest、Jupyter runtime
- ML：Hugging Face、PyTorch、TensorFlow、W&B
- Rust/Ruby：cargo registry/git、rustup downloads、rbenv、gem/bundler
- 容器：Docker BuildX、Kubernetes cache

### 明确排除

| 类别 | 原因 |
|---|---|
| `user.sh` 广域 `~/Library/Caches/*` 等 | 路径过宽；与保护语义冲突 |
| `~/Library/Preferences/*.plist` 单条 | 非缓存语义；风险高 |
| `not_running` / `pgrep` guard | 引擎 guard 本批不扩量 |
| `$var` 循环 / custom handler | 配额与工期 |
| sudo / `system.sh` | 继续 SkipReason |
| symlink 保护组合 | 延后 Batch 4+ |

### Custom 配额

- 全库 custom **≤ 5%**（设计 6.1）。Batch 2 后 6.5%，本批通过净增 **40 条 `all`** 可降至 ≈ **3.5%**（3/86）。
- Batch 3：**新增 custom = 0**。

## 5. Fixture 与验证

### Fixture

- 每条**关键**新规则至少 1 个正向或负向 fixture。
- 优先从 `clean_dev_caches.bats` / `clean_app_caches.bats` 抽取；失败则手写 `batch3_*` JSON。
- `expect_not_selected` 负向断言优先于纯正向。

### 门禁（Batch 3 Done）

1. `cargo test -p vole-core`（含 `verify_clean_fixtures`）通过。
2. `bash scripts/verify-clean-candidates.sh` 通过。
3. 本批净增 ∈ **[30, 50]**。
4. 本批新增 custom = 0；全库 custom 占比 **≤ 5%**。
5. README 更新规则覆盖量（目标 ≈ 86 条）。
6. `inventory-mole-rules.py` 可复现，`ported` 计数 +40。

## 6. 止损

| 触发 | 动作 |
|---|---|
| 连续 5 条需新策略/custom | 停批；写 `docs/findings/2026-07-phase4c-batch3.md` |
| fixture 抽取大面积失败 | 缩小 allowlist；手写高价值 fixture |
| 双跑保护相关分歧 | **立即停**该规则；修安全/语义后再继续 |
| 净增 <30 且无法补齐 | findings 说明下调；不得灌水 |

## 7. 风险

| 风险 | 缓解 |
|---|---|
| Teams legacy 多条路径重复感 | 与 mole 保持一致拆分；fixture 覆盖代表性子路径 |
| dev.sh 路径含 `*` 变体 | 对照 mole bats；负向 fixture |
| 与 mole 标签细微差异 | fixture label 逐字对齐 TOML |
| 进度幻觉 | Done 定义强制 fixture + 门禁 |

## 8. 实施衔接

批准本设计后：

1. 产出 `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch3.md`。
2. 分支 `phase4c-rules-batch3` 执行；每 Task commit；PR 合入 `main`。

## 9. 开放问题

| 问题 | 默认 |
|---|---|
| 本批目标条数 | **40**（区间 30–50） |
| 是否纳入 Block C mtime 规则 | 可选；默认 0–5 条，不足则从 Block A/B 补 `all` |
| 是否做体积排序 | 否（不阻塞） |
| Batch 4 方向 | 剩余 app_caches + 少量 guard 预研（Batch 4 已完成，见 `docs/wukong-code/specs/2026-07-29-phase4c-rules-batch4-design.md`） |
