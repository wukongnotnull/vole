# Phase 4：`clean` 命令 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task.

**Goal:** 实现 `vole clean`——安全闸口、应用保护层、规则引擎、plan/apply 两阶段与 NDJSON 报告，对齐 mole 删除语义且默认走废纸篓。

**Architecture:** `vole-core::safety` 承载 `validate_path_for_deletion` 与 TOCTOU；`vole-core::protection` 承载应用保护清单；`vole-core::rules` 表驱动规则；`vole-sys` 提供 `Trash`/`Fs` 后端；`vole-cli` 接线 `--plan` / `--apply` / `--json-stream`。

**参照设计文档：** `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` 第 8 节 Phase 4、5.6、5.7、6.1–6.3、7A。

## Global Constraints

- **4a 完成前不写任何清理规则。**
- 默认废纸篓；`trashed_bytes` / `deleted_bytes` 分口径报告。
- `clean.lock` 全局 `flock(LOCK_EX | LOCK_NB)`。
- plan apply 必须重走安全闸口 + `(dev,ino,mtime)` 校验。
- 一致性测试仅在 `VOLE_TEST_ROOT` 一次性环境运行。

---

## 子阶段

### 4a 安全闸口与应用保护层（~2.5 周）

- [x] Task 1: `validate_path_for_deletion` + `is_critical_deletion_path` + 测试（`core_safe_functions.bats` 子集）
- [x] Task 2: `is_endpoint_security_cache_path` + 前缀数据
- [x] Task 3: `should_protect_path` 数据化（TOML）+ 判定引擎
- [x] Task 4: `mole_delete` / 废纸篓后端 + `safe_remove` 包装
- [x] Task 5: 合并 `spike_toctou` → `safety::verify_plan_entry` + plan 威胁模型测试
- [x] Task 6: property test（随机路径 ∩ 保护清单 = ∅）

### 4b 规则引擎（~1.5 周）

- [x] Task 7: 规则 TOML schema + 策略 trait（glob、mtime、pgrep…）
- [x] Task 8: guard 与路径语义（设计 6.2 八条）
- [x] Task 9: `Orchestrator` 扩展为 plan 生成管线

### 4c 规则移植（~4–6 周，v1 Top 100–150）

- [x] Task 10: bats → JSON fixture 抽取脚本
- [x] Task 11: 首批规则批次 + `verify-clean-candidates.sh` 双跑 diff

### 4d plan/apply 与 CLI（~1 周）

- [x] Task 12: `vole clean --plan --json-stream`
- [x] Task 13: `vole clean --apply <plan>` + TTL
- [ ] Task 14: 交互 whitelist + 报告 NDJSON

---

## 验收（Phase 4 末）

见设计文档 Phase 4 验收 9 条；4a 子集：全部 `validate_path_for_deletion` 抽取测试通过。

```bash
cargo test -p vole-core safety::
cargo test --workspace
bash scripts/verify-clean-candidates.sh  # 4c 起
```
