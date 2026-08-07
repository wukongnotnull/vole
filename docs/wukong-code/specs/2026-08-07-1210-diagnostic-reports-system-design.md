# 系统 DiagnosticReports 清理设计（system.sh 续刀）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：Mole `system.sh` → `safe_sudo_find_delete "/Library/Logs/DiagnosticReports" "*" "$MOLE_CRASH_REPORT_AGE_DAYS" "f"`（`maxdepth 1`，`MOLE_CRASH_REPORT_AGE_DAYS=7`）；用户域 `diagnostic-reports` 已落地；iconservices / Rosetta Privilege exact 先例
- 包版本意图：**`1.14.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 落地系统崩溃报告叶清理：

- **根**：`/Library/Logs/DiagnosticReports/`
- **仅单层叶**（`maxdepth 1`）；**`older_than_days = 7`**
- **PrivilegeBackend**：前缀下单层叶 allow（非整目录、非嵌套）
- **apply**：`sudo -n` permanent；绑形状谓词 + **年龄重验**（防篡改 plan）
- plan **不**用 sudo 扩扫描（不可读 → 空候选）

**采纳路径**：方案 A — 形状 allow + TOML `older_than_days` + 特权 apply。

## 2. 问题与风险

1. **不可信 plan**：仅靠共享三树 allowlist 会放大提权面（Rosetta Medium 教训）。必须 `is_system_diagnostic_report_leaf`。
2. **年龄闸失效**：plan 入选后文件被更新，或篡改计划指向新文件。apply 用当前 mtime 相对 7 天重验，失败则 skip。
3. **目录 vs 文件**：Mole 只删 `-type f`；apply/plan 拒绝目录叶与更深路径。
4. **废纸篓**：提权写死 permanent。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 叶 + older_than 7（已选）** | 对齐 Mole | 保守 | plan/apply 均要年龄逻辑 |
| B. 整目录 exact | 仿 iconservices | 简单 | 更激进，不齐 Mole |
| C. 叶全清无年龄 | — | — | 偏离 Mole |

## 4. 产品行为

```bash
vole clean --plan                 # 可读则含 ≥7 天叶
sudo -v
vole clean --apply <plan.json>    # sudo -n → permanent；否则 NeedsPrivilege
```

- `rule_id`：`diagnostic-reports-system`
- `category`：`app-caches`（紧邻 `diagnostic-reports`）或 `user-devtools`（与其它系统提权规则邻近；优先 **app-caches** 成对）
- `paths`：`["/Library/Logs/DiagnosticReports/*"]`
- `[rule.strategy] kind = "older_than_days"`，`days = 7`
- 规则数：**519 → 520**；不 bump schema
- 环境：`VOLE_TEST_SYSTEM_LIBRARY` → `$BASE/Logs/DiagnosticReports/`

## 5. 实现

### 5.1 形状谓词

```rust
pub const DIAGNOSTIC_REPORTS_SYSTEM_MARKER: &str = "/Library/Logs/DiagnosticReports/";
// 或 test remap: $BASE/Logs/DiagnosticReports/

pub fn is_system_diagnostic_report_leaf(path: &str) -> bool
```

- normalize 后：含 live 或 mapped marker；suffix 单层、非空、无 `/`
- **不**要求 critical 豁免（`/Library/Logs/...` 非 critical 整树）

### 5.2 Privilege

`path_allowed_for_privilege`：`is_system_diagnostic_report_leaf(s)` → true（与 Rosetta/iconservices 并列）。

plan：`rule_id` 时用 remapped expand（存在的叶），再走 `OlderThanDays(7)`；不可读目录 → 空。

### 5.3 Apply

- 绑 `is_system_diagnostic_report_leaf`
- allowlist
- **年龄重验**：symlink_metadata mtime < now−7d，否则 skip
- probe → permanent sudo；无 unload

常量：`DIAGNOSTIC_REPORTS_SYSTEM_RULE_ID`、`DIAGNOSTIC_REPORTS_SYSTEM_AGE_DAYS = 7`。

## 6. 覆盖说明

- coverage：标明 **系统 DiagnosticReports（≥7 天叶 + sudo -n）已落地**
- 仍未移植：system.sh 其余、交互提权 / 桌面
- README：**520**

## 7. 非目标

- 用户域 `diagnostic-reports` 行为变更
- `/private/var/log`、Install macOS、泛 `/Library/Logs`
- 交互 sudo / 桌面 SMAppService / plan 阶段 sudo
- schema bump / 默认打 tag

## 8. 测试与安全

1. 形状：叶 true；目录/嵌套/其它 Logs false
2. allowlist 接纳叶；三树回归不变
3. plan：force fixture + 旧 mtime 入选；新文件不出选
4. apply：RecordingPrivilege 删旧叶；新叶 / 三树 path + 本 rule_id → skip
5. PR：**security-review**

## 9. 验收

1. Apple Silicon/Intel 上可读目录时可清 ≥7 天叶
2. 未满 7 天与嵌套路径不可经本特权分支删除
3. 规则 520；版本 **1.14.0**（不打 tag）

## 10. 实现后文档

- `docs/releases/v1.14.0.md`、findings
- 后续：`/private/var/log` 窄点 / 交互提权
