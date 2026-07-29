# Phase 4c Batch 2：Clean 规则扩展 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 建立 mole 规则库存工具，并完成本期 Batch 2——净增约 30–50 条以 `all` / `keep_newest_*` 为主的 clean 规则，配套 fixture 与 `verify-clean-candidates` 门禁。

**Architecture:** 不改 crate 边界。候选从 `third_party/mole-1.48.1/lib/clean/*.sh` 库存化；规则写入 `data/rules/*.toml`，经现有 `rules::load` → `Orchestrator::build_plan`；表驱动断言走 `tests/fixtures/clean/` + `clean_fixture::verify_clean_fixtures`。本批 **新增 0 条 custom**。

**Tech Stack:** Python 3（库存/抽取）、TOML 规则数据、Rust 既有引擎、bash 验证脚本。

**参照：** `docs/wukong-code/specs/2026-07-29-phase4c-rules-batch2-design.md`（已确认）；父设计 §6 / §7 / Phase 4c；spike `docs/findings/2026-07-spike-rule-throughput.md`。

## Global Constraints

- 许可证：GPL-3.0-only。
- 仅 macOS；不移植需 sudo 的 system 规则。
- 不改 NDJSON / `docs/protocol.md`（FROZEN）。
- Batch 2 净增规则 ∈ **[30, 50]**；不足 30 须 findings 说明，禁止灌水。
- 本批 **新增 custom = 0**；全库 custom ≤ 启用规则 5%。
- 每条关键规则至少 1 个 fixture 正向或负向覆盖。
- 提交粒度：每 Task 至少一次 commit；分支 `phase4c-rules-batch2`（已有）。

---

## File Structure

```
data/rules/
  app-caches.toml                 # Batch 2 主文件（新建）
  user-devtools.toml              # 可选：npm/brew/jetbrains 等
docs/findings/
  2026-07-phase4c-batch2-selection.md   # 选批清单 + 计数
scripts/
  inventory-mole-rules.py         # 新建
  extract-clean-fixtures.py       # 扩 allowlist
tests/fixtures/clean/             # 新增若干 JSON
```

---

## Task 1: mole 规则库存脚本

**Files:**
- Create: `scripts/inventory-mole-rules.py`
- Create: `docs/findings/2026-07-phase4c-batch2-inventory-sample.json`（脚本对当前 tree 跑一次的截断样例，或写入 findings 说明如何生成）

**Interfaces:**
- Produces: CLI `python3 scripts/inventory-mole-rules.py [--json PATH] [--csv PATH]`  
  每行/对象字段：`source_file`, `line`, `label`, `path_expr`, `complexity_guess` ∈ {`all`,`mtime`,`guard`,`custom`,`sudo`,`unknown`}, `ported` bool（对照 `data/rules/**/*.toml` 的 `id` / path 启发式）

**Steps:**

- [x] **Step 1: 写最小可运行脚本骨架** — 扫描 `third_party/mole-1.48.1/lib/clean/*.sh`，用正则匹配：

```python
# 匹配: safe_clean <path> "label"
SAFE_CLEAN_RE = re.compile(
    r'''safe_clean\s+((?:\\.|[^\s"'])+|"(?:\\.|[^"])*"|'(?:\\.|[^'])*')\s+"([^"]*)"'''
)
```

对含 `sudo` 的行标记 `complexity_guess=sudo`；含 `keep`/`mtime`/版本保留逻辑的邻近上下文标 `mtime`；否则默认 `all`。

- [x] **Step 2: 加载已移植 id** — 解析 `data/rules/*.toml` 中所有 `id = "..."`；输出里 `ported=true` 若 label 规范化后已存在近似 id（或仅列出未匹配 path，人工选批）。

- [x] **Step 3: 本地跑通**

```bash
python3 scripts/inventory-mole-rules.py --json /tmp/mole-rules.json
python3 -c 'import json; d=json.load(open("/tmp/mole-rules.json")); print(len(d), sum(1 for x in d if x["complexity_guess"]=="all"))'
```

Expected: 条目数 ≫ 已移植数；`app_caches.sh` 中 Xcode/VS Code 行出现。

- [x] **Step 4: Commit**

```bash
git add scripts/inventory-mole-rules.py
git commit -m "$(cat <<'EOF'
feat(scripts): add mole clean-rule inventory helper

EOF
)"
```

---

## Task 2: 冻结 Batch 2 选批清单

