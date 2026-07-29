# Phase 4c Batch 3：Clean 规则扩展 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 在 Batch 2（46 条）基础上净增约 **40** 条以 `all` 为主的 clean 规则，累计 ≈ **86** 条；压低 custom 占比至 ≤5%；配套 fixture 与门禁。

**Architecture:** 不改 crate 边界。候选自 `inventory-mole-rules.py` 差集；规则写入 `data/rules/app-caches.toml` + `user-devtools.toml`；表驱动断言走 `tests/fixtures/clean/` + `verify_clean_fixtures`。本批 **新增 0 条 custom**。

**Tech Stack:** Python 3（库存）、TOML、Rust 既有引擎、bash 验证脚本。

**参照：** `docs/wukong-code/specs/2026-07-29-phase4c-rules-batch3-design.md`；Batch 2 计划 `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch2.md`。

## Global Constraints

- 许可证：GPL-3.0-only。
- 仅 macOS；不移植 sudo / 广域 `user.sh` sweep。
- 不改 NDJSON / `docs/protocol.md`（FROZEN）。
- Batch 3 净增 ∈ **[30, 50]**；目标 **40**。
- 本批新增 custom = 0；合入后全库 custom ≤ 5%。
- 每条关键规则 ≥1 fixture。
- 分支：`phase4c-rules-batch3`（自 `main` 创建）。

---

## File Structure

```
data/rules/
  app-caches.toml                 # 追加 Block A
  user-devtools.toml              # 追加 Block B
docs/findings/
  2026-07-phase4c-batch3-selection.md
tests/fixtures/clean/             # batch3_* JSON
scripts/
  extract-clean-fixtures.py       # 扩 allowlist
  verify-clean-candidates.sh      # 沿用
```

---

## Task 1: 基线与库存刷新

**Files:**
- Modify: `docs/findings/2026-07-phase4c-batch3-selection.md`（文首基线计数）
- Use: `scripts/inventory-mole-rules.py`

**Steps:**

- [x] **Step 1: 确认 main 基线**

```bash
rg -c '^\[\[rule\]\]' data/rules/*.toml
python3 scripts/inventory-mole-rules.py --json /tmp/mole-rules-b3.json
python3 -c 'import json; d=json.load(open("/tmp/mole-rules-b3.json")); print("ported", sum(1 for x in d if x["ported"]), "unported_all", sum(1 for x in d if not x["ported"] and x["complexity_guess"]=="all"))'
```

Expected: ported=40, unported_all≈416, total enabled rules≈46.

- [x] **Step 2: 记录 custom 占比**

```bash
rg 'kind = "custom"' data/rules/
```

Expected: 3 处（ai-agents×2, codex×1）；本批不得新增。

- [x] **Step 3: Commit**（若仅 findings 基线段）

```bash
git add docs/findings/2026-07-phase4c-batch3-selection.md
git commit -m "$(cat <<'EOF'
docs(findings): add Phase 4c Batch 3 baseline counts

EOF
)"
```

---

## Task 2: 冻结 Batch 3 选批清单

**Files:**
- Create: `docs/findings/2026-07-phase4c-batch3-selection.md`

**内容必须包含：**

1. 文首 `Target: 40 rules` / `Actual: TBD`
2. Block A / B 表：`proposed_id`, `mole_label`, `path`, `strategy`, `toml_file`, `notes`
3. 排除项说明（user.sh 广域、guard、custom）

**建议选批（实施时可微调 ±2，总数仍 ∈ [30,50]）：**

### Block A — `app-caches.toml`（18 × `all`）

