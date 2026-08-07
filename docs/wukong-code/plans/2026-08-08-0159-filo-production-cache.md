# Filo production Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付 1 条窄 `all` 规则 `filo-production-cache`，对齐 Mole `dev.sh` Filo Electron 主 Chromium Cache；发版 **1.32.0**。

**Architecture:** 纯 TOML `strategy.kind = "all"`；既有 plan/apply + `is_explicit_clean_cache_path` 的 `"/Cache/"` 段已足够。无新 handler、无 PrivilegeBackend、无 protection 改动。

**Tech Stack:** Rust / TOML rules / clean fixtures / vole-core tests

## Global Constraints

- 版本：**1.32.0**；规则 **533 → 534**；**不 bump** `schema_version`
- 只交付 **1** 条规则；**禁止** `user.sh` 广域、盲增 custom
- 不改 W1 / W2a / W2b 核心（除 `coverage.rs` + Cargo/发版窄改）
- 全程中文；task-level commit；合并 `gh pr merge --merge`
- Design：`docs/wukong-code/specs/2026-08-08-0158-filo-production-cache-design.md`

## File map

| 文件 | 职责 |
|---|---|
| `data/rules/user-devtools.toml` | 新增 `filo-production-cache` |
| `tests/fixtures/clean/w2c_filo_production_cache_selects_child.json` | fixture |
| `crates/vole-core/src/ops/coverage.rs` | 「已落地」追加 Filo production Cache |
| `Cargo.toml` | workspace version → 1.32.0 |
| `Formula/vole.rb` / `README.md` / `docs/releases/v1.32.0.md` / `docs/findings/2026-08-filo-production-cache.md` | 发版对齐 |

---

## Task 1: TOML 规则 + fixture（RED→GREEN）

**Files:**
- Modify: `data/rules/user-devtools.toml`（紧挨现有 `filo-code-cache` 块之前或之后插入）
- Create: `tests/fixtures/clean/w2c_filo_production_cache_selects_child.json`

**Interfaces:**
- Produces: 启用规则 id `filo-production-cache`；fixture id `w2c_filo_production_cache_selects_child`

- [ ] **Step 1: RED fixture** — 先加 fixture，再确认规则缺失时行为（或先写规则再验绿；本仓库表驱动：无规则则 `expect_selected` 失败）

写入 `tests/fixtures/clean/w2c_filo_production_cache_selects_child.json`：

```json
{
  "id": "w2c_filo_production_cache_selects_child",
  "source_bats": "manual",
  "source_test": "filo-production-cache selects glob child",
  "fixture": [
    {
      "mkdir": "~/Library/Application Support/Filo/production/Cache/item"
    }
  ],
  "expect_selected": [
    "~/Library/Application Support/Filo/production/Cache/item|Filo production cache"
  ],
  "expect_not_selected": [
    "~/Library/Application Support/Filo/production/Code Cache/item"
  ]
}
```

说明：`expect_not_selected` 中的 Code Cache 路径**不必** mkdir（断言「未选入」即可）；若 harness 要求路径存在可改为只保留 `expect_selected`。以 `batch9_filo_gpu_cache_selects_child.json` 为模板时，可省略 `expect_not_selected` 内未物化路径——**采用与 batch9 相同的极简形**（仅 `expect_selected`）若极简更稳。

极简正本：

```json
{
  "id": "w2c_filo_production_cache_selects_child",
  "source_bats": "manual",
  "source_test": "filo-production-cache selects glob child",
  "fixture": [
    {
      "mkdir": "~/Library/Application Support/Filo/production/Cache/item"
    }
  ],
  "expect_selected": [
    "~/Library/Application Support/Filo/production/Cache/item|Filo production cache"
  ],
  "expect_not_selected": []
}
```

- [ ] **Step 2: 跑 RED**

```bash
cargo test -p vole-core verify_clean_fixtures -- --nocapture
```

Expected: FAIL（无匹配规则 / expect_selected 未命中）

- [ ] **Step 3: GREEN — 追加 TOML**

在 `data/rules/user-devtools.toml` 邻近 `filo-code-cache` 插入：

