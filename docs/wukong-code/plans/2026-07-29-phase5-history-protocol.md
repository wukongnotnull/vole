# Phase 5：History 优先 + 协议冻结与收尾 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 先落地 mole 兼容的 `vole history`（文本 + `--json`），再正式冻结 `docs/protocol.md`，最后补齐 completion / 轻量交互入口，并把签名与 Homebrew 收尾做成可执行但不阻塞 history 的后置任务。

**Architecture:** History 是**读路径**：解析 mole 兼容的 `operations.log` / `deletions.log`（`~/Library/Logs/mole/`，与 Phase 1 oplog 写入路径一致），聚合成 session + deletion audit，经 `vole-cli` 输出。**不**改写现有 oplog 写入格式。协议冻结是文档与 CI 门禁收口，不引入新事件类型（除非 history JSON 需要独立文档小节）。交互菜单与签名/Homebrew 延后到 history + protocol 之后。

**Tech Stack:** Rust 1.97.1、`clap`、`serde`/`serde_json`、现有 `vole-core::oplog` / `DeletionLogger`、mole `tests/history.bats` 作契约参照。

**参照：**
- 设计：`docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §8 Phase 5、§5.6（协议冻结时点 = Phase 4 结束）
- Mole：`third_party/mole-1.48.1/bin/history.sh`、`lib/core/history.sh`、`tests/history.bats`
- 已有写入：`crates/vole-core/src/oplog.rs`、`scripts/verify-oplog-mole.sh`
- Phase 4：`docs/wukong-code/plans/2026-07-29-phase4-clean.md`（已完成）

## Global Constraints

- 许可证：**GPL-3.0-only**。
- 平台：仅 macOS；非 macOS `compile_error!`。
- `unsafe` 只在 `vole-sys`；其余 `#![forbid(unsafe_code)]`。
- 依赖单向：`vole-cli` → `vole-core` → `vole-sys` → `vole-proto`。
- 不引入 `tokio`。
- History **默认读 mole 路径**（`~/Library/Logs/mole/operations.log` + `deletions.log`），与 mole `history_*_log_file` 一致；测试用 temp HOME / env 覆盖。
- `--limit`：整数，夹紧到 **1..=200**，默认 **20**（对齐 mole `MOLE_HISTORY_DEFAULT_LIMIT` / `MOLE_HISTORY_MAX_LIMIT`）。
- JSON 字段名与 mole `history_render_json` **逐字段对齐**（见 Task 1 契约）。
- 本阶段**不**大规模补 clean 规则；规则扩展另开计划。
- 提交粒度：每个 Task 至少一次提交；完成后 `git commit`（用户惯例：每任务完成即提交）。

---

## File Structure

Phase 5 结束时的增量形态（在 Phase 4 之上）：

```
vole/
├── docs/
│   ├── protocol.md                          # 标注 FROZEN + history 附录（若需要）
│   └── findings/
│       └── 2026-07-phase5-signing-deferred.md   # 或更新既有 TCC/signing 文档
├── crates/
│   ├── vole-core/src/
│   │   └── history/
│   │       ├── mod.rs                       # 公共 API：load + render
│   │       ├── parse.rs                     # operations / deletions 解析
│   │       ├── session.rs                   # session 聚合（对齐 mole history_load_sessions）
│   │       └── json.rs                      # HistoryJson 结构体
│   └── vole-cli/src/
│       ├── main.rs                          # 注册 history / （后续）interactive
│       └── history_cmd.rs                   # clap + 调用 core
├── scripts/
│   ├── verify-history-mole.sh               # vole history --json vs mole fixture 对照
│   └── check-protocol-frozen.sh             # 可选：冻结标记门禁
└── docs/wukong-code/plans/
    └── 2026-07-29-phase5-history-protocol.md # 本文件
```

---

## Task 1: History 契约测试与 JSON 骨架（TDD 红）

**Files:**
- Create: `crates/vole-core/src/history/mod.rs`
- Create: `crates/vole-core/src/history/json.rs`
- Create: `crates/vole-core/tests/history_json_contract.rs`（或 `history/` 内 `#[cfg(test)]`）
- Modify: `crates/vole-core/src/lib.rs`（`pub mod history`）

**Mole JSON 契约（必须对齐）：**

```json
{
  "logs": { "operations": "<path>", "deletions": "<path>" },
  "limit": 20,
  "sessions": [
    {
      "command": "clean",
      "started_at": "...",
      "ended_at": "...",
      "items": 0,
      "size": "0B",
      "operation_count": 0,
      "actions": {
        "removed": 0, "trashed": 0, "skipped": 0,
        "failed": 0, "rebuilt": 0, "other": 0
      }
    }
  ],
  "deletions": [
    {
      "timestamp": "...",
      "mode": "trash",
      "status": "ok",
      "size_kb": 1,
      "path": "/tmp/x"
    }
  ]
}
```

- `sessions` / `deletions`：**最新在前**，长度 ≤ `limit`
- 空日志：空数组，非错误退出
- `size_kb`：可解析为数字则 number，否则 `null`

**Steps:**

