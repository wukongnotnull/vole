# Mole 对齐收口与闸控任务轨 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。
>
> **默认只执行 Task 1–3（收口核对）。** Task G1–G4 / D1 **禁止**开跑，除非人类在本会话明确写出「批准执行轨 GX」（或 D1）。Task G5 / N1 **永不**变成功能实现；仅允许核对式维护。

**Goal:** 把 [`2026-08-08-1727-mole-parity-roadmap-design.md`](../specs/2026-08-08-1727-mole-parity-roadmap-design.md) 落成可执行计划：完成近满配收口核对与文档门禁；将未对齐项按优先级写成闸控任务轨（显式批准前零实现）。

**Architecture:** 收口轨只读验证 + findings/交叉链接，不 bump 版本、不改产品行为。闸控轨（optimize 长尾 / 桌面特权助手）沿用既有 `optimize/catalog.rs` `in_m3` 翻转 + Mole handler 移植模式，或另仓 SMAppService；每轨先过闸门再另开 design（规格 §4.1）。本代际永不做项用禁区核对固化。

**Tech Stack:** Rust workspace（`vole-core` / `vole-cli`）、`scripts/inventory-mole-rules.py`、Mole `third_party/mole-1.48.1`、可选 vole-macos。

## Global Constraints

- 规格权威：[`docs/wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md`](../specs/2026-08-08-1727-mole-parity-roadmap-design.md)
- Mole 钉版：`third_party/mole-1.48.1`
- 快照版本：**1.45.0**（G2–G4 已合入；G1=1.42.0 / #92；收口轨核对时为 1.41.0）
- **默认下一项实现：无**（规格 §1 / §4.1；G2–G4 已完成；仅 G5 `disk_verify` 默认永不升必做；D1 可用通道代码已开 PR，待真机 uid==0 验收后改 coverage）
- 闸控轨开跑前：该轨必须已有（或本会话当场完成）**专用 design**，再按单轨单 PR
- `disk_verify`：**默认拒绝升必做**（规格 §3.3 P5）
- 本代际禁止实现：`purge` / `installer` / `touchid` / `hints` / Mole 式 `update`；禁止 `clean --apply` 删本地快照；禁止删 `/Library/Updates`、`/macOS Install Data`
- 合入 PR 用 **merge commit**（非 squash）
- 任务用语：实现项 / 下一项（不用隐喻缩写）

## File map

| 文件 | 职责 |
|---|---|
| `scripts/inventory-mole-rules.py` | Mole `safe_clean` vs `data/rules` 核对 |
| `data/rules/*.toml` | 启用规则计数（期望 540） |
| `crates/vole-core/src/ops/coverage.rs` | coverage 诚实面；「仍未移植」仅桌面特权助手 |
| `crates/vole-core/src/optimize/catalog.rs` | 23 task；22 `in_m3: true`；1 长尾 `false`（仅 `disk_verify`） |
| `crates/vole-core/src/optimize/tasks/actions.rs` | 闸控轨启用后的 plan/apply handler |
| `crates/vole-core/src/ops/optimize_plan.rs` / `optimize_apply.rs` | 闸控轨接线 |
| `docs/findings/2026-08-mole-parity-closeout.md` | 收口 findings |
| `docs/wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md` | 规格；收口后可回链本 plan |
| `third_party/mole-1.48.1/lib/optimize/tasks.sh` | 闸控轨 Mole 对照 |
| `vole-macos/`（另仓） | 仅轨 D1；本 plan 默认不改 |

---

## Part A — 收口核对（默认执行）

> **状态（2026-08-08）：已完成。** 依据 findings [`docs/findings/2026-08-mole-parity-closeout.md`](../../findings/2026-08-mole-parity-closeout.md) 与 commit `9a780f0`（`docs: record Mole near-parity closeout verification`）。另完成 Task G5「保持 false」核对与 Task N1 禁区核对（同日、同 findings）。

### Task 1: Inventory 与规格数字核对

**Files:**
- Read: `scripts/inventory-mole-rules.py`
- Read: `data/rules/*.toml`
- Read: `crates/vole-core/src/optimize/catalog.rs`
- Create: `/tmp/mole-parity-closeout-check.json`（本地临时，不入库）

**Interfaces:**
- Consumes: 规格 §1 / §3.1 断言（规则 540；inventory 513/507；unported_all=0；match_reason none=6；optimize 18/5）
- Produces: 核对通过记录（写入 Task 2 findings）

- [x] **Step 1: 跑 inventory 并落 JSON**（2026-08-08 · findings）

