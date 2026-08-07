# `/private/var/log` 旧日志清理设计（system.sh 续刀）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：Mole `system.sh` 系统日志段 → `safe_sudo_find_delete "/private/var/log" "*.log|*.gz|*.asl"`（`MOLE_LOG_AGE_DAYS=7`）；探测 `find` 用 `-maxdepth 3`，**实际删除** `safe_sudo_find_delete` 内为 `-maxdepth 5`；`is_private_allowlisted` 已放行该树；PrivilegeBackend 尚不含此面
- 包版本意图：**`1.15.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 落地 `/private/var/log` 旧日志文件清理：

- **根**：`/private/var/log`
- **文件**：相对根 **深度 1..=5**（对齐 Mole **删除** maxdepth，非仅探测的 3）
- **扩展名**：`.log` / `.gz` / `.asl`（后缀匹配，小写）
- **年龄**：`older_than_days = 7`
- **Privilege**：形状谓词成立才 allow；apply 绑谓词 + `is_file` + 年龄重验 + `sudo -n` permanent
- plan **不** sudo（不可读 → 空）

**采纳路径**：方案 A — 形状/扩展名/深度谓词 + 特权 apply（仿 DiagnosticReports）。

## 2. 问题与风险

1. **探测 vs 删除深度不一致**：Mole 探测 maxdepth 3、删除 5。Vole **写死 5**，与真删对齐；文档注明差异。
2. **不可信 plan**：必须 `is_private_var_log_clean_target`，禁止仅靠 `is_private_allowlisted` 或三树 allowlist。
3. **年龄**：apply 用当前 mtime 相对 7 天重验。
4. **symlink**：仅普通文件（plan/apply 拒绝 symlink / 目录）。
5. **提权面**：比单路径 Rosetta 宽（整棵 log 树下过滤文件）；用扩展名+深度收窄。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. maxdepth 5 + 三扩展名 + 7d（已选）** | 对齐 Mole 删除 | 一致 | 面比 exact 宽 |
| B. maxdepth 3 | 对齐探测 | 更窄 | 比 Mole 真删少清 |
| C. 整树 exact | — | — | 过激 |

## 4. 产品行为

```bash
vole clean --plan
sudo -v
vole clean --apply <plan.json>
```

- `rule_id`：`private-var-log`
- `category`：`user-devtools` 或新建紧邻其它系统特权规则；推荐 **user-devtools**（与 rosetta/icon 特权规则邻近）
- `paths`：占位 `["/private/var/log"]`（实际候选由 plan 专用列举，不依赖 `*` 跨深度）
- strategy：`older_than_days` / `days = 7`（对 enumerated 文件再滤）
- 规则数：**520 → 521**
- 测试 remap：设 `VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` 时，根为 `$ROOT/private/var/log`（`Library` 的 parent + `private/var/log`）

## 5. 实现

### 5.1 谓词

```rust
pub const PRIVATE_VAR_LOG_LIVE: &str = "/private/var/log";
pub const PRIVATE_VAR_LOG_AGE_DAYS: u32 = 7;
pub const PRIVATE_VAR_LOG_MAX_DEPTH: u32 = 5;

pub fn is_private_var_log_clean_target(path: &str) -> bool
```

- 落在 live 或 mapped 根之下
- 相对路径组件数 ∈ 1..=5，无 `..`
- 文件名以 `.log` / `.gz` / `.asl` 结尾
- （路径层）不要求存在 / 是文件；**apply** 再验 `is_file`

### 5.2 Privilege / plan / apply

- `path_allowed_for_privilege`：谓词 true → allow
- `private_var_log_plan_candidates()`：walk 根 maxdepth 5 收集满足扩展名的文件
- apply：`is_private_var_log_clean_target` + file + age + allowlist + probe + permanent

## 6. 覆盖说明

- coverage：标明 **`/private/var/log` 旧日志（sudo -n）已落地**
- 仍未移植：system.sh 其余、交互提权 / 桌面
- README：**521**

## 7. 非目标

- Adobe `/Library/Logs`、`/private/var/db/*`、`/private/tmp`
- 交互 sudo / 桌面 / plan 阶段 sudo
- schema bump / 默认打 tag

## 8. 测试与安全

1. 谓词：合法深度/扩展名 true；深度 6、错误扩展名、根外 false
2. apply：旧 `.log` 删除；新鲜 skip；三树 path + 本 rule_id skip
3. PR：**security-review**

## 9. 验收

1. 可读时 plan 含 ≥7 天目标扩展名文件（深度 ≤5）
2. 特权删除仅经本规则形状面
3. 规则 521；版本 **1.15.0**（不打 tag）

## 10. 实现后文档

- `docs/releases/v1.15.0.md`、findings
- 后续：`/private/var/db/diagnostics` 等 / 交互提权
