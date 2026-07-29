# Phase 0.5 Spike 汇总

**日期**：2026-07-29

## 三项结论

**A. 规则移植速率（Task 9）**  
20 条代表性规则的中位预估 75 min/条，外推 Phase 4c **约 19.5 周**——远超 6 周止损线。建议收缩到 Top 100–150 条或先建批量导入工具再复测。

**B. 不可信 plan 校验（Task 10）**  
`rustix` 逐段 `openat` 原型 2 小时内完成，三个 TOCTOU 攻击用例全绿。Phase 4d **1 周估算成立**，保留 plan/apply 两阶段。

**C. 平台行为（Task 11）**  
废纸篓不释放 `df` 空间（实证）；Chrome History 无 WAL、运行中 `mode=ro` 被锁；Homebrew **Cask** 2026-09 起强制签名+公证，**Formula** 无此要求；TCC 本机未弹窗，Developer ID 仍建议 Phase 1 前申请。

## 假设 → 实测 → 影响

| 假设 | 实测 | 影响 |
|---|---|---|
| Phase 4c 3–5 周 | 外推 ~19.5 周 | 必须收缩规则范围或提效工具 |
| plan/apply 校验 1 周 | 原型 2h，集成待做 | 保持两阶段 |
| Homebrew 可能强制公证 | 仅 Cask；Formula 豁免 | CLI 优先 Formula；Cask 需 $99/yr |
| WAL 三路径策略 | 本机无 WAL；锁时 ro 失败 | 按 journal 模式分支 |
| 重编译重弹 TCC | 本机未观测到 | Phase 1 Developer ID 实测 |

## 决策

**选项 2：调整方案后推进** — 收缩 Phase 4c 到 Top 100–150 条规则；保持 plan/apply；CLI 走 Homebrew Formula；Phase 1 前申请 Developer ID。