```bash
cd /Users/wukong/Documents/vole
python3 scripts/inventory-mole-rules.py --json /tmp/mole-parity-closeout-check.json
```

Expected stdout 含：

```text
"total": 513
"ported": 507
"unported_all": 0
```

- [x] **Step 2: 断言 6 条 none 全为 custom**（2026-08-08 · findings）

```bash
python3 - <<'PY'
import json
rows=json.load(open('/tmp/mole-parity-closeout-check.json'))
none=[r for r in rows if r.get('match_reason')=='none']
assert len(none)==6, len(none)
assert all(r['complexity_guess']=='custom' for r in none), none
print('ok', [(r['source_file'], r['proposed_id']) for r in none])
PY
```

Expected: `ok` 打印 6 元组（含 `app_caches.sh` / `apps.sh` / `caches.sh` / `dev.sh` / `user.sh`）。

- [x] **Step 3: 断言启用规则数 = 540**（2026-08-08 · findings）

```bash
python3 - <<'PY'
from pathlib import Path
import re
text='\n'.join(p.read_text() for p in Path('data/rules').glob('*.toml'))
parts=re.split(r'\[\[rules?\]\]', text)
enabled=0
for part in parts[1:]:
    if not re.search(r'(?m)^\s*id\s*=\s*"', part):
        continue
    if 'disabled = true' in part or 'disabled=true' in part:
        continue
    enabled += 1
assert enabled==540, enabled
print('enabled', enabled)
PY
```

Expected: `enabled 540`

- [x] **Step 4: 断言 optimize catalog 18/5**（2026-08-08 · findings；main 仍 18/5）

```bash
rg -n 'in_m3: true' crates/vole-core/src/optimize/catalog.rs | wc -l
rg -n 'in_m3: false' crates/vole-core/src/optimize/catalog.rs | wc -l
```

Expected: `true` 行数 **18**；`false` 行数 **5**。五条 false id 必须为：

```bash
rg -n 'in_m3: false' -B6 crates/vole-core/src/optimize/catalog.rs | rg 'id:'
```

Expected 含：`spotlight_index_optimize`、`spotlight_orphan_rules_cleanup`、`shared_file_list_repair`、`disk_verify`、`login_items_audit`。

- [x] **Step 5: coverage「仍未移植」仅桌面特权助手**（2026-08-08 · findings）

```bash
rg -n '仍未移植' crates/vole-core/src/ops/coverage.rs
```

Expected 唯一产品句类似：`仍未移植：桌面 SMAppService / 特权助手。`

- [x] **Step 6: Commit（若无文件变更则跳过 commit）**

本 Task 默认无源码改动。若本地为核对新建了不入库临时文件，勿 `git add /tmp/...`。核对无源码改动，跳过独立 commit；成果并入 Task 2 `9a780f0`。

---

### Task 2: 收口 findings + 规格回链本 plan

**Files:**
- Create: `docs/findings/2026-08-mole-parity-closeout.md`
- Modify: `docs/wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md`（§5 表格增一行指向本 plan）

**Interfaces:**
- Consumes: Task 1 核对结果
- Produces: findings 文档；规格 §5 回链

- [x] **Step 1: 写 findings**（2026-08-08 · `docs/findings/2026-08-mole-parity-closeout.md`）

创建 `docs/findings/2026-08-mole-parity-closeout.md`：

```markdown
# Mole 近满配收口核对

**日期**：2026-08-08  
**状态**：完成  
**规格**：[`../wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md`](../wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md)  
**计划**：[`../wukong-code/plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../wukong-code/plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md)

## 核对结果

| 项 | 期望 | 实际 |
|---|---|---|
| 包版本 | 1.41.0 | （填入 `Cargo.toml` workspace version） |
| 启用规则 | 540 | （Task 1） |
| Mole inventory | 513 / ported 507 / unported_all 0 | （Task 1） |
| match_reason none | 6 且全 custom | （Task 1） |
| optimize in_m3 | 18 true / 5 false | （Task 1） |
| coverage 仍未移植 | 仅桌面 SMAppService / 特权助手 | （Task 1） |

## 结论

