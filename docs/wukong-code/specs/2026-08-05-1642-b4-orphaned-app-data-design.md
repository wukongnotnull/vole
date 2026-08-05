# B4：Orphaned App Data 设计

- 日期：2026-08-05（同日审阅修订：apply 重判定位为新机制、Spotlight fail-closed、执行预算、注入要求、去重顺序、env 命名对齐）
- 状态：草案（已修订，待终审）
- 依据：`2026-07-30-v1-closeout-design.md` §5 B4；`2026-07-30-1900-v2-product-goals-design.md` §4.3；Mole `third_party/mole-1.48.1/lib/clean/apps.sh`（`clean_orphaned_app_data` / `is_bundle_orphaned` / `scan_installed_apps`）；`SECURITY_AUDIT.md` orphan 相关条款
- 包版本意图：能力扩展 → **`1.3.0`**（SemVer MINOR；见 `2026-07-30-semver-policy-design.md`）

## 1. 结论

在 **`vole clean` plan→apply** 路径上交付与 Mole 主路径对齐的 **用户域 orphaned app data** 清理：对「已安装 app 清单中不存在、且数据目录 mtime ≥ 30 天」的 `~/Library/{Caches,Logs,Saved Application State}` 条目产出 plan 候选，apply 默认进废纸篓。

**不做（本里程碑）**：system services orphan（需 sudo）、Containers / Group Containers / LaunchAgents / Application Scripts、orphan dotdir 只读 hint、`purge`、真提权。Claude VM orphan 列为 **B4.1 可选第二刀**（同函数、同安全闸口），不阻塞 B4.0 主路径验收。

## 2. 问题与风险

Orphan 判定是启发式：mtime + 安装扫描 + Spotlight（mdfind）回退。误判会删掉「仍在用但安装位置非常规 / Spotlight 瞬时失败」的应用数据。Mole 已踩过 mdfind timeout 被当成「未安装」的坑（fail-closed）。Vole 必须把同等闸口做成可测、默认可恢复、可禁用。

## 3. 方案对比（已选推荐）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. clean 内建 custom 扫描（推荐）** | 新增 TOML 规则 + `custom` handler id `orphaned_app_data`，融入既有 `vole clean --plan/--apply` | 复用保护层 / 废纸篓 / oplog / 不可信 plan 闸口；与 Toolbox 等 custom 一致 | plan 扫描变慢，需预算与超时 |
| B. 独立子命令 `vole orphaned` | 新 CLI 面 | 心智隔离 | 重复 plan/apply 基建；菜单/补全/协议消费者成本高 |
| C. 只读 hint 先行 | 仅报告不删 | 风险最低 | 不兑现 B4「清理」价值；用户仍要靠 Mole |

