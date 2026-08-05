# B4.1 Claude VM orphan 验收

**日期**：2026-08-05  
**状态**：实现完成（包 **1.4.0**）  
**计划**：`docs/wukong-code/plans/2026-08-05-1829-b41-claude-vm-orphan.md`

## 勾选

- [x] 仅扫描 `~/Library/Application Support/Claude` 下 depth≤3 的 `*.bundle`
- [x] `MOLE_CLAUDE_VM_ORPHAN_AGE_DAYS` 默认 7；非法/空/0 → 7
- [x] Claude 进程探针可注入；Live：`pgrep -x Claude`；失败 fail-closed（视为 running）
- [x] mdfind / Spotlight fail-closed（复用 `OrphanDeps`）
- [x] apply 重判按路径分流 Claude vs 常规 orphan
- [x] 与 `ai-agents` Claude Desktop bundled keep-N 隔离
- [x] CI 用 `FakeOrphanDeps`，无真 pgrep/mdfind
- [x] `coverage_note` / README 不再把 Claude VM 标为未移植

## 非目标（确认未做）

- system services orphan
- Containers stubs
- Application Support 泛扫
