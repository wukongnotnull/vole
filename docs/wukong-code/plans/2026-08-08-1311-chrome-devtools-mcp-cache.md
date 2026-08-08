# Chrome DevTools MCP Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付 1 条窄 `all` 规则 `chrome-devtools-mcp-cache`，对齐 Mole `dev.sh` Chrome DevTools MCP Chromium profile 主 Cache；发版 **1.40.0**。

**Architecture:** 纯 TOML `strategy.kind = "all"`；开 MCP 家族首条规则（路径 `~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/*`）。无新 handler、无 PrivilegeBackend、无 guards。保护层预期零改（路径含 `/Cache/`）。

**Tech Stack:** Rust / TOML rules / clean fixtures / vole-core tests

## Global Constraints

- 版本：**1.40.0**；规则 **536 → 537**；**不 bump** `schema_version`；若合入前 main 已被抬到 ≥1.40，则顺延下一 MINOR
- 只交付 **1** 条规则；**禁止** `user.sh` 广域、盲增 custom
- 不改 W2a / W2b 核心（除 `coverage.rs` + Cargo/发版窄改）；撞车则 rebase
- 全程中文；task-level commit；合并 `gh pr merge --merge`
- Design：`docs/wukong-code/specs/2026-08-08-1310-chrome-devtools-mcp-cache-design.md`

## File map

| 文件 | 职责 |
|---|---|
| `data/rules/user-devtools.toml` | 新增 `chrome-devtools-mcp-cache`（紧挨 `antigravity-browser-cache` 之后，独立 MCP 家族） |
| `tests/fixtures/clean/w2c_chrome_devtools_mcp_cache_selects_child.json` | fixture |
| `crates/vole-core/src/protection/path.rs` | **仅当 RED 证明需要** 时补豁免（预期不改） |
| `crates/vole-core/src/ops/coverage.rs` | 「已落地」追加 Chrome DevTools MCP Cache |
| `Cargo.toml` | workspace version → 1.40.0 |
| `Formula/vole.rb` / `README.md` / `docs/releases/v1.40.0.md` / `docs/findings/2026-08-chrome-devtools-mcp-cache.md` | 发版对齐 |

---

## Task 1: TOML 规则 + fixture（RED→GREEN）

**Files:**
- Modify: `data/rules/user-devtools.toml`（在 `antigravity-browser-cache` 块之后插入）
- Create: `tests/fixtures/clean/w2c_chrome_devtools_mcp_cache_selects_child.json`
- Modify (条件): `crates/vole-core/src/protection/path.rs`

**Interfaces:**
- Produces: 启用规则 id `chrome-devtools-mcp-cache`；fixture id `w2c_chrome_devtools_mcp_cache_selects_child`

- [ ] **Step 1: RED fixture**

写入 `tests/fixtures/clean/w2c_chrome_devtools_mcp_cache_selects_child.json`：

```json
{
  "id": "w2c_chrome_devtools_mcp_cache_selects_child",
  "source_bats": "manual",
  "source_test": "chrome-devtools-mcp-cache selects glob child",
  "fixture": [
    {
      "mkdir": "~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/item"
    }
  ],
  "expect_selected": [
    "~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/item|Chrome DevTools MCP browser cache"
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

在 `data/rules/user-devtools.toml` 的 `antigravity-browser-cache` 块之后、`antigravity-cache` 之前插入：

```toml
[[rule]]
id = "chrome-devtools-mcp-cache"
category = "user-devtools"
label = "Chrome DevTools MCP browser cache"
platform = ["macos"]
paths = ["~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/*"]
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

Expected: PASS。若因保护拦下则最小补豁免；**禁止**向 `CACHE_SEGMENTS` 盲加无关段。

- [ ] **Step 5: Commit**

```bash
git add data/rules/user-devtools.toml \
  tests/fixtures/clean/w2c_chrome_devtools_mcp_cache_selects_child.json
# 若改了 protection: git add crates/vole-core/src/protection/path.rs
git commit -m "$(cat <<'EOF'
feat(clean): add chrome-devtools-mcp-cache all rule

EOF
)"
```

---