**Files:**
- Create: `docs/findings/2026-07-phase4c-batch2-selection.md`

**内容必须包含：**

1. 从库存中勾选的 **目标 30–50** 条表：`proposed_id`, `mole_label`, `path`, `strategy`, `toml_file`, `notes`
2. 优先块 A（纯 `all`，来自 `app_caches.sh`）：至少覆盖 spike 1–8 等价项 + VS Code/Zed/通讯类缓存等，凑够 ~25–35 条 `all`
3. 优先块 B（`keep_newest_by_mtime` / `older_than_days`）：~5–15 条（JetBrains 旧扩展、npm logs、brew 等——**仅当可无 custom 表达**）
4. 明确排除：`sudo`、需新 custom、本批不做的 symlink/custom

**示例行（实施时按库存改真实 path）：**

| proposed_id | label | path | strategy |
|---|---|---|---|
| xcode-cache | Xcode cache | `~/Library/Caches/com.apple.dt.Xcode/*` | all |
| vscode-logs | VS Code logs | `~/Library/Application Support/Code/logs/*` | all |
| simulator-cache | Simulator cache | `~/Library/Developer/CoreSimulator/Caches/*` | all |

- [x] **Step 1: 跑库存，人工勾选填表**（目标计数写在文首：`Target: N rules`）

- [x] **Step 2: Commit**

```bash
git add docs/findings/2026-07-phase4c-batch2-selection.md
git commit -m "$(cat <<'EOF'
docs(findings): freeze Phase 4c Batch 2 rule selection list

EOF
)"
```

---

## Task 3: 写入第一波 `app-caches.toml`（~15–20 条 `all`）

**Files:**
- Create: `data/rules/app-caches.toml`
- Test: 扩展或手写 `tests/fixtures/clean/` 至少 **3** 个 JSON（覆盖 Xcode cache、VS Code logs、Simulator cache）

**规则模板（每条）：**

```toml
[[rule]]
id = "xcode-cache"
category = "app-caches"
label = "Xcode cache"
platform = ["macos"]
paths = ["~/Library/Caches/com.apple.dt.Xcode/*"]
impact = "Xcode 将按需重建缓存"
disabled = false
last_verified = "2026-07"

[rule.strategy]
kind = "all"
```

**Fixture 最小形状**（与现有一致）：

```json
{
  "id": "xcode_cache_selects_cache_child",
  "source_bats": "manual",
  "source_test": "xcode-cache selects cache child",
  "fixture": [
    { "mkdir": "~/Library/Caches/com.apple.dt.Xcode/Cache.db" }
  ],
  "expect_selected": [
    "~/Library/Caches/com.apple.dt.Xcode/Cache.db|Xcode cache"
  ],
  "expect_not_selected": []
}
```

**注意：** `mkdir`/`write` 路径语义以 `crates/vole-core/src/clean_fixture.rs` 为准；label 必须与 TOML `label` **逐字一致**。

**Steps:**

- [x] **Step 1: 写 1 个失败 fixture + 空/缺规则** — 先提交 fixture，确认 `cargo test -p vole-core verify_clean_fixtures` **FAIL**

- [x] **Step 2: 写入对应 TOML 规则使该 fixture 绿**

- [x] **Step 3: 按选批清单追加至 ~15–20 条 `all`，每增加一组就跑：**

```bash
cargo test -p vole-core verify_clean_fixtures -- --nocapture
```

Expected: PASS

- [x] **Step 4: Commit**

```bash
git add data/rules/app-caches.toml tests/fixtures/clean/
git commit -m "$(cat <<'EOF'
feat(rules): add Batch 2 wave-1 app-caches all-strategy rules

EOF
)"
```

---

## Task 4: 第二波纯路径规则（凑够本批 `all` 主体）

**Files:**
- Modify: `data/rules/app-caches.toml`（或拆 `data/rules/app-caches-chat.toml` 若单文件过大）
- Create: 额外 fixture（通讯/编辑器缓存各至少 1 个代表性）

**Steps:**

- [x] **Step 1: 按选批清单追加剩余 `all` 规则**（Discord/Slack/Zoom/WeChat 等 mole 已有 `safe_clean` 且无 sudo）

- [x] **Step 2: fixture + `verify_clean_fixtures` 绿**

- [x] **Step 3: 计数检查**

```bash
rg -c '^\[\[rule\]\]' data/rules/*.toml
```

记录总数与 Batch 2 增量（相对 `main` 上 ai-agents/codex/example）。