**采纳 A。** 规则默认可启用；用户可用既有规则禁用机制关掉。

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                 # 候选中可含 orphaned 条目（rule_id 见下）
vole clean --apply <plan.json>    # 默认废纸篓；--permanent 仅与 --apply
```

- 人读摘要：条目 label 形如 `Orphaned Caches: com.example.app`
- `--json` / `--json-stream`：与现 clean 事件一致；`coverage_note` 更新为「orphaned 用户域主路径已落地；system services / Containers 等仍未移植」
- 环境变量（沿用 Mole 原名，与既有 custom `MOLE_JETBRAINS_TOOLBOX_KEEP` 惯例一致）：
  - `MOLE_ORPHAN_AGE_DAYS`（默认 **30**；解析失败或 **< 7** 一律回退 30——阈值下限防止 `0` 让当天数据过闸）
  - 不引入 enable 新开关；禁用走规则 `disabled = true`

### 4.2 规则与 `rule_id`

- TOML 规则 id：`orphaned-app-data`
- plan 条目：`rule_id = "orphaned-app-data"`；每条一个绝对路径；`label` 含资源类型 + bundle id
- **不 bump** `schema_version`（零协议破坏；靠既有 Plan 字段）

### 4.3 扫描根（B4.0 必做）

仅下列用户域（Mole `resource_types` 主循环；**禁止**扩大）：

| 根 | 匹配 |
|---|---|
| `$HOME/Library/Caches` | `com.*` / `org.*` / `net.*` / `io.*` 顶层名 |
| `$HOME/Library/Logs` | 同上 |
| `$HOME/Library/Saved Application State` | `*.savedState`（strip 后缀得 bundle id） |

前缀集合**刻意**只含 com/org/net/io（对齐 Mole `resource_types`）：`dev.*` / `app.*` / `co.*` 等孤儿数据不扫，属已知保守取舍，不是遗漏。零尺寸条目跳过（对齐 Mole `size_kb == 0` 分支）。

**明确永不扫描（Mole CRITICAL 注释，本设计写死）**：

- `~/Library/LaunchAgents` / LaunchDaemons
- `~/Library/Containers`
- `~/Library/Application Scripts`
- `~/Library/Group Containers`（TeamID 前缀假阳性）
- `/Library/**`（系统域；无 sudo 代际）

### 4.4 B4.1（可选，不阻塞 1.3.0）

- Claude Desktop workspace VM：`$HOME/Library/Application Support/Claude/**/*.bundle`，年龄默认 **7** 天（`MOLE_CLAUDE_VM_ORPHAN_AGE_DAYS`，沿用 Mole 原名），且 Claude 进程 / bundle 安装检查对齐 Mole `is_claude_vm_bundle_orphaned`
- 若 B4.1 未进同一发版：`coverage_note` 仍可提「Claude VM orphan 未做」

## 5. 判定流水线（`is_bundle_orphaned` 对齐）

对候选路径抽出的 `bundle_id`，**全部通过**才标为 orphan（任一步失败 → 跳过，fail-closed）：

1. **`should_protect_data(bundle_id)`**（既有 `vole-core` 保护目录）→ 否
2. **敏感族 glob**（对齐 `ORPHAN_NEVER_DELETE_PATTERNS`：1Password / Bitwarden / Keychain / ssh / gpg 等）→ 否
3. **已安装 / 活跃集合**含该 id → 否  
   集合来源（并集、去重）：
   - 目录扫描：`/Applications`、`/System/Applications`、`$HOME/Applications`、Homebrew Caskroom、Setapp Applications（maxdepth 对齐 Mole）
   - 运行中：优先 **`lsappinfo`**（避免 osascript TCC 弹窗）；可选补充既有 process probe；测试模式跳过 AppleScript
   - 用户域 + `/Library/LaunchAgents` 的 plist basename（仅作「仍活跃」证据，**不**作为删除目标）
4. **系统组件 deny 名**（loginwindow / dock / finder / safari 等 Mole case）→ 否
5. **年龄**：路径 mtime 距今 `< MOLE_ORPHAN_AGE_DAYS`（默认 30，下限 7）→ 否
6. **mdfind 回退**（仅 reverse-DNS bundle id）：`kMDItemCFBundleIdentifier == '<id>'`  
   - **前置**：先探测 Spotlight 可用性（每次 plan 一次，如 `mdutil -s /` 或对已知存在 bundle 校准）；不可用 → 整个 mdfind 分支 fail-closed，**该 bundle 视为仍安装**（Mole 在 Spotlight 关闭时会把空结果当「未安装」，Vole 不复制这个弱点）
   - 超时 / 非零退出 → 视为仍安装，跳过，且不写入「未找到」缓存  
   - 有命中 → 跳过  
   - 明确空结果（且 Spotlight 可用）→ 才可继续
7. **白名单**（`crate::whitelist`）→ 否
8. **路径闸口**：`validate_path_for_deletion` + `ProtectionMode::Cleanup`；删除只经既有 `mole_delete_verified`（默认 Trash）

### 5.1 执行预算（写死）

- mdfind / lsappinfo 子进程一律有超时（对齐 Mole medium probe ≈ 10s；vole-core 无通用 timeout helper，按 `scan/du.rs` 模式实现）
- 每次 plan 的 mdfind 调用上限 **64** 次（进程内缓存去重后）；超限的 bundle 一律视为仍安装
- 安装扫描**不做磁盘缓存**（砍掉 Mole 的 300s cache file）：进程内单次扫描，plan / apply 各自扫。目录权限失败 → 宁可整体跳过 orphan 判定，不可当成「零安装」

## 6. 架构落点

```
crates/vole-core/src/orphan/          # NEW：扫描 + 判定（纯库，可单测）
  mod.rs
  installed.rs                        # scan_installed_apps + running + agents
  judge.rs                            # is_bundle_orphaned
  scan.rs                             # 三根目录枚举 → PathBuf 列表
