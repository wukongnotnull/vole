# uninstall / optimize apply 权限响亮提示设计

- 日期：2026-08-05
- 状态：已批准
- 依据：CLI 打磨轨（orphan FDA 响亮提示后续）；用户选定窄刀「仅 apply 收口」
- 对照：[`2026-08-05-1919-orphan-fda-loud-hint-design.md`](2026-08-05-1919-orphan-fda-loud-hint-design.md)
- 包版本意图：**PATCH `1.4.2`**

## 1. 结论

当 `vole uninstall --apply` / `vole optimize --apply` 的 `Report.skipped_by_reason` 含 **`TccDenied` 或 `NeedsPrivilege`** 时，不得只留下冷冰冰的计数：须给出与 orphan FDA 提示同风格、可操作的中文警告。

- **人读**：stderr 一行警告（在既有 apply 摘要 / plan-carried `coverage_note` 之后）
- **`--json`**：把同一句追加进 `report.coverage_note`（无则新建）
- **`--json-stream`**：不额外刷中文 stdout；既有 per-entry `skipped` 事件已带 reason。若随后还有 `--json` 打印最终 report，仍追加 `coverage_note`

**不改** plan 生成期的英/混 coverage（uninstall protected 统计、optimize sudo 长尾列表）。

## 2. 触发

`report.skipped_by_reason` 中任一 summary：

- `reason == TccDenied`（含 EndpointSecurity / TCC 类拒绝）
- `reason == NeedsPrivilege`（保护路径 / 系统路径闸口）

任一命中即响亮。仅有 `Whitelisted` / `PathVanished` / `AppRunning` 等不触发。

## 3. 固定文案

```text
注意：部分条目因权限或系统保护被跳过。若涉及用户库数据，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole；系统路径可能需 sudo，请改用 Mole 或具备相应权限的环境后重试。
```

约束：不假装所有 `NeedsPrivilege` 都是 FDA；一句内同时覆盖 FDA 与 sudo/系统保护。

## 4. 架构

```
vole-core::ops::coverage
  APPLY_PERMISSION_WARN: &str
  report_has_permission_skips(report: &Report) -> bool
  coverage_with_apply_permission_hint(base: Option<&str>, report: &Report) -> Option<String>

vole-cli/{uninstall,optimize}.rs
  apply 返回后：
    - json：report.coverage_note = coverage_with_apply_permission_hint(...)
    - human：print_human_report 后若 has_permission_skips → eprintln!(APPLY_PERMISSION_WARN)
    - 避免 human 双重：json 才改 coverage_note 字符串；human 单独打 WARN（与 orphan clean 分通道一致）
```

可选：core apply 不改；组装留在 CLI（与 orphan plan notices 不同，此处以 Report 为唯一信号源）。

## 5. 非目标

- 不改 clean orphan 逻辑
- 不 bump `schema_version` / 不新增 `SkipReason`
- 不改 plan 阶段文案国际化/重写
- 不实现真 sudo

## 6. 测试

- 单测：`report_has_permission_skips` / `coverage_with_apply_permission_hint` 边界
- CLI：若无现成 harness，findings 记手工；至少 core helper 绿

## 7. 验收

1. apply 无权限类 skip → 无警告  
2. 有 `TccDenied` 或 `NeedsPrivilege` → human stderr 有文案；`--json` note 含文案  
3. clean orphan 行为回归不变  
4. CI 绿；发版 `1.4.2`
