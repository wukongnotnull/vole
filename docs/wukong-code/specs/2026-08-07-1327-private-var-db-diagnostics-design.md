# `/private/var/db/diagnostics` 清理设计（system.sh 续刀）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：Mole `system.sh` → `safe_sudo_find_delete "/private/var/db/diagnostics" "*" "$MOLE_LOG_AGE_DAYS"` + `safe_sudo_find_delete … "*.tracev3" "30"`；`safe_sudo_find_delete` 固定 `-maxdepth 5`、`-type f`；`is_private_allowlisted` 已放行该树
- 包版本意图：**`1.16.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 落地 `/private/var/db/diagnostics` 旧诊断文件清理：

- **根**：`/private/var/db/diagnostics`
- **文件**：相对根深度 **1..=5**（对齐 Mole `safe_sudo_find_delete`）
- **类型**：普通文件；无扩展名白名单限制（对齐 Mole 第一刀 `*`），但形状谓词禁止 `..` / 超深 / 根本身
- **年龄（分龄，一规则）**：
  - 非 `.tracev3`：**≥7** 天（`MOLE_LOG_AGE_DAYS`）
  - `.tracev3`：**≥30** 天（对齐 Mole 第二刀意图）
- **Privilege**：形状谓词成立才 allow；apply 绑谓词 + `is_file` + **按扩展名分龄重验** + `sudo -n` permanent
- plan **不** sudo

**采纳**：一规则 `private-var-db-diagnostics` + plan 用 `older_than_days=7` 初筛；apply 对 `.tracev3` 再要求 30d。

**相对 Mole**：Mole 第一刀 `*`/`7` 会先删掉 ≥7d 的 `.tracev3`，使第二刀几乎冗余。Vole **刻意更严**：`.tracev3` 一律 30d（大 volume 友好、对齐注释意图）。

## 2. 问题与风险

1. **两龄重叠**：必须在 apply（及 plan 枚举过滤）显式分龄，禁止「一律 7d」误删较新 `.tracev3`。
2. **不可信 plan**：必须 `is_private_var_db_diagnostics_clean_target`；禁止仅靠三树 / private allowlist。
3. **面较宽**：`*` 无扩展名过滤 → 深度≤5 + 仅文件 + 年龄收窄；禁止目录删除。
4. **非目标边界**：不含 `DiagnosticPipeline` / `powerlog` / memory reports。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 一规则 + apply 分龄（已选）** | 7/30 | 对齐用户批准、单一 id | apply 多分支 |
| B. 两规则 | 各一龄 | 清晰 | 双 TOML / 双 carve-out |
| C. 字面 Mole 一律 7d | 含 tracev3 | 完全一致 | 浪费 30d 意图、易删大文件 |

## 4. 产品行为

```bash
vole clean --plan
sudo -v
vole clean --apply <plan.json>
```

- `rule_id`：`private-var-db-diagnostics`
- `category`：`user-devtools`
- `paths`：`["/private/var/db/diagnostics"]`（候选专用 walk）
- strategy：`older_than_days` / `days = 7`（plan 初筛；`.tracev3` 在 candidates 阶段直接按 30d 过滤更佳）
- 规则数：**521 → 522**
- 测试 remap：`VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` → `$ROOT/private/var/db/diagnostics`

## 5. 实现

### 5.1 谓词

```rust
pub const PRIVATE_VAR_DB_DIAGNOSTICS_LIVE: &str = "/private/var/db/diagnostics";
pub const PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH: usize = 5;
pub const PRIVATE_VAR_DB_DIAGNOSTICS_AGE_DAYS: u32 = 7;
pub const PRIVATE_VAR_DB_DIAGNOSTICS_TRACEV3_AGE_DAYS: u32 = 30;

pub fn is_private_var_db_diagnostics_clean_target(path: &str) -> bool
```

- live / mapped 根下；相对组件 1..=5；无 `..`；叶非空（任意文件名）

### 5.2 Privilege / plan / apply

- `path_allowed_for_privilege`：谓词 true → allow
- `private_var_db_diagnostics_plan_candidates()`：walk ≤5 文件；mtime 按扩展名分龄
- apply：谓词 + file + `age_for_path` + allowlist + probe + permanent

```rust
fn diagnostics_age_days(path: &Path) -> u32 {
    if path.extension().and_then(|e| e.to_str()) == Some("tracev3") {
        PRIVATE_VAR_DB_DIAGNOSTICS_TRACEV3_AGE_DAYS
    } else {
        PRIVATE_VAR_DB_DIAGNOSTICS_AGE_DAYS
    }
}
```

## 6. 覆盖说明

- coverage：标明 **`/private/var/db/diagnostics`（7d / .tracev3 30d + sudo -n）已落地**
- 仍未移植：system.sh 其余（Pipeline/powerlog/Adobe…）、交互提权 / 桌面
- README：**522**

## 7. 非目标

- `/private/var/db/DiagnosticPipeline`、`powerlog`、memory reports
- Adobe `/Library/Logs`、交互提权、schema bump、打 tag

## 8. 测试与安全

1. 谓词：深度 1/5 true，6 false；根外 false
2. apply：旧普通文件删；旧 `.tracev3`(≥30) 删；7–29d `.tracev3` skip；新鲜 skip；三树 + 本 rule_id skip
3. PR：**security-review**

## 9. 验收

1. plan 枚举分龄正确
2. 特权删除仅经本规则形状面
3. 规则 **522**；版本 **1.16.0**（不打 tag）

## 10. 实现后文档

- `docs/releases/v1.16.0.md`、findings
- 后续：DiagnosticPipeline / Adobe logs / 交互提权