- [ ] **Step 1: 写失败测试** — 对空日志目录调用 `history::load(...).to_json(limit)`，断言含 `limit`、`sessions: []`、`deletions: []`、`logs.operations` / `logs.deletions` 路径字段

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p vole-core history_json -- --nocapture
```

Expected: FAIL（module / 类型不存在）

- [ ] **Step 3: 最小骨架** — `HistoryReport { logs, limit, sessions, deletions }` + serde，空实现返回空数组

- [ ] **Step 4: 测试通过**

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/history crates/vole-core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(history): add empty HistoryReport JSON contract skeleton

EOF
)"
```

---

## Task 2: 解析 operations.log → sessions（对齐 mole）

**Files:**
- Create: `crates/vole-core/src/history/parse.rs`
- Create: `crates/vole-core/src/history/session.rs`
- Modify: `crates/vole-core/src/history/mod.rs`
- Test: 用 mole `tests/history.bats` 同类 fixture（手写 temp 文件即可）

**解析规则（对照 mole `history_load_sessions`）：**

- 行格式：`timestamp|pid|command|action|path|size_kb|note`（至少前 3–4 段）
- `action=start`：开新 session（command 取自字段）
- `action=end`：关闭当前 session（ended_at）
- 其它 action：计入 `operation_count` 与 `actions.*` 桶（removed/trashed/skipped/failed/rebuilt/other）
- `items` / `size`：按 mole 聚合逻辑（size 显示字符串，可先复用 `vole-core::units`）
- 未结束 session：`ended_at` 空字符串（mole 文本显示 `not ended`；JSON 为空串）

**Steps:**

- [ ] **Step 1: 写失败测试** — fixture：一条 start、若干 trashed、一条 end；断言 session 的 `command`、`actions.trashed`、`operation_count`、`ended_at` 非空

- [ ] **Step 2: 运行确认失败**

- [ ] **Step 3: 实现解析与聚合**

- [ ] **Step 4: 测试通过**；另加「无 start 的孤儿行」与「未 end」用例

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(history): parse operations.log into mole-compatible sessions

EOF
)"
```

---

## Task 3: 解析 deletions.log → deletion audit

**Files:**
- Modify: `crates/vole-core/src/history/parse.rs`
- Modify: `crates/vole-core/src/history/mod.rs`

**解析规则（对照 mole `history_load_deletions`）：**

- 行含 timestamp、mode、status、size、path（与现有 `DeletionLogger` 写出格式一致；以 mole `deletions.log` 样例为准）
- 最新在前，limit 截断
- 非法行跳过，不崩

**Steps:**

- [ ] **Step 1: 写失败测试** — 写入 2 行 deletion，`limit=1` 只返回最新 1 条

- [ ] **Step 2: 运行确认失败**

- [ ] **Step 3: 实现**

- [ ] **Step 4: 通过**

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(history): parse deletions.log into audit entries

EOF
)"
```

---

## Task 4: `vole history` CLI（文本 + `--json`）

**Files:**
- Create: `crates/vole-cli/src/history_cmd.rs`
- Modify: `crates/vole-cli/src/main.rs`
- Create: `scripts/verify-history-mole.sh`（可选但推荐）
- Create: `crates/vole-cli/tests/history_cli.rs`（或 bats 风格 shell）

**CLI：**

```
vole history [--json] [--limit N]
```

- `--limit`：默认 20，夹紧 1..=200（非法输入 → 退出码非 0 或夹紧；**优先夹紧**，对齐 mole `history_normalize_limit`）
- `--json`：stdout 仅 JSON；无 `--json`：人类可读文本（布局对齐 mole `history_render_text`，不必像素级相同，但 sections 一致：Recent sessions / Deletion audit / Logs）
- 日志缺失：当作空，exit 0

**Steps:**

- [ ] **Step 1: 写 CLI 测试** — temp HOME 下写 mini oplog，跑 `vole history --json --limit 5`，`jq` 检查 `limit==5` 与 session 字段

- [ ] **Step 2: 确认失败（子命令不存在）**

- [ ] **Step 3: 实现 clap + 文本渲染**

- [ ] **Step 4: `cargo test -p vole-cli` + 手动：

```bash
cargo run -p vole-cli -- history --json --limit 5
```

- [ ] **Step 5: 可选** — `scripts/verify-history-mole.sh`：同一 fixture 下对比 mole `bin/history.sh --json` 与 `vole history --json` 的 sessions/deletions 关键字段（允许 path 字符串差异仅来自 HOME）

- [ ] **Step 6: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(cli): add vole history with --json and --limit

