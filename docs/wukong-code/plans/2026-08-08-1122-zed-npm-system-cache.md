# Zed system-node npm cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付 1 条窄 `all` 规则 `zed-npm-system-cache`，对齐 Mole `app_caches.sh` Zed system-node npm cache；发版 **1.37.0**。

**Architecture:** 纯 TOML `strategy.kind = "all"`；紧邻既有 `zed-npm-cache`（`node-v*/cache`）。无新 handler、无 PrivilegeBackend。保护层优先零改；若 fixture 因小写 `/cache/` 被拦，再最小补 `is_explicit_clean_cache_path`（Zed `node/.../cache` 形状，禁止广加裸 `/cache/`）。

**Tech Stack:** Rust / TOML rules / clean fixtures / vole-core tests

## Global Constraints

- 版本：**1.37.0**；规则 **534 → 535**；**不 bump** `schema_version`；若合入前 main 已被抬到 ≥1.37，则顺延下一 MINOR
- 只交付 **1** 条规则；**禁止** `user.sh` 广域、盲增 custom
- 不改 W2a / W2b 核心（除 `coverage.rs` + Cargo/发版窄改）；撞车则 rebase
- 全程中文；task-level commit；合并 `gh pr merge --merge`
- Design：`docs/wukong-code/specs/2026-08-08-1121-zed-npm-system-cache-design.md`

## File map

| 文件 | 职责 |
|---|---|
| `data/rules/app-caches.toml` | 新增 `zed-npm-system-cache`（紧挨 `zed-npm-cache`） |
| `tests/fixtures/clean/w2c_zed_npm_system_cache_selects_child.json` | fixture |
| `crates/vole-core/src/protection/path.rs` | **仅当 RED 证明需要** 时补 Zed node cache 豁免 |
| `crates/vole-core/src/ops/coverage.rs` | 「已落地」追加 Zed system-node npm cache |
| `Cargo.toml` | workspace version → 1.37.0 |
| `Formula/vole.rb` / `README.md` / `docs/releases/v1.37.0.md` / `docs/findings/2026-08-zed-npm-system-cache.md` | 发版对齐 |

---

## Task 1: TOML 规则 + fixture（RED→GREEN）

**Files:**
- Modify: `data/rules/app-caches.toml`（在 `zed-npm-cache` 块之后插入）
- Create: `tests/fixtures/clean/w2c_zed_npm_system_cache_selects_child.json`
- Modify (条件): `crates/vole-core/src/protection/path.rs`

**Interfaces:**
- Produces: 启用规则 id `zed-npm-system-cache`；fixture id `w2c_zed_npm_system_cache_selects_child`

- [ ] **Step 1: RED fixture**

写入 `tests/fixtures/clean/w2c_zed_npm_system_cache_selects_child.json`：

```json
{
  "id": "w2c_zed_npm_system_cache_selects_child",
  "source_bats": "manual",
  "source_test": "zed-npm-system-cache selects glob child",
  "fixture": [
    {
      "mkdir": "~/Library/Application Support/Zed/node/cache/item"
    }
  ],
  "expect_selected": [
    "~/Library/Application Support/Zed/node/cache/item|Zed system-node npm cache"
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

在 `data/rules/app-caches.toml` 的 `zed-npm-cache` 之后插入：

```toml
[[rule]]
id = "zed-npm-system-cache"
category = "app-caches"
label = "Zed system-node npm cache"
platform = ["macos"]
paths = ["~/Library/Application Support/Zed/node/cache/*"]
impact = "Application cache/logs; safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "all"
```

- [ ] **Step 4: 跑 GREEN（保护回归）**

```bash
cargo test -p vole-core verify_clean_fixtures -- --nocapture
```

Expected: PASS。若因 `is_explicit_clean_cache_path` 不认小写 `/cache/` 而 fail：在 `path.rs` 增加窄豁免（例如路径含子串 `/Library/Application Support/Zed/node/` 且含 `/cache/`），并补一条 protection 单测；**禁止**向 `CACHE_SEGMENTS` 盲加通用 `"/cache/"`。

- [ ] **Step 5: Commit**

```bash
git add data/rules/app-caches.toml tests/fixtures/clean/w2c_zed_npm_system_cache_selects_child.json
# 若改了 protection: git add crates/vole-core/src/protection/path.rs
git commit -m "$(cat <<'EOF'
feat(clean): add zed-npm-system-cache all rule

