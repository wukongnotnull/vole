# FDA / Library 不可访问的响亮提示设计

- 日期：2026-08-05
- 状态：待审阅
- 依据：CLI 打磨轨（v1.4.0 后）；B4 设计 §7「FDA 不可用时降级跳过并响亮提示」；Mole `clean_orphaned_app_data`（`Skipped: No permission to access Library folders`）
- 包版本意图：行为增强、零协议破坏 → 建议随下一次发版进 **PATCH `1.4.1`**（或与其它打磨合并为 MINOR）；本设计不阻塞独立发版时机

## 1. 结论

当 `orphaned-app-data` 因无法读取 `~/Library/Caches`（或安装扫描失败）而整规则降级时，**不得再静默返回空列表**。须：

1. 对机器读者：emit 既有 `StreamEvent::Skipped { rule_id: "orphaned-app-data", reason: TccDenied }`（**不 bump** `schema_version`）
2. 对人读者：`vole clean --plan` 在非 `--json` / 非 `--json-stream` 时于 **stderr** 输出一行可操作提示（对齐 Mole，并指向 macOS Full Disk Access）

其它 custom handler（Toolbox / Codex / FCP / 剪映 / Claude keep-N）行为不变。

## 2. 问题

现状：

- `select_orphaned_paths` 在 `Library/Caches` 不可读或 `scan_installed_bundle_ids` 失败时返回 `Err(OrphanScanError::LibraryInaccessible)`
- `custom_handlers::orphaned_app_data` 用 `.unwrap_or_default()` **吞掉错误** → plan 上像「没有 orphan」，用户不知道是权限问题
- B4 安全清单曾要求「FDA 不可用时降级跳过并响亮提示」，实现只完成了降级，未完成「响亮」

## 3. 方案对比

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. CustomSelectResult + Skipped 事件 + stderr 文案（推荐）** | `select_custom` 返回 paths + 可选 degraded；plan emit `TccDenied`；CLI 人读加 stderr | 零协议 bump；复用 `SkipReason`；与 guard skip 同通道 | 需改 `select_custom` 签名 |
| B. 只改 `coverage_note` | 文案追加一句 FDA | 改动最小 | plan 当次仍无信号；json-stream 看不到规则级 skip |
| C. 新协议字段 / 新 SkipReason | 如 `FdaDenied` | 语义更准 | 要 bump `schema_version`；消费者成本高 |

**采纳 A。** `TccDenied` 在协议里已表示「权限/TCC 类拒绝」；FDA 属于同一用户可操作类别。不新增 enum 变体。

## 4. 产品行为

### 4.1 触发条件

以下任一成立即视为 orphan 规则 **degraded**（整规则无候选）：

1. `fs::read_dir(~/Library/Caches)` 失败（Mole / B4 探测方式）
2. `OrphanDeps::scan_installed_bundle_ids` 返回 `Err`（不可当成零安装）

不触发：规则 `disabled`、正常扫完零 orphan、单路径 whitelist/保护层 skip（那些仍走既有 per-path 闸口）。

### 4.2 机器可读

- `--json-stream`：plan 期间出现一条  
  `{"type":"skipped","rule_id":"orphaned-app-data","reason":"tcc_denied"}`  
  （具体 JSON 字段名以现有 `StreamEvent::Skipped` / `SkipReason::TccDenied` 序列化为准）
- `--json` plan 文件：`entries` 不含伪候选；**不**把 degraded 塞进 plan entry（避免 apply 误删）
- `coverage_note`：**本里程碑不强制**改写全局 note；degraded 已由事件表达。可选 follow-up：note 脚注 FDA——**非本设计必做**

### 4.3 人读（stderr）

仅当同时满足：

- 命令为 `vole clean --plan`（apply 不做 orphan 全量再扫根目录，degraded 已在 plan 侧留下事件）
- 未使用 `--json` / `--json-stream`
- 本 plan 中 orphan 规则发生了 degraded

输出固定一行（中文，可操作）：

```text
orphaned-app-data 已跳过：无法访问 ~/Library；请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许 Terminal/所用终端或 Vole 后重试。
```

约束：

- 走 **stderr**（stdout 仍留给 plan 表 / 管道）
- 一行、无 spinner、无交互确认
- 英文环境不在本里程碑做 i18n；文案固定中文（与现 CLI 其它 stderr 一致）

### 4.4 apply

本设计**不**新增 apply 阶段的 FDA 文案。若 plan 在无 FDA 时生成（空 entries + 当时有 Skipped 事件），用户应重新 `--plan`。apply 对已有 orphan 条目仍走既有重判；重判失败仍记 skip（既有行为）。

