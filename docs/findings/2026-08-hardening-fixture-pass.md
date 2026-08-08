# Findings · 2026-08 hardening / fixture pass（建议第 2 项）

- 日期：2026-08-08
- 基线：`main` @ `15f6781`（1.45.0；G1–G4 已合入）
- 范围：缺陷修复 / 文档一致性 / fixture / conformance 门禁核对 / 安全加固；**不含** G5 `disk_verify`、D1

## 本机验证（通过）

| 检查 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `./scripts/check-license.sh` | PASS |
| `./scripts/check-dep-direction.sh` | PASS |
| `./scripts/check-protocol-doc.sh` | PASS |
| `cargo test -p vole-core --lib`（基线全量曾 534 passed / 1 ignored） | PASS（基线） |
| inventory `scripts/inventory-mole-rules.py` | total 513 / ported 507 / unported_all 0；none=6 且均为 custom |
| 启用规则计数 `data/rules` | 540 |
| optimize catalog | 23 task；`in_m3` 22 true / 1 false（仅 `disk_verify`） |
| coverage 诚实面 | 仍未移植仅「桌面 SMAppService / 特权助手」；G1–G4 已写入已移植段 |
| README / Formula / workspace 版本 | 一致为 **1.45.0** |
| 环境 | 未全局预设 `VOLE_TEST_NO_AUTH=1` |

## G1–G4 巡检

| 轨 | 状态 | 备注 |
|---|---|---|
| G1 `login_items_audit` | 主路径 + 单测齐全 | 发现并修复 sentinel 路径误判（见下） |
| G2 `spotlight_orphan_rules_cleanup` | 主路径 + plan/apply/Fake | 无缺口 |
| G3 `spotlight_index_optimize` | 主路径 + env gate 单测 | 无缺口 |
| G4 `shared_file_list_repair` | 主路径 + Fake plan/apply | 补 TestMode plan / apply Skipped / 禁 sfltool 源码锁 |

### 未改（有意）

- **Formula `sha256` 占位全零**：release 资产发布前的既有流程占位，非 G1–G4 回归；不在本 pass 伪造 hash。
- **optimize 无 clean-style JSON fixture**：G1–G4 以 hermetic Fake + `actions` 单测覆盖；conformance harness 仍面向 clean plan-stage（CI `conformance-plan-only`），不要求 optimize 双跑 fixture。
- **G5 / D1**：按规格与用户边界，本 pass 不实现。
- **无关大版本 bump**：本修复为 PATCH 级行为修正，不抬 1.45.0。

## 本 pass 代码变更

1. **缺陷**：`is_unavailable_audit_path` 仅看 basename，导致登录项名为 `login_items_audit` 时 broken 候选被误判为 unavailable → `NeedsPrivilege`。改为要求父目录为 `.vole-optimize-action`。
2. **安全加固测试**：源码级禁止非特权 `sfltool`（G1）与任何 `sfltool` 调用（G4）。
3. **覆盖补全**：G4 `VOLE_TEST_NO_AUTH` apply → `Skipped`；G4 plan TestMode 空候选。

## 测试证据（本分支）

```text
optimize::tasks::login_items_audit::tests::*                     8 passed
optimize::tasks::shared_file_list::tests::*                      7 passed
optimize::tasks::actions::tests::apply_login_items_audit_*       ok
optimize::tasks::actions::tests::apply_shared_file_list_repair_test_no_auth_skipped  ok
optimize::catalog::tests::m3_main_path_flags                     ok（disk_verify 仍排除；len==22）
ops::coverage::tests::*                                          8 passed
```
