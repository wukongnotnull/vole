# Orphan FDA 响亮提示验收

**日期**：2026-08-05  
**状态**：已实现  
**设计**：`docs/wukong-code/specs/2026-08-05-1919-orphan-fda-loud-hint-design.md`  
**计划**：`docs/wukong-code/plans/2026-08-05-1923-orphan-fda-loud-hint.md`

## 勾选

- [x] `select_custom` 返回 `CustomSelectResult`；orphan degrade 不再 `unwrap_or_default`
- [x] plan emit `Skipped { TccDenied }` + `PlanNotice::OrphanLibraryInaccessible`
- [x] `--json` / plan-out：当次 `coverage_note` 追加警告
- [x] human `--plan`：stderr 在 coverage 后再打一行警告
- [x] 文案中性（不声称扫描失败一定是 FDA）
- [x] 其它 custom handler 无 degrade

## 语义说明

`SkipReason::TccDenied` 在本路径表示规则级「无法访问用户 Library / 安装扫描失败」（含 FDA 等权限问题），**不仅**表示既有 `EndpointSecurityCache` 路径校验失败。协议允许日后追加更细变体；本里程碑刻意复用。

## 验证

- `cargo test -p vole-core --lib` 通过
- `cargo clippy -p vole-core -p vole-cli --all-targets -- -D warnings` 通过
