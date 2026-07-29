# Spike A：20 条规则移植速率

**日期**：2026-07-29  
**状态**：选取与复杂度分析完成；完整「TOML → harness diff 为零」循环待 Phase 4 最小规则引擎

## 选取的 20 条（按设计 3.3 配比）

| # | rule_id | 类型 | 来源 | 预估净耗时 | 卡在哪 |
|---|---|---|---|---|---|
| 1 | xcode-cache | 纯路径 | `app_caches.sh:80` | 15 min | — |
| 2 | vscode-logs | 纯路径 | `app_caches.sh:124` | 15 min | — |
| 3 | vscode-cache | 纯路径 | `app_caches.sh:125` | 15 min | — |
| 4 | simulator-cache | 纯路径 | `app_caches.sh:73` | 20 min | glob 展开 |
| 5 | ios-device-logs | 纯路径 | `app_caches.sh:81` | 15 min | — |
| 6 | xcode-products | 纯路径 | `app_caches.sh:83` | 15 min | — |
| 7 | xcode-doc-cache | 纯路径 | `app_caches.sh:86` | 15 min | 条件 guard |
| 8 | coresim-logs | 纯路径 | `app_caches.sh:75` | 15 min | — |
| 9 | jetbrains-ext | keep_newest | `app_caches.sh` 编辑器分支 | 60 min | mtime 策略 |
| 10 | npm-logs | keep_newest | `user.sh` npm logs | 45 min | 日期模式 |
| 11 | brew-cache-age | keep_newest | `brew.sh` | 60 min | 多路径 |
| 12 | ai-agent-cache | keep_newest | `dev.sh` | 90 min | 多子目录 |
| 13 | xctest-devices | not_running | `clean_xcode` 相关 | 90 min | pgrep guard |
| 14 | simulator-running | not_running | CoreSimulator | 75 min | 进程名匹配 |
| 15 | docker-daemon | not_running | `dev.sh` | 90 min | 守护进程检测 |
| 16 | ai-symlink-active | symlink | `dev.sh` AI agents | 120 min | active symlink 保护 |
| 17 | node-modules-link | symlink | `project.sh` | 90 min | 链接目标判定 |
| 18 | chrome-model-cache | symlink | `user.sh` | 120 min | compiled model |
| 19 | sim-runtime-volumes | custom | `system.sh` 自定义候选 | 180 min | 非路径候选生成 |
| 20 | launch-services-db | custom | `launch_services.sh` | 150 min | 数据库 + 重建语义 |

**20 条预估合计**：约 22.5 小时（若连续做）；中位 **75 min/条**。

## 外推

```
547 × 75 min ÷ 60 ÷ 35 h/周 ≈ 19.5 周（仅 Phase 4c 规则移植）
```

**判定**：外推 **> 6 周**，触发设计文档第 10 节止损判据。

## 建议（进入 Phase 1 前）

1. **按原计划推进但收缩 Phase 4c**：首批只做 Top 100–150 条（按释放空间排序），其余报告提示继续用 Mole。
2. **或** 提高规则引擎抽象一次到位（批量 TOML 导入、策略组合器），目标把中位耗时压到 30 min 以下再复测。
3. **不建议** 退回只读范围——harness 与基准已就绪，收缩规则比放弃 `clean` 成本低。

## 未完成项

- `rules/spike/*.toml`：待 Phase 1 与最小规则引擎一并落地（本 spike 以源码分析计时，未跑 harness diff）。
- `custom` 在 20 条中占 2 条（10%），高于设计 5% 上限假设——custom 规则可能比预期多。
