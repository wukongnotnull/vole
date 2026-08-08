# Mole 近满配收口核对

**日期**：2026-08-08  
**状态**：完成  
**规格**：[`../wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md`](../wukong-code/specs/2026-08-08-1727-mole-parity-roadmap-design.md)  
**计划**：[`../wukong-code/plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../wukong-code/plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md)

## 核对结果

| 项 | 期望 | 实际 |
|---|---|---|
| 包版本 | 1.41.0 | 1.41.0 |
| 启用规则 | 540 | 540 |
| Mole inventory | 513 / ported 507 / unported_all 0 | 513 / 507 / 0 |
| match_reason none | 6 且全 custom | 6：obsolete-editor-label-extension；orphaned-claude-workspace-vm；orphaned-label-bundle-id；description；label×2（dev.sh / user.sh） |
| optimize in_m3 | 18 true / 5 false | 18 / 5（spotlight_index_optimize、spotlight_orphan_rules_cleanup、shared_file_list_repair、disk_verify、login_items_audit） |
| coverage 仍未移植 | 仅桌面 SMAppService / 特权助手 | 通过 |

## 禁区自检

- CLI `enum Command` 无 Purge / Installer / TouchId / Hints
- `disk_verify` 保持 `in_m3: false`（规格 P5）
- 无生产路径删除 `/Library/Updates`、`/macOS Install Data`
- 无生产调用 `deletelocalsnapshots`

## 结论

近满配必做已关闭。默认下一项实现：无。闸控轨见计划 Part B/C；本代际永不做见 Part D。