- [x] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(rules): add Batch 2 wave-2 app cache all-strategy rules

EOF
)"
```

---

## Task 5: `keep_newest_by_mtime` / `older_than_days` 子集

**Files:**
- Create or modify: `data/rules/user-devtools.toml`
- Create: fixture 覆盖「保留最新 N 个、删更旧」

**示例（npm logs — 以 mole 实际 path 为准）：**

```toml
[[rule]]
id = "npm-logs-keep-newest"
category = "user-devtools"
label = "npm logs"
platform = ["macos"]
paths = ["~/.npm/_logs/*"]
last_verified = "2026-07"

[rule.strategy]
kind = "keep_newest_by_mtime"
keep = 5
```

**Fixture：** 创建 6 个不同 `mtime` 的文件，`expect_selected` 仅最旧的那批（与策略一致）。

**Steps:**

- [x] **Step 1: 红 fixture → 绿 TOML**（TDD）

- [x] **Step 2: 确认无新增 `kind = "custom"`**

```bash
rg 'kind = "custom"' data/rules/
```

Expected: 仅既有 ai-agents 等，本批文件无新增。

- [x] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(rules): add Batch 2 keep_newest / older_than_days rules

EOF
)"
```

---

## Task 6: 扩展 bats 抽取 allowlist（可选增强）

**Files:**
- Modify: `scripts/extract-clean-fixtures.py`（allowlist 增加可抽取的 `clean_*.bats`）
- Modify: `scripts/extract-clean-fixtures.md`

**Steps:**

- [x] **Step 1: 对候选 bats 试跑抽取，人工校对后纳入 `tests/fixtures/clean/`**

```bash
python3 scripts/extract-clean-fixtures.py --bats clean_dev_caches.bats
```

- [x] **Step 2: `verify_clean_fixtures` 仍绿；失败则修正 TOML label/path 或丢掉坏 fixture**

- [x] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
test(fixtures): expand clean fixture coverage for Batch 2

EOF
)"
```

若无可靠 bats 可抽：本 Task 可改为「手写 fixture 补到每条关键规则 ≥1」，并在 findings 注明。

---

## Task 7: 门禁、README、选批闭环

**Files:**
- Modify: `README.md`（Phase 4/规则覆盖量：Batch 2 后约 N 条）
- Modify: `docs/findings/2026-07-phase4c-batch2-selection.md`（勾选 Actual count）
- Modify: `scripts/verify-clean-candidates.sh`（删除过时「Task 12 未接线」类文案，若仍存在）
- Create if needed: `docs/findings/2026-07-phase4c-batch2.md`（仅当计数 <30 或止损）

**Steps:**

- [x] **Step 1: 全量验证**

```bash
cargo test -p vole-core
bash scripts/verify-clean-candidates.sh
cargo clippy -p vole-core -- -D warnings
```

Expected: 全绿（无 `VOLE_TEST_ROOT` 时双跑 SKIP 可接受）。

- [x] **Step 2: 计数门禁** — Batch 2 净增 ∈ [30,50] 或 findings 下调说明。

- [x] **Step 3: 更新本计划所有 Task checkbox 为 `[x]`**

- [x] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
docs: close Phase 4c Batch 2 verification and coverage notes

EOF
)"
```

---

## Task 8:（可选）`VOLE_TEST_ROOT` 双跑抽检

**Steps:**

- [x] 在一次性目录导出 `VOLE_TEST_ROOT`，对 3–5 条高信心规则跑 mole dry-run vs `vole clean --plan`（按 `verify-clean-candidates.sh` 后半段）
- [x] 保护相关分歧 → **停**并修；仅标签差异记 findings
- [x] Commit findings only if new

---

## Spec coverage（self-review）

| Spec 要求 | Task |
|---|---|
| 库存脚本 | T1 |
| 选批 30–50、分类优先 | T2–T5 |
| `app-caches.toml` / fixtures | T3–T4 |
| keep_newest 子集、0 custom | T5 |
| extract allowlist | T6 |
| 门禁 / README / 止损 findings | T7–T8 |
| 不改协议 / 不提权 | Global Constraints |

---

## Execution Handoff

Plan complete and saved to `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch2.md`. Two execution options:

**1. Subagent-Driven (recommended)** — 每 Task 新 subagent + 两阶段 review  

**2. Inline Execution** — 本会话按 Task 执行（executing-plans）

Which approach?