| proposed_id | label | path |
|---|---|---|
| whatsapp-cache | WhatsApp cache | `~/Library/Caches/net.whatsapp.WhatsApp/*` |
| skype-cache | Skype cache | `~/Library/Caches/com.skype.skype/*` |
| tencent-meeting-cache | Tencent Meeting cache | `~/Library/Caches/com.tencent.meeting/*` |
| wecom-cache | WeCom cache | `~/Library/Caches/com.tencent.WeWorkMac/*` |
| qq-cache | QQ cache | `~/Library/Caches/com.tencent.qq/*` |
| feishu-cache | Feishu cache | `~/Library/Caches/com.feishu.*/*` |
| teams-legacy-cache | Microsoft Teams legacy cache | `~/Library/Application Support/Microsoft/Teams/Cache/*` |
| teams-legacy-logs | Microsoft Teams legacy logs | `~/Library/Application Support/Microsoft/Teams/logs/*` |
| teams-legacy-tmp | Microsoft Teams legacy temp files | `~/Library/Application Support/Microsoft/Teams/tmp/*` |
| dingtalk-cache | DingTalk iDingTalk cache | `~/Library/Caches/dd.work.exclusive4aliding/*` |
| dingtalk-logs | DingTalk logs | `~/Library/Application Support/iDingTalk/log/*` |
| chatgpt-cache | ChatGPT cache | `~/Library/Caches/com.openai.chat/*` |
| claude-desktop-cache | Claude desktop cache | `~/Library/Caches/com.anthropic.claudefordesktop/*` |
| claude-logs | Claude logs | `~/Library/Logs/Claude/*` |
| lm-studio-cache | LM Studio cache | `~/Library/Caches/com.lmstudio.lmstudio/*` |
| sketch-cache | Sketch cache | `~/Library/Caches/com.bohemiancoding.sketch3/*` |
| adobe-cache | Adobe cache | `~/Library/Caches/Adobe/*` |
| screenflow-cache | ScreenFlow cache | `~/Library/Caches/net.telestream.screenflow10/*` |

### Block B — `user-devtools.toml`（22 × `all`）

| proposed_id | label | path |
|---|---|---|
| tnpm-cacache | tnpm cache directory | `~/.tnpm/_cacache/*` |
| yarn-cache | Yarn cache | `~/.yarn/cache/*` |
| yarn-v1-cache | Yarn v1 cache | `~/Library/Caches/Yarn/*` |
| pyenv-cache | pyenv cache | `~/.pyenv/cache/*` |
| poetry-cache | Poetry cache | `~/.cache/poetry/*` |
| ruff-cache | Ruff cache | `~/.cache/ruff/*` |
| mypy-cache | MyPy cache | `~/.cache/mypy/*` |
| pytest-cache | Pytest cache | `~/.pytest_cache/*` |
| jupyter-runtime | Jupyter runtime cache | `~/.jupyter/runtime/*` |
| huggingface-cache | Hugging Face cache | `~/.cache/huggingface/*` |
| pytorch-cache | PyTorch cache | `~/.cache/torch/*` |
| tensorflow-cache | TensorFlow cache | `~/.cache/tensorflow/*` |
| wandb-cache | Weights & Biases cache | `~/.cache/wandb/*` |
| cargo-registry-cache | Rust cargo cache | `~/.cargo/registry/cache/*` |
| cargo-git-cache | Cargo git cache | `~/.cargo/git/*` |
| rustup-downloads | Rust downloads cache | `~/.rustup/downloads/*` |
| rbenv-cache | rbenv download cache | `~/.rbenv/cache/*` |
| gem-spec-cache | gem spec cache | `~/.gem/specs/*` |
| bundler-cache | Ruby Bundler cache | `~/.bundle/cache/*` |
| docker-buildx-cache | Docker BuildX cache | `~/.docker/buildx/cache/*` |
| kube-cache | Kubernetes cache | `~/.kube/cache/*` |
| cpan-build | CPAN build artifacts | `~/.cpan/build/*` |

**Steps:**

- [x] **Step 1: 跑库存核对 path/label 与 mole 一致**

```bash
python3 scripts/inventory-mole-rules.py | rg -i 'whatsapp|yarn|huggingface|adobe'
```

- [x] **Step 2: 填表并 commit**

```bash
git add docs/findings/2026-07-phase4c-batch3-selection.md
git commit -m "$(cat <<'EOF'
docs(findings): freeze Phase 4c Batch 3 rule selection list

EOF
)"
```

---

## Task 3: Block A — app-caches 第一波（~10 条 + 3 fixtures）

**Files:**
- Modify: `data/rules/app-caches.toml`
- Create: `tests/fixtures/clean/batch3_whatsapp_cache_selects_child.json`（及 2 个代表 fixture）

**规则模板：**

```toml
[[rule]]
id = "whatsapp-cache"
category = "app-caches"
label = "WhatsApp cache"
platform = ["macos"]
paths = ["~/Library/Caches/net.whatsapp.WhatsApp/*"]
impact = "App will rebuild cache on demand"
disabled = false
last_verified = "2026-07"

[rule.strategy]
kind = "all"
```

**Steps:**