## 5. 架构落点

```
crates/vole-core/src/rules/custom_handlers.rs
  select_custom(...) -> CustomSelectResult { paths, degrade: Option<CustomDegrade> }
  CustomDegrade::LibraryInaccessible  // 仅 orphaned 使用

crates/vole-core/src/ops/plan.rs
  Custom 分支：若 degrade.is_some() → emit Skipped { TccDenied }；paths 仍按空/结果推进

crates/vole-cli/src/clean.rs
  plan 人读路径：收集 Orchestrator 侧 degraded 标志或从事件旁路拿到一次 hint → eprintln!
```

### 5.1 如何把 degraded 传到 CLI（写死推荐）

**方案 A1（推荐）**：`Orchestrator` / `Plan` 增加可选字段：

```rust
pub struct Plan {
    // ...
    /// plan 期间的规则级降级提示（人读；机器侧已用 StreamEvent）
    pub notices: Vec<PlanNotice>,
}

pub enum PlanNotice {
    OrphanLibraryInaccessible,
}
```

`plan_to_proto` **忽略** `notices`（零协议变更）；仅 CLI human 路径消费。

**方案 A2**：CLI 只靠监听 `StreamEvent::Skipped` 中 orphan+TccDenied 再打文案——json-stream 已有事件；人读 plan 若未开 stream，需 Orchestrator 在非 stream 时仍把 notice 挂到 `Plan`。  
→ **必须采用 A1**（或等价：`Plan.notices`），不能只依赖 stream 回调。

### 5.2 `select_custom` 签名

```rust
pub struct CustomSelectResult {
    pub paths: Vec<PathBuf>,
    pub degrade: Option<CustomDegrade>,
}

pub enum CustomDegrade {
    /// orphaned：~/Library/Caches 不可读或安装扫描失败
    LibraryInaccessible,
}

pub fn select_custom(...) -> CustomSelectResult
```

非 orphan handler：`degrade: None`，`paths` 与今天相同。

`orphaned_app_data`：`Ok` → paths；`Err(LibraryInaccessible)` → `paths: []` + `Some(LibraryInaccessible)`。

### 5.3 SkipReason 映射

`CustomDegrade::LibraryInaccessible` → `SkipReason::TccDenied`。

理由：协议已有；消费者已认识；FDA/TCC 均为「授权后重试」类。不引入新 reason。

## 6. 非目标（写死）

- 不实现真 sudo / 系统域 orphan
- 不新增 `SkipReason` / 不 bump `schema_version`
- 不打开「系统设置」深链（macOS 版本差异大）；文案只描述路径
- 不改 uninstall / optimize 的权限文案（可列为 CLI 打磨下一刀）
- 不改 Mole 兼容 JSON plan schema 增加字段
- 不对每个不可读的普通规则路径统一改文案（仅 orphan 整规则降级）

## 7. 测试策略

| 层 | 内容 |
|---|---|
| 单测 `custom_handlers` / orphan select | Library 不可读 → `degrade = Some(LibraryInaccessible)`，paths 空 |
| 单测 `plan` | Fake deps `scan_error` 或无可读 Caches；capture events；存在 orphan+TccDenied；entries 无 orphan 候选 |
| 单测 `Plan.notices` | degraded 时含 `OrphanLibraryInaccessible`；正常 orphan 选出时 notices 不含该项 |
| CLI（可选轻测或手工） | human plan 在 fixture HOME 无 Caches 权限模拟下 stderr 含关键字 `完全磁盘访问权限` |

CI：继续注入 `FakeOrphanDeps`，不依赖真机 FDA。

## 8. 安全与兼容

- 不降低删除闸口；degraded 时**更少**删除候选
- 静默空洞消失，降低「以为扫过、其实没权限」的误操作风险
- json / stream 消费者：多一条可能的 `skipped` 事件；属向后兼容追加行为，非破坏

## 9. 验收

1. 设计无占位符；`TccDenied` 映射写死  
2. FDA/扫描失败时有 `Skipped` 事件且有 `Plan.notices`  
3. human `--plan` stderr 出现指定文案；`--json` / `--json-stream` 不刷该中文行到 stdout  
4. 其它 custom handler 回归绿  
5. 合并前 CI 绿；发版说明在 PATCH/MINOR 发版笔记中提一句「orphaned FDA 响亮提示」

## 10. 下一步

审阅通过后写实施计划：`docs/wukong-code/plans/YYYY-MM-DD-HHmm-orphan-fda-loud-hint.md`，再按任务实施。