EOF
)"
```

---

## Task 2: coverage + 1.37.0 发版对齐

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs` — 在「Filo production Cache、」后追加 `Zed system-node npm cache、`
- Modify: `Cargo.toml` — `version = "1.37.0"`
- Modify: `Formula/vole.rb` — version / url 中的 `1.35.0` → `1.37.0`
- Modify: `README.md` — 规则数 **535**；成熟度行点名本规则 / 1.37.0（沿现网句式）
- Create: `docs/releases/v1.37.0.md`
- Create: `docs/findings/2026-08-zed-npm-system-cache.md`

**Interfaces:**
- Produces: `coverage_note(_)` 含「Zed system-node npm cache」；workspace 版本 1.37.0

- [ ] **Step 1: RED** — 在 `coverage.rs` 测试模块加断言：

```rust
#[test]
fn coverage_note_mentions_zed_system_node_npm_cache() {
    let note = coverage_note(535);
    assert!(note.contains("Zed system-node npm cache"));
    assert!(note.contains("已落地"));
}
```

- [ ] **Step 2: 跑 RED**

```bash
cargo test -p vole-core coverage_note_mentions_zed_system_node_npm_cache -- --nocapture
```

Expected: FAIL（文案尚无该串）

- [ ] **Step 3: GREEN** — 改 `coverage_note`；bump 版本与发版文件。`docs/releases/v1.37.0.md`：

```markdown
# v1.37.0

## 新增

- 规则 `zed-npm-system-cache`：清理 `~/Library/Application Support/Zed/node/cache/*`
  - 对齐 Mole `app_caches.sh` Zed system-node npm cache
  - 纯 `all`；无 custom / 无 sudo

## 仍未移植

- 桌面 SMAppService / 特权助手

## 规则

534 → **535**
```

findings 短文：Filo 首刀并列候选现落地；同 label 伪已移植 → 路径级差集。

- [ ] **Step 4: 跑 GREEN**

```bash
cargo test -p vole-core coverage_note_mentions_zed_system_node_npm_cache -- --nocapture
cargo test -p vole-core
cargo clippy -p vole-core -- -D warnings
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/coverage.rs Cargo.toml Formula/vole.rb README.md \
  docs/releases/v1.37.0.md docs/findings/2026-08-zed-npm-system-cache.md
git commit -m "$(cat <<'EOF'
chore(release): bump 1.37.0 for zed-npm-system-cache

EOF
)"
```

---

## Task 3: PR + merge + 0119 文档小 PR

**Files:** git / gh；另分支改 `docs/wukong-code/specs/2026-08-08-0119-mole-parity-roadmap-design.md` 一句标 W2c 续刀完成

- [ ] **Step 1:** `git push -u origin HEAD`；`gh pr create`
- [ ] **Step 2:** 等 CI 绿；自检：仅 1 规则、无 user/custom、保护无广域泄漏
- [ ] **Step 3:** `gh pr merge <N> --merge --delete-branch`
- [ ] **Step 4:** 从最新 main 开 `docs/w2c-batch6-zed-roadmap-complete`，0119 追加续刀完成句；若与 W2b 文档 PR 冲突则 rebase；小 PR merge commit

---

## Spec coverage self-check

| Spec 要求 | Task |
|---|---|
| 1 条 `zed-npm-system-cache` | T1 |
| 纯 all / 无 custom / 无 sudo | T1 |
| fixture + verify | T1 |
| coverage 已落地名 | T2 |
| 1.37.0 / 535 | T2 |
| PR merge commit + 0119 小 PR | T3 |
| 不碰 W2a/W2b 核心 | 文件表约束 |

无 TBD；无 placeholder。