- [x] **Step 1: TDD — 先写 1 个失败 fixture**
- [x] **Step 2: 写入 TOML 使 fixture 绿**
- [x] **Step 3: 追加至 ~10 条 Block A 首批（通讯类）**

```bash
cargo test -p vole-core verify_clean_fixtures -- --nocapture
```

- [x] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(rules): add Batch 3 wave-1 app-caches rules

EOF
)"
```

---

## Task 4: Block A — app-caches 第二波（凑满 18 条）

**Files:**
- Modify: `data/rules/app-caches.toml`
- Create: AI/创意类 fixture（ChatGPT、Adobe 等至少 2 个）

**Steps:**

- [x] **Step 1: 追加剩余 Block A 规则**
- [x] **Step 2: fixture + verify 绿**
- [x] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(rules): add Batch 3 wave-2 app-caches rules

EOF
)"
```

---

## Task 5: Block B — user-devtools（22 条 `all`）

**Files:**
- Modify: `data/rules/user-devtools.toml`
- Create: `batch3_yarn_cache_selects_child.json`, `batch3_huggingface_cache_selects_child.json`, `batch3_cargo_registry_cache_selects_child.json` 等

**Steps:**

- [x] **Step 1: 红 fixture → 绿 TOML（TDD，至少 3 条代表）**
- [x] **Step 2: 追加全部 Block B 规则**
- [x] **Step 3: 确认无新增 custom**

```bash
rg 'kind = "custom"' data/rules/user-devtools.toml data/rules/app-caches.toml
```

Expected: 无匹配。

- [x] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(rules): add Batch 3 user-devtools cache rules

EOF
)"
```

---

## Task 6: 扩展 bats 抽取 allowlist（可选）

**Files:**
- Modify: `scripts/extract-clean-fixtures.py`
- Modify: `scripts/extract-clean-fixtures.md`

**Steps:**

- [x] **Step 1: 试跑 `clean_dev_caches.bats` 抽取**

```bash
python3 scripts/extract-clean-fixtures.py --bats clean_dev_caches.bats
```

- [x] **Step 2: 人工校对后纳入 `tests/fixtures/clean/`**
- [x] **Step 3: `verify_clean_fixtures` 仍绿**
- [x] **Step 4: Commit**（或无可靠抽取则跳过，在 findings 注明）

---

## Task 7: 门禁、README、选批闭环

**Files:**
- Modify: `README.md`（Phase 4c Batch 3 覆盖量 ≈ 86）
- Modify: `docs/findings/2026-07-phase4c-batch3-selection.md`（Actual: 40）
- Modify: `docs/wukong-code/specs/2026-07-29-phase4c-rules-batch3-design.md`（状态 → 已确认）

**Steps:**

- [x] **Step 1: 全量验证**

```bash
cargo test -p vole-core
bash scripts/verify-clean-candidates.sh
cargo clippy -p vole-core -- -D warnings
```

- [x] **Step 2: 计数门禁**

```bash
rg -c '^\[\[rule\]\]' data/rules/*.toml
python3 scripts/inventory-mole-rules.py | head -5
```

Expected: 净增 40；ported=80；custom 占比 ≤5%。

- [x] **Step 3: 更新本计划所有 Task checkbox 为 `[x]`**
- [x] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
docs: close Phase 4c Batch 3 verification and coverage notes

EOF
)"
```

---

## Task 8:（可选）`VOLE_TEST_ROOT` 双跑抽检

**Steps:**

- [x] 对 3–5 条高信心规则（如 yarn-cache、huggingface-cache、whatsapp-cache）跑 mole dry-run vs `vole clean --plan`
- [x] 保护分歧 → 停并修；标签差异记 findings
- [x] Commit findings only if new

---

## Spec coverage（self-review）

| Spec 要求 | Task |
|---|---|
| 库存基线 | T1 |
| 选批 30–50 | T2 |
| app-caches 扩展 | T3–T4 |
| user-devtools 扩展 | T5 |
| 0 custom / 占比 ≤5% | T1, T5, T7 |
| extract allowlist | T6 |
| 门禁 / README | T7–T8 |
| 排除广域 user.sh / guard | Global Constraints |

---

## Execution Handoff

Plan complete and saved to `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch3.md`.

**1. Subagent-Driven (recommended)** — 每 Task 新 subagent + 两阶段 review  

**2. Inline Execution** — 本会话按 Task 执行（executing-plans）

Which approach?
