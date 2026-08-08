# V2 CLI 全家桶收口 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans (inline). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 产品 v2 CLI 续篇收口：§3.2 命令面闸门进 CI、README/findings/规格回链；无新 CLI 能力故不 bump 版本。

**Architecture:** 复用既有 `scripts/check-command-surface.sh --enforce`；CI 增加硬门禁；文档宣告「产品 v2 CLI 全家桶」完成态（包线停在 2.5.0）。

**Tech Stack:** bash 闸门脚本、GitHub Actions、Markdown docs

## Global Constraints

- 权威规格：`docs/wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md` §3 / §5「收口」
- 无新用户可见 CLI → **不** bump `2.5.0`
- 别名已在 M5 落地；闸门 `--enforce` 本地已 0 gaps
- PR 合并：`gh pr merge --merge --delete-branch`（CI 全绿后）

---

### Task 1: CI 硬门禁 + 脚本注释

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/check-command-surface.sh`（头部注释：收口后为 CI 硬门禁）

- [ ] **Step 1:** 在 License / dep-direction 旁增加 `./scripts/check-command-surface.sh --enforce`
- [ ] **Step 2:** 本地再跑 `--enforce`，期望 exit 0、无 MISSING
- [ ] **Step 3:** Commit（可与 docs 同 commit 或紧随）

### Task 2: README + findings + 规格回链

**Files:**
- Modify: `README.md`
- Create: `docs/findings/2026-08-v2-cli-complete-closeout.md`
- Modify: `docs/wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md`（收口状态一句 + §5/§9）
- Modify: `docs/findings/2026-08-v2-m4-gate-checklist.md`（收口完成勾选，可选短更）

- [ ] **Step 1:** README 明确「产品 v2 CLI 全家桶」；列出/指向子命令（含 purge/installer/touchid/update/remove/hints-as-clean）；去掉「尚未移植自更新/自卸载」过时表述
- [ ] **Step 2:** 写 closeout findings，勾选 M4–M10 + 收口
- [ ] **Step 3:** 规格 2030 回链一句「收口已完成」；§9 勾选收口
- [ ] **Step 4:** Commit → push → PR → CI 绿后 merge

---

## Self-Review

1. Spec coverage: §3.2 CI、§3.4.8–10 README/findings/闸门、§5 收口行 → Task 1–2
2. No version bump（仅文档/闸门）
3. Aliases already present — no code change unless gate fails
