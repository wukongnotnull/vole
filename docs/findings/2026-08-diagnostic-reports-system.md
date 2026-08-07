# 系统 DiagnosticReports（1.14.0）

Mole `safe_sudo_find_delete …/DiagnosticReports`（mtime +7，maxdepth 1，type f）。

## 落点

- `is_system_diagnostic_report_leaf`：单层叶形状（+ test remap）
- Privilege allow + plan 列文件叶；`older_than_days = 7`
- apply：形状 + `is_file` + 年龄重验 + `sudo -n` permanent

## 安全

- 不接受目录根 / 嵌套 / 其它 Logs
- 新鲜叶 apply 必 skip