crates/vole-core/src/rules/custom_handlers.rs  # 注册 handler id `orphaned_app_data`
crates/vole-core/src/ops/apply_plan.rs         # NEW：orphaned 条目 apply 重判钩子（§6.2）
data/rules/<category>/orphaned-app-data.toml   # NEW：[rule.strategy] kind="custom" handler="orphaned_app_data"
crates/vole-core/src/ops/coverage.rs           # 文案更新
tests/fixtures/orphaned/                       # NEW：假 HOME + 假安装树
docs/releases/v1.3.0.md                        # 发版说明
README.md                                      # 特性 / 与 Mole 对比更新
```

`docs/protocol.md` 无需变更（零协议改动）。

### 6.1 plan 接线（贴合现有 `select_custom` 流程）

规则 `paths` 直接声明三根 glob（`~/Library/Caches/*` 等）；`select_custom("orphaned_app_data", entries, home, rule)` 收到 glob 展开后的 entries，对每条跑 judge 过滤——与 Toolbox / Codex handler 同模式，fixture 机制可直接复用。**不**引入「handler 自行枚举」新模式。

**规则顺序**：orphaned-app-data 必须在全部具名规则**之后**评估（B3 同路径去重「先入选者胜」，具名规则的 label/策略优先）。加载序保证 + 单测锁定。

### 6.2 apply 重判（新增机制，现状没有）

现状 `apply_plan.rs` 只重查 guards + 路径闸口，**不会**重跑 custom handler。本设计新增：按 `rule_id == "orphaned-app-data"` 的条目在 apply 时重新跑完整 judge（含安装扫描 + mdfind fail-closed），重判失败 → skip + 记入 report。实现为 apply 侧的 handler-recheck 钩子（仅此规则注册，不改协议）。

注：plan 条目若被篡改成其他 `rule_id`，走的是该规则的常规闸口（既有信任模型，Cleanup 模式保护路径仍挡）；本机制只保证「挂 orphaned 名义的删除必过 orphan 重判」。

### 6.3 可注入性（测试与 CI 决定性）

`orphan::scan` / `installed.rs` 的全部外部依赖必须可注入：扫描根、安装目录列表、运行中探针（复用既有 `ProcessProbe` 思路）、mdfind 探针、Spotlight 可用性。fixture / CI（含 conformance plan-stage）一律用注入的假环境，**不得**读真实 `/Applications`、不得起 mdfind / lsappinfo 子进程——否则 CI 结果随 runner 状态漂移。

**不**复用 `uninstall` 的 `find_app_leftovers` 做 orphan 发现：uninstall 从「已知 .app」扩残留；orphan 从「Library 条目」反查「无 .app」。可共享：bundle id 校验、`should_protect_data`、删除漏斗、sibling 概念不用于 orphan 主路径（无 app 身份）。

## 7. 安全评审清单（合并前必勾）

- [ ] 扫描根集合与 §4.3 完全一致；代码注释复述 NEVER 列表
- [ ] mdfind 超时 / 错误 fail-closed 有单测
- [ ] **Spotlight 不可用（mdutil off）时不产生 orphan 误判**有单测
- [ ] 敏感族 + `should_protect_data` 有单测（至少 1Password / com.apple.*）
- [ ] apply 重判：plan 篡改「缩短年龄 / 伪造 rule」不能删过闸路径
- [ ] 规则顺序单测：同路径下具名规则胜出 orphaned
- [ ] `MOLE_ORPHAN_AGE_DAYS` 下限 clamp 有单测（0 / 负值 / 非数字 → 30）
- [ ] 默认 Trash；`--permanent` 仅 apply
- [ ] 无 sudo；无 `/Library` 删除
- [ ] 无 Group Containers / Containers / LaunchAgents 删除
- [ ] FDA 不可用时降级跳过并响亮提示（探测方式对齐 Mole：读 `~/Library/Caches` 失败即降级）
- [ ] 迭代上限（对齐 `MOLE_MAX_ORPHAN_ITERATIONS` 精神）+ mdfind 调用上限 64，防止异常目录拖死
- [ ] CI / conformance 全程注入假环境，无真机探针子进程
- [ ] 独立 findings：`docs/findings/2026-08-b4-orphaned-security-review.md`

## 8. 测试策略

| 层 | 内容 |
|---|---|
| 单测 | judge 各闸口；mdfind 失败不删；年龄边界；敏感 glob |
| Fixture | 假 `$HOME`：无对应 .app 的旧 Cache → 入选；有 .app / 新 mtime / 保护 bundle → 不入选 |
| CLI | `vole clean --plan` 在 fixture 下出现 `orphaned-app-data`；apply dry 路径走 Trash mock / 现有 delete 测试床 |
| 回归 | 既有 clean / uninstall / optimize 套件不红 |

不在 CI 跑真实删机上 Library；不把 apply-stage conformance 扩到真删。

## 9. 非目标（写死）

- `clean_orphaned_system_services` / PrivilegedHelperTools
- `clean_orphaned_container_stubs`
- orphan dotdir hint（`hints.sh`）
- Spotlight orphan rules（属 optimize 长尾）
- TeamID / 厂商前缀通配扩大匹配
- 将 Application Support 泛扫描纳入 B4.0（仅 B4.1 Claude VM 特例）

## 10. 里程碑与版本

| 步 | 内容 | 版本 |
|---|---|---|
| B4.0 | 本设计 + 实现 + 安全 findings + 发版 | **1.3.0** |
| B4.1 | Claude VM orphan（可选） | 1.3.x 或 1.4.0 |
| 另轨 | system services orphan | 需单独 sudo/提权设计，不挂 B4 |

## 11. 验收

1. 设计无占位符；NEVER 列表与 Mole CRITICAL 一致  
2. `coverage_note` 不再把用户域 orphaned 标为「仍未移植」  
3. README「与 Mole 对比」可写 orphaned 用户域主路径已支持，并诚实写清未做项  
4. §7 清单全部勾选后才打 `v1.3.0` tag  

## 12. 下一步

设计审阅通过后 → `writing-plans` 产出 `docs/wukong-code/plans/2026-08-05-*-b4-orphaned-app-data.md`，再按 task 实现。