近满配必做已关闭。默认下一项实现：无。闸控轨见计划 Part B/C；本代际永不做见 Part D。
```

把表格「实际」列换成 Task 1 真实输出数字。

- [x] **Step 2: 规格 §5 增回链**（2026-08-08 · `9a780f0`）

在 `2026-08-08-1727-mole-parity-roadmap-design.md` §5 表末追加：

```markdown
| [`../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md) | 本规格之收口核对 + 闸控任务轨计划 |
```

- [x] **Step 3: Commit** — `9a780f0` `docs: record Mole near-parity closeout verification`

```bash
git add docs/findings/2026-08-mole-parity-closeout.md \
  docs/wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md
git commit -m "$(cat <<'EOF'
docs: record Mole near-parity closeout verification

EOF
)"
```

---

### Task 3: 禁区与闸控索引自检（不改行为）

**Files:**
- Read: `crates/vole-cli/src/main.rs`
- Read: `crates/vole-core/src/optimize/catalog.rs`

**Interfaces:**
- Produces: findings 追加「禁区自检」小节（可 amend 进 Task 2 同 commit 若尚未 push；否则新 commit）

- [x] **Step 1: CLI 无本代际禁止子命令**（2026-08-08 · findings「禁区自检」）

```bash
rg -n 'Purge|Installer|TouchId|Touchid|Hints|enum Command' crates/vole-cli/src/main.rs | head -40
```

Expected: `enum Command` 变体仅含 Clean / Uninstall / Optimize / Status / Analyze / History / Completions（及既有辅助），**无** Purge/Installer/TouchId/Hints。

- [x] **Step 2: 确认长尾仍 false**（2026-08-08 · findings；main 仍 `main.len() == 18`）

```bash
cargo test -p vole-core catalog::tests::m3_main_path_flags -- --exact
```

Expected: PASS；且断言 `main.len() == 18`、不含五条长尾 id。

- [x] **Step 3: 在 findings 追加禁区自检通过句并 commit（若有改动）**

禁区自检已写入 findings「禁区自检」节，并入 `9a780f0`（未另开 commit）。

```bash
git add docs/findings/2026-08-mole-parity-closeout.md
git commit -m "$(cat <<'EOF'
docs: note Mole parity forbidden-surface self-check

EOF
)"
```

若无文件变更：跳过 commit。

---

## Part B — 闸控轨 G1–G5（optimize 可选长尾）

**闸门协议（硬性）：**

1. 人类明确：「批准执行轨 G\<n\>」
2. 当场或已有专用 design：`docs/wukong-code/specs/YYYY-MM-DD-HHmm-<task>-design.md`
3. **单轨单 PR**；发版 MINOR 另议
4. 未满足 1–2：**整轨跳过**，不得改 `in_m3`

对照源：`third_party/mole-1.48.1/lib/optimize/tasks.sh` + `catalog.sh`。

---

### Task G1: `login_items_audit`（P1 · 闸控）

> **状态：已完成 · 1.42.0 / PR #92**（merge commit `af69af8`）。专用 design：[`2026-08-08-1754-optimize-login-items-audit-design.md`](../specs/2026-08-08-1754-optimize-login-items-audit-design.md)。`login_items_audit.in_m3 = true`；主路径 **19**。

**Gate:** 已收到「批准执行轨 G1」并合入 main。

**Files（批准后）：**
- Modify: `crates/vole-core/src/optimize/catalog.rs`（`login_items_audit.in_m3: true`）
- Modify: `crates/vole-core/src/optimize/tasks/actions.rs`（plan/apply）
- Modify: `crates/vole-core/src/ops/optimize_plan.rs` / `optimize_apply.rs`
- Test: `catalog.rs` 单测；actions 单测；必要时 CLI fixture
- Mole: `opt_login_items_audit`（`tasks.sh` 约 1442 行起）

**Interfaces:**
- Consumes: 既有 uninstall login items 能力（避免重复破坏性删除语义冲突——design 须写清 audit vs uninstall）
- Produces: `in_m3` 主路径含 `login_items_audit`；plan 可预览；apply fail-closed

- [x] **Step 0: 闸门** — 已收到「批准执行轨 G1」（闸门已过）

若本会话无「批准执行轨 G1」：输出 `SKIP G1` 并结束本 Task。

- [x] **Step 1: 写/确认专用 design 已批准** — `docs/wukong-code/specs/2026-08-08-1754-optimize-login-items-audit-design.md`（`50d6000`，已随 PR #92 合入）

- [x] **Step 2: RED — catalog 期望含 login_items_audit 且 main.len 19**（TDD 已跑；合入后 `m3_main_path_flags` 期望 `main.len() == 19`）

- [x] **Step 3: GREEN — `login_items_audit.in_m3 = true`**（`c0e438a`）

- [x] **Step 4: RED/GREEN — `plan_login_items_audit` / apply handler** — `login_items_audit.rs` + actions；只读 audit；禁非特权 `sfltool dumpbtm`；fail-closed（`22d379d` / `c0e438a`）

- [x] **Step 5: coverage / README / releases / 版本 bump** — **1.42.0**；`docs/releases/v1.42.0.md`

- [x] **Step 6: Commit + PR（merge commit）** — PR [#92](https://github.com/wukongnotnull/vole/pull/92) → `af69af8`

```bash
git commit -m "$(cat <<'EOF'
feat(optimize): enable login_items_audit on main path

EOF
)"
```

---

### Task G2: `spotlight_orphan_rules_cleanup`（P2 · 闸控）

> **状态：已完成 · 1.43.0 / PR #93**（merge `d059eb7`）。专用 design：[`2026-08-08-1822-optimize-spotlight-orphan-rules-cleanup-design.md`](../specs/2026-08-08-1822-optimize-spotlight-orphan-rules-cleanup-design.md)。主路径 **20**。

**Gate:** 已收到「批准执行轨 G2」并合入 main。

**Files（批准后）：**
- Modify: `catalog.rs`（`spotlight_orphan_rules_cleanup.in_m3: true`）
- Modify: `actions.rs` + optimize plan/apply
- Mole: `opt_spotlight_orphan_rules_cleanup`（见 `tasks.sh` / catalog 注册名）

**Interfaces:**
- Produces: 主路径 +1（在 G1 已合入后为 20，否则相对当时 baseline +1；design 写死期望 `main.len`）

- [x] **Step 0: 闸门** — 已收到「批准执行轨 G2」
- [x] **Step 1: 专用 design 批准** — [`2026-08-08-1822-optimize-spotlight-orphan-rules-cleanup-design.md`](../specs/2026-08-08-1822-optimize-spotlight-orphan-rules-cleanup-design.md)
- [x] **Step 2: RED catalog 单测** — `main.contains("spotlight_orphan_rules_cleanup")`；`main.len()==20`
- [x] **Step 3: GREEN 翻转 `in_m3`**
- [x] **Step 4: RED/GREEN plan/apply** — fail-closed；`defaults` 重写 keep；禁静默大范围删
- [x] **Step 5: coverage / 发版** — **1.43.0**
- [x] **Step 6: Commit + PR** — PR [#93](https://github.com/wukongnotnull/vole/pull/93)

```bash
git commit -m "$(cat <<'EOF'
feat(optimize): enable spotlight_orphan_rules_cleanup

EOF
)"
```

---

### Task G3: `spotlight_index_optimize`（P3 · 闸控）

> **状态：已完成 · 1.44.0 / PR #95**（merge `cf0eb24`）。专用 design：[`2026-08-08-1836-optimize-spotlight-index-optimize-design.md`](../specs/2026-08-08-1836-optimize-spotlight-index-optimize-design.md)。主路径 **21**。

**Gate:** 已收到「批准执行轨 G3」并合入 main。

**Files（批准后）：**
- Modify: `catalog.rs` / `actions.rs` / privilege 若需 `sudo -n mdutil`
- Mole: `opt_spotlight_index_optimize`（`tasks.sh` 约 754 行；含 `sudo mdutil -E`）

**Interfaces:**
- Consumes: 既有 `PrivilegeBackend` / `sudo -n`（禁止新交互 sudo 体系）
- Produces: 智能检测后重建索引；低压/健康状态 noop

- [x] **Step 0: 闸门** — 已收到「批准执行轨 G3」
- [x] **Step 1: 专用 design** — [`2026-08-08-1836-optimize-spotlight-index-optimize-design.md`](../specs/2026-08-08-1836-optimize-spotlight-index-optimize-design.md)（与 `system_maintenance` 去重）
- [x] **Step 2–4: catalog 翻转 + plan/apply TDD** — `PrivilegeBackend::rebuild_spotlight_index`；`VOLE_TEST_NO_AUTH` 下永不真 sudo
- [x] **Step 5: coverage / 发版** — **1.44.0**
- [x] **Step 6: Commit + PR** — PR [#95](https://github.com/wukongnotnull/vole/pull/95)

```bash
git commit -m "$(cat <<'EOF'
feat(optimize): enable spotlight_index_optimize

EOF
)"
```

---

### Task G4: `shared_file_list_repair`（P4 · 闸控）

> **状态：已完成 · 1.45.0 / PR #97**（merge `23813ab`）。专用 design：[`2026-08-08-1902-optimize-shared-file-list-repair-design.md`](../specs/2026-08-08-1902-optimize-shared-file-list-repair-design.md)。主路径 **22**（仅 `disk_verify` 仍长尾）。

**Gate:** 已收到「批准执行轨 G4」并合入 main。

**Files（批准后）：**
- Modify: `catalog.rs` / `actions.rs` / optimize plan/apply
- Mole: `opt_shared_file_list_repair`（`tasks.sh` 约 1118 行）

**Interfaces:**
- Produces: Finder favorites / recent documents 修复；高复杂 → design 可决定仅 plan 报告、apply 仍 coverage

- [x] **Step 0: 闸门** — 已收到「批准执行轨 G4」
- [x] **Step 1: 专用 design** — [`2026-08-08-1902-optimize-shared-file-list-repair-design.md`](../specs/2026-08-08-1902-optimize-shared-file-list-repair-design.md)
- [x] **Step 2–4: TDD 接线** — `plutil -lint` 失败才删；跳过 ApplicationRecentDocuments；禁 sfltool
- [x] **Step 5: coverage / 发版** — **1.45.0**
- [x] **Step 6: Commit + PR** — PR [#97](https://github.com/wukongnotnull/vole/pull/97)

```bash
git commit -m "$(cat <<'EOF'
feat(optimize): enable shared_file_list_repair

EOF
)"
```

---

### Task G5: `disk_verify`（P5 · 默认拒绝升必做）

> **状态（2026-08-08）：「保持 false」核对已完成**（findings / main `in_m3: false`）。实现轨仍默认永久 SKIP，无推翻批准。

**Gate:** **默认永久 SKIP。** 仅当人类写出「推翻默认并批准执行轨 G5」才可进入实现；否则本 Task 只做「保持 false」核对。

**Files:**
- Read: `crates/vole-core/src/optimize/catalog.rs`
- Mole: `opt_disk_verify`（`tasks.sh` 约 1189 行）

- [x] **Step 1: 确认仍为长尾**（2026-08-08 · findings；main 复核 `in_m3: false`）

```bash
rg -n 'id: "disk_verify"' -A5 crates/vole-core/src/optimize/catalog.rs
```

Expected: `in_m3: false`

- [x] **Step 2: 无推翻批准 → 结束**（2026-08-08）

输出：`KEEP disk_verify out of main path (spec P5)`。**禁止**翻转 `in_m3`。

- [ ] **Step 3: （仅推翻默认后）** 先写 design，再按 G1 同构 TDD；commit message 须含风险说明。

---

## Part C — 闸控轨 D1（桌面特权助手）

### Task D1: SMAppService / PrivilegedHelper（延后 · 闸控）

> **状态：可用通道代码已交付 · 待真机验收（2026-08-08）**  
> 闸门已过（「批准执行轨 D1」）。vole-macos [#3](https://github.com/wukongnotnull/vole-macos/pull/3) 交付白名单 XPC 删除/bootout、Clean UI 接线与降级、Hardened Runtime；骨架见 [#2](https://github.com/wukongnotnull/vole-macos/pull/2)。  
> **剩余阻塞：** 真机系统设置批准后 ping `uid==0`；公证（缺 notarytool Keychain 凭据，已文档化）；本仓 coverage「仍未移植」句待 uid==0 验收后删除；Uninstall UI 仍为扩展点。  
> 专用 design：[`vole-macos` `2026-08-08-1822-smappservice-privileged-helper-design.md`](https://github.com/wukongnotnull/vole-macos/blob/main/docs/wukong-code/specs/2026-08-08-1822-smappservice-privileged-helper-design.md)  
> 实施 plan：[`vole-macos` `2026-08-08-1823-smappservice-privileged-helper.md`](https://github.com/wukongnotnull/vole-macos/blob/main/docs/wukong-code/plans/2026-08-08-1823-smappservice-privileged-helper.md)

**Gate:** 已收到「批准执行轨 D1」。规格 §4.2 下一代际队列项；本轨骨架不改 CLI `sudo -v`。

**Files（批准后 · 主要在另仓）：**
- `vole-macos/` Xcode 工程、Helper target、SMAppService 注册
- 可选：`vole` 仓 `PrivilegeBackend` 适配桌面 backend（design 定界）
- **禁止**在未批准时改 CLI `sudo -v` 语义冒充 Helper

**Interfaces:**
- Consumes: 现有 Clean MVP sidecar；覆盖笔记「仍未移植：桌面 SMAppService / 特权助手」
- Produces: 用户批准一次后的持久提权删除/unload 通道（具体 API 以 D1 design 为准）

- [x] **Step 0: 闸门** — 已收到「批准执行轨 D1」
- [x] **Step 1: 在 vole-macos 写专用 design 并批准** — `2026-08-08-1822-smappservice-privileged-helper-design.md`
- [x] **Step 2: 按该 design 另写 `vole-macos` 实施 plan 并执行** — 骨架 [#2](https://github.com/wukongnotnull/vole-macos/pull/2)；可用通道 [#3](https://github.com/wukongnotnull/vole-macos/pull/3)
- [ ] **Step 3: vole coverage「仍未移植」句在 Helper 可用后删除或改写；发版说明双仓同步** — **阻塞：待真机 ping uid==0 验收**
- [x] **Step 4: PR（各仓）用 merge commit** — vole-macos 骨架 [#2](https://github.com/wukongnotnull/vole-macos/pull/2)；可用通道 [#3](https://github.com/wukongnotnull/vole-macos/pull/3)；本仓 docs 同步本 PR

---

## Part D — 本代际永不做（禁区维护 · 非功能轨）

### Task N1: 禁区回归（只读核对）

> **状态（2026-08-08）：已完成。** 依据 findings「禁区自检」与 commit `9a780f0`。

**Files:**
- Read: protection / install-macos / local snapshots 相关测试源

- [x] **Step 1: 无删除 Updates / Install Data 的 apply 路径**（2026-08-08 · findings）

```bash
rg -n '/Library/Updates|Install Data' crates/vole-core/src --glob '*.rs' | head -40
```

Expected: 仅 keep / 禁区 / 注释 / 测试断言，**无**删除这两个路径的 apply 实现。

- [x] **Step 2: 无 deletelocalsnapshots**（2026-08-08 · findings）

```bash
rg -n 'deletelocalsnapshots' crates --glob '*.rs'
```

Expected: 无生产调用（测试里可断言「不得出现」）。

- [x] **Step 3: findings 记「禁区仍在」一句（可并入 Task 2 文档）**（2026-08-08 · findings「禁区自检」）

无行为变更则可不单独 commit。

---

## 执行顺序（默认）

```text
Task 1 → Task 2 → Task 3 → Task G5 Step1–2（保持 false）→ Task N1
STOP
```

> **进度（2026-08-08）：** 上列默认顺序已全部完成（findings + `9a780f0`）。闸控轨：**G1–G4 已完成**（1.42.0–1.45.0 / PR #92 #93 #95 #97）；**G5** 保持 `disk_verify` false；**D1 部分完成**（vole-macos Helper 骨架；coverage 仍保留「仍未移植」）。

之后仅在显式批准后：`D1`（分仓）或推翻默认的 `G5`。  
**永不**默认进入：`purge` / `installer` / `touchid` / `hints` / `update` 实现任务（本计划不下发此类 Task）。

## Spec coverage

| 规格章节 | 计划落点 |
|---|---|
| §1 结论 / 默认下一项无 | Part A；执行顺序 STOP |
| §1.1 已对齐 | Task 1 数字核对 |
| §1.2 A 可选长尾 | Part B G1–G5 |
| §1.2 B 永不做 | Part D N1；不下发实现 Task |
| §1.2 C 延后桌面 | Part C D1 |
| §3.1 inventory 假阴性 | Task 1 Step 2 |
| §3.3 P1–P5 | G1–G5 |
| §4.1 显式批准 + 单 design | 闸门协议 |
| §4.2 下一代际队列 | D1 闸门 + 不授权默认执行 |

## Self-review

1. **Spec coverage:** §1–§4 均有落点；B 类永不做无实现 Task。  
2. **Placeholder scan:** 闸控轨在未批准时以 Step 0 SKIP 结束；批准后要求专用 design 补全 handler 细节（符合规格 §4.1），非空泛 TBD。  
3. **Type consistency:** optimize 轨统一经 `in_m3` + `actions.rs` + `optimize_plan`/`optimize_apply`。

---

Plan complete and saved to `docs/wukong-code/plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`. Two execution options:

**1. Subagent-Driven** — 每 Task 新 subagent，Task 间复审（推荐用于闸控轨）

**2. Inline Execution** — 本会话按 executing-plans 跑（推荐用于 Part A 收口）

按仓库习惯默认 **Inline** 且 **只跑 Task 1–3（+ G5 核对 + N1）**。需要跑闸控轨时请明确「批准执行轨 GX / D1」。