## Task 2: coverage + 1.40.0 发版对齐

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs` — 在「Antigravity browser Cache、」后追加 `Chrome DevTools MCP Cache、`
- Modify: `Cargo.toml` — `version = "1.40.0"`
- Modify: `Formula/vole.rb` — version / url 中的版本串 → `1.40.0`
- Modify: `README.md` — 规则数 **537**；成熟度行点名本规则 / 1.40.0（沿现网句式）
- Create: `docs/releases/v1.40.0.md`
- Create: `docs/findings/2026-08-chrome-devtools-mcp-cache.md`

**Interfaces:**
- Produces: `coverage_note(_)` 含「Chrome DevTools MCP Cache」；workspace 版本 1.40.0

- [ ] **Step 1: RED** — 在 `coverage.rs` 测试模块加断言：

```rust
#[test]
fn coverage_note_mentions_chrome_devtools_mcp_cache() {
    let note = coverage_note(537);
    assert!(note.contains("Chrome DevTools MCP Cache"));
    assert!(note.contains("已落地"));
}
```

同时把既有 `coverage_note(536)` 抽样断言升到 `537`（若测试写死启用数）。

- [ ] **Step 2: 跑 RED**

```bash
cargo test -p vole-core coverage_note_mentions_chrome_devtools_mcp_cache -- --nocapture
```

Expected: FAIL（文案尚无该串）

- [ ] **Step 3: GREEN** — 改 `coverage_note`；bump 版本与发版文件。`docs/releases/v1.40.0.md`：

```markdown
# v1.40.0

## 新增

- 规则 `chrome-devtools-mcp-cache`：清理 `~/.cache/chrome-devtools-mcp/chrome-profile/Default/Cache/*`
  - 对齐 Mole `dev.sh` Chrome DevTools MCP Chromium profile 主 Cache
  - 纯 `all`；无 custom / 无 sudo

## 仍未移植

- 桌面 SMAppService / 特权助手

## 规则

536 → **537**
```

findings 短文：路径级差集（`~/.cache/chrome-devtools-mcp/…`）；与 Antigravity browser Cache 同构，本刀开 MCP 家族首条。

- [ ] **Step 4: 跑 GREEN**

```bash
cargo test -p vole-core coverage_note_mentions_chrome_devtools_mcp_cache -- --nocapture
cargo test -p vole-core
cargo clippy -p vole-core -- -D warnings
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/coverage.rs Cargo.toml Formula/vole.rb README.md \
  docs/releases/v1.40.0.md docs/findings/2026-08-chrome-devtools-mcp-cache.md
git commit -m "$(cat <<'EOF'
chore(release): bump 1.40.0 for chrome-devtools-mcp-cache

EOF
)"
```

---

## Task 3: PR + merge + 0119 文档小 PR

**Files:** git / gh；另分支改 `docs/wukong-code/specs/2026-08-08-0119-mole-parity-roadmap-design.md`

- [ ] **Step 1:** `git push -u origin HEAD`；`gh pr create`
- [ ] **Step 2:** 等 CI 绿；自检：仅 1 规则、无 user/custom、保护无广域泄漏
- [ ] **Step 3:** `gh pr merge <N> --merge --delete-branch`
- [ ] **Step 4:** 从最新 main 开 `docs/w2c-batch6-chrome-devtools-mcp-roadmap-complete`，0119：本刀完成；下一刀——若差集仍大可续 W2c（Antigravity profile 兄弟 / QQ Music iRRCache 等），**否则明确写「暂停 Batch6 必做；optimize 后置长尾保持 coverage；W3 不开发」**；小 PR merge commit

---

## Spec coverage self-check

| Spec 要求 | Task |
|---|---|
| 1 条 `chrome-devtools-mcp-cache` | T1 |
| 纯 all / 无 custom / 无 sudo | T1 |
| fixture + verify | T1 |
| coverage 已落地名 | T2 |
| 1.40.0 / 537 | T2 |
| PR merge commit + 0119 小 PR（含暂停 Batch6 表述） | T3 |
| 不碰 W2a/W2b 核心 | 文件表约束 |

无 TBD；无 placeholder。
