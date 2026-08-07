# Private Var DB DiagnosticPipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans (inline) to implement task-by-task.

**Goal:** 落地 `private-var-db-diagnostic-pipeline`：深度 ≤5、≥7d、`sudo -n`（1.17.0）。

**Architecture:** 形状谓词 + Privilege + walk + older_than_days=7 apply 绑定。

## Global Constraints

- 版本 **1.17.0**；规则 **522 → 523**；不打 tag；security-review

### Task 1: 谓词 + Privilege + plan/apply + TOML

仿 diagnostics，去掉 tracev3 分龄；rule 放 `user-devtools.toml`。

### Task 2: Coverage / 1.17.0

coverage / README 523；Cargo/Formula；releases + findings；`cargo test -p vole-core --lib`

### Task 3: PR

push + PR + security-review + CI 绿后 merge