EOF
)"
```

---

## Task 5: 冻结 `docs/protocol.md`

**Files:**
- Modify: `docs/protocol.md`（文首加 **Status: FROZEN**；注明冻结日期与 Phase 4 结束依据；history 若用独立 JSON **非** NDJSON 事件流，则加「附录：History JSON」并声明其不属于 StreamEvent）
- Modify: `scripts/check-protocol-doc.sh`（若需检查 FROZEN 标记或禁止未文档化新字段）
- Modify: `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md`（可选一行：Phase 5 Task：协议已冻结 → 指向 protocol.md）

**规则：**

- 冻结后：**不得**静默新增/重命名 NDJSON `StreamEvent` 字段；若必须变更 → 新 major `schema_version` + 新计划
- History JSON **不是** NDJSON 流的一部分；单独附录，避免与 `vole-proto` 事件混淆

**Steps:**

- [ ] **Step 1: 审计** — `rg` `StreamEvent` / `protocol.md` 与 `vole-proto` 字段仍一致；`bash scripts/check-protocol-doc.sh` 已绿

- [ ] **Step 2: 写入 FROZEN 声明 + History 附录（字段表）**

- [ ] **Step 3: CI/脚本仍通过**

```bash
bash scripts/check-protocol-doc.sh
cargo test -p vole-proto
```

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
docs(protocol): freeze NDJSON schema after Phase 4; document history JSON

EOF
)"
```

---

## Task 6: Shell completion（bash/zsh/fish）

**Files:**
- Modify: `crates/vole-cli/src/main.rs`（clap `CommandFactory` + `complete` 子命令或 `--generate-completion`）
- Create: `scripts/install-completions.sh`（可选）
- Docs: `README.md` 一小节如何安装

**Steps:**

- [ ] **Step 1: 选方案** — 优先 `clap_complete` 生成；子命令 `vole completions <shell>` 或隐藏 flag

- [ ] **Step 2: 实现 + 生成物可打印到 stdout（不强制提交生成文件）**

- [ ] **Step 3: 本地验证 zsh/bash 至少一种

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(cli): add shell completion generation

EOF
)"
```

---

## Task 7: 轻量交互入口（非完整 TUI）

**Files:**
- Create: `crates/vole-cli/src/interactive.rs`（或 `menu.rs`）
- Modify: `crates/vole-cli/src/main.rs` — 无子命令时进入菜单，或 `vole` / `vole menu`

**范围（刻意缩小）：**

- 菜单项：`status` / `clean --plan` / `history` / 退出
- **不**做完整 ratatui 多屏；stdin 数字选择 + 调用已有子命令逻辑即可
- 若时间紧：可只做「无参数时打印帮助并提示常用命令」——但设计写的是 interactive menu，优先最小可选菜单

**Steps:**

- [ ] **Step 1: 约定入口**（推荐：`vole` 无 args → 菜单；`vole --help` 仍显示帮助）

- [ ] **Step 2: 实现最小循环**

- [ ] **Step 3: 手动点选验证**

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(cli): add minimal interactive command menu

EOF
)"
```

---

## Task 8: Developer ID / 公证 / Homebrew 路径（可延期落地）

**Files:**
- Create or update: `docs/findings/2026-07-phase5-signing.md`
- Create: `scripts/sign-and-notarize.sh`（占位：检查 env、打印步骤；无证书时 skip）
- Create: `HomebrewFormula/vole.rb` 或 `docs/homebrew.md` 草稿
- Modify: `install.sh`（若仓库已有；否则创建最小 curl|sh 安装说明）

**验收（设计 Phase 5）：**

- 有证书的机器：脚本能完成签名+公证或明确失败原因
- 无证书：文档写清申请步骤与 CI 缺口；**不**阻塞 history/protocol 合并

**Steps:**

- [ ] **Step 1: 写 findings：当前无证书时的状态 + 所需 Apple 账号步骤**

- [ ] **Step 2: 占位脚本 + Homebrew formula 草稿（url/sha256 待 release）**

- [ ] **Step 3: Commit（允许标记 partial）**

```bash
git commit -m "$(cat <<'EOF'
docs(release): add signing/Homebrew placeholders for Phase 5 closeout

EOF
)"
```

---

## Task 9: Phase 5 验收清单

**Steps:**

- [ ] `vole history --json` 对 mole 写出的日志可读；对 vole 自己写出的 oplog/deletions 可读

- [ ] `scripts/verify-oplog-mole.sh` 仍绿（写路径未破坏）

- [ ] `docs/protocol.md` 含 FROZEN；`check-protocol-doc.sh` 绿

- [ ] completion 至少一种 shell 可生成

- [ ] 交互入口可启动并调用 history/status/clean --plan 之一

- [ ] `cargo test` / `cargo clippy -D warnings`（工作区）通过

- [ ] 更新本计划所有 Task checkbox 为 `[x]`

- [ ] Commit plan 勾选状态

```bash
git commit -m "$(cat <<'EOF'
docs(plan): mark Phase 5 tasks complete

EOF
)"
```

---

## Execution Handoff

计划写好后，执行方式二选一：

1. **Subagent-Driven（推荐）** — 每 Task 新 subagent + 两阶段 review；强制 skill：`wukong-code:subagent-driven-development`
2. **Inline Execution** — 本会话按 Task 执行；强制 skill：`wukong-code:executing-plans`

**本计划优先级：** Task 1–5 必须完成才算 Phase 5 核心交付；Task 6–7 应完成；Task 8 允许「文档+占位」即过关。

**分支建议：** `phase5-history`（或继续本 `phase5-plan` 分支开发），PR 合入 `main`；不直接推 `main`。