```toml
[[rule]]
id = "filo-production-cache"
category = "user-devtools"
label = "Filo production cache"
platform = ["macos"]
paths = ["~/Library/Application Support/Filo/production/Cache/*"]
impact = "Application cache/logs; safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "all"
```

- [ ] **Step 4: 跑 GREEN**

```bash
cargo test -p vole-core verify_clean_fixtures -- --nocapture
```

Expected: PASS（含新 fixture）

- [ ] **Step 5: Commit**

```bash
git add data/rules/user-devtools.toml tests/fixtures/clean/w2c_filo_production_cache_selects_child.json
git commit -m "$(cat <<'EOF'
feat(clean): add filo-production-cache all rule

EOF
)"
```

---

## Task 2: coverage + 1.32.0 发版对齐

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs` — 在「已落地」枚举末尾 `Time Machine 失败中备份…` 一带追加 `Filo production Cache、`（或独立短句，保持与现网句式一致）
- Modify: `Cargo.toml` — `version = "1.32.0"`
- Modify: `Formula/vole.rb` — version / url 中的 `1.28.0` → `1.32.0`（按现网 Homebrew sync 惯例）
- Modify: `README.md` — 成熟度行改为 **1.32.0** 并点名 Filo production Cache
- Create: `docs/releases/v1.32.0.md`
- Create: `docs/findings/2026-08-filo-production-cache.md`

**Interfaces:**
- Produces: `coverage_note(_)` 含「Filo production Cache」；workspace 版本 1.32.0

- [ ] **Step 1: RED** — 在 `coverage.rs` 测试模块加断言：

```rust
#[test]
fn coverage_note_mentions_filo_production_cache() {
    let note = coverage_note(534);
    assert!(note.contains("Filo production Cache"));
    assert!(note.contains("已落地"));
}
```

- [ ] **Step 2: 跑 RED**

```bash
cargo test -p vole-core coverage_note_mentions_filo_production_cache -- --nocapture
```

Expected: FAIL（文案尚无该串）

- [ ] **Step 3: GREEN** — 改 `coverage_note` 字符串；bump 版本与发版文件。`docs/releases/v1.32.0.md`：

```markdown
# v1.32.0

## 新增

- 规则 `filo-production-cache`：清理 `~/Library/Application Support/Filo/production/Cache/*`
  - 对齐 Mole `dev.sh` Filo Electron 主 Chromium Cache
  - 纯 `all`；无 custom / 无 sudo

## 仍未移植

- 本地快照报告、桌面 SMAppService / 特权助手

## 规则

533 → **534**
```

findings 短文写清：inventory 按 label 伪已移植 → 路径级差集选中本条。

- [ ] **Step 4: 跑 GREEN**

```bash
cargo test -p vole-core coverage_note_mentions_filo_production_cache -- --nocapture
cargo test -p vole-core
cargo clippy -p vole-core -- -D warnings
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/coverage.rs Cargo.toml Formula/vole.rb README.md \
  docs/releases/v1.32.0.md docs/findings/2026-08-filo-production-cache.md
git commit -m "$(cat <<'EOF'
chore(release): bump 1.32.0 for filo-production-cache

EOF
)"
```

---

## Task 3: PR + merge

**Files:** 无代码；git / gh

- [ ] **Step 1:** `git push -u origin HEAD`
- [ ] **Step 2:** `gh pr create`（Summary / Test plan）
- [ ] **Step 3:** 等 CI 绿；inline code review（自检：仅 1 规则、无 user/custom、保护零改）
- [ ] **Step 4:** `gh pr merge <N> --merge --delete-branch`（条件允许时）

---

## Spec coverage self-check

| Spec 要求 | Task |
|---|---|
| 1 条 `filo-production-cache` | T1 |
| 纯 all / 无 custom / 无 sudo | T1 |
| fixture + verify | T1 |
| coverage 已落地名 | T2 |
| 1.32.0 / 534 | T2 |
| PR merge commit | T3 |
| 不碰 W1/W2a/W2b 核心 | 文件表约束 |

无 TBD；无 placeholder。
