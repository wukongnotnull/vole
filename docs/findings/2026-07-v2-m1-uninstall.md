# M1：`vole uninstall` 勾选

**日期**：2026-07-30  
**状态**：实现完成（发版 **1.1.0**）  
**设计**：[`docs/wukong-code/specs/2026-07-30-1900-v2-product-goals-design.md`](../wukong-code/specs/2026-07-30-1900-v2-product-goals-design.md)  
**计划**：[`docs/wukong-code/plans/2026-07-30-1910-v2-m0-m1-uninstall.md`](../wukong-code/plans/2026-07-30-1910-v2-m0-m1-uninstall.md)  
**M0 spike**：[`2026-07-v2-m0-uninstall-spike.md`](2026-07-v2-m0-uninstall-spike.md)

## 主路径

| 项 | 状态 |
|---|---|
| `ProtectionMode::Uninstall` | ✅ |
| `should_protect_from_uninstall` + 官方卸载器 | ✅ |
| leftovers + naming variants + sibling guard | ✅ |
| `build_uninstall_plan` | ✅ |
| `apply_uninstall_proto_plan` / `apply_uninstall_plan` | ✅ |
| `vole uninstall` CLI plan/apply/json | ✅ |
| 菜单 + 补全（clap） | ✅ |
| 协议注明 `uninstall:` 前缀 | ✅ |

## 长尾（coverage_note）

| 顺序 | 项 | 状态 |
|---|---|---|
| ① | brew cask 卸载联动 | ✅ **1.33.0** |
| ② | login items | 未实现 |
| ③ | 系统 LaunchDaemons / `/Library` sudo | 未实现 |
