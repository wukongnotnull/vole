# FDA / Library 不可访问的响亮提示设计

- 日期：2026-08-05（同日：设计评审修订）
- 状态：已实现（2026-08-05）
- 实现计划：[`../plans/2026-08-05-1923-orphan-fda-loud-hint.md`](../plans/2026-08-05-1923-orphan-fda-loud-hint.md)
- 验收：[`../../../findings/2026-08-orphan-fda-loud-hint.md`](../../../findings/2026-08-orphan-fda-loud-hint.md)
- 依据：CLI 打磨轨（v1.4.0 后）；B4 设计 §7「FDA 不可用时降级跳过并响亮提示」；Mole `clean_orphaned_app_data`（`Skipped: No permission to access Library folders`）；`docs/protocol.md`（允许追加 `SkipReason` 变体；本设计**不**追加）
- 包版本意图：行为增强、零协议破坏 → 建议随下一次发版进 **PATCH `1.4.1`**（或与其它打磨合并为 MINOR）；本设计不阻塞独立发版时机

## 1. 结论

当 `orphaned-app-data` 因无法读取 `~/Library/Caches`（或安装扫描失败）而整规则降级时，**不得再静默返回空列表**。须在三条输出通道同时有信号：

1. **NDJSON / stream**：emit 既有 `StreamEvent::Skipped { rule_id: "orphaned-app-data", reason: TccDenied }`（**不 bump** `schema_version`，**不**新增 `SkipReason`）
2. **人读 `--plan`**：stderr 一行可操作提示
3. **`--json` plan（及 `--plan-out`）**：把一句警告**追加**进**当次** `coverage_note`（现有可选字段，零 schema bump），使脚本用户在只读 plan JSON 时也能发现降级

其它 custom handler（Toolbox / Codex / FCP / 剪映 / Claude keep-N）行为不变。

## 2. 问题

现状：

- `select_orphaned_paths` 在 `Library/Caches` 不可读或 `scan_installed_bundle_ids` 失败时返回 `Err(OrphanScanError::LibraryInaccessible)`
- `custom_handlers::orphaned_app_data` 用 `.unwrap_or_default()` **吞掉错误** → plan 上像「没有 orphan」，用户不知道是权限问题
- B4 安全清单曾要求「FDA 不可用时降级跳过并响亮提示」，实现只完成了降级，未完成「响亮」

## 3. 方案对比

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. CustomSelectResult + Skipped + stderr + 当次 coverage_note 追加（推荐）** | `select_custom` 返回 degrade；emit `TccDenied`；人读 stderr；**json plan 的 coverage_note 追加警告** | 三条通道全覆盖；零协议 bump | 需改 `select_custom` / `Plan` |
| B. 只改全局静态 `coverage_note()` | 文案永久加一句 FDA | 改动最小 | 无权限时仍无「当次」信号；有权限时误导 |
| C. 新 `SkipReason::FdaDenied` | 协议追加变体 | 语义最准 | 消费方要认新字符串；本里程碑收益不足（协议虽允许追加，但本设计刻意复用 `TccDenied`） |

**采纳 A。**

### 3.1 设计评审已锁定的决策

| 项 | 结论 |
|---|---|
| 纯 `--json` 如何响亮 | **必须**追加进当次 plan `coverage_note`（评审选择） |
| 是否新增 `FdaDenied` | **否**；复用 `TccDenied`，并在 §5.3 写清语义外延 |
| Caches 不可读 vs 安装扫描失败 | **同一 degrade 枚举**；用户文案用中性「无法访问用户 Library / 安装扫描失败」，**不**对扫描失败独断写成「请开 FDA」（避免误导） |

## 4. 产品行为

### 4.1 触发条件

以下任一成立即视为 orphan 规则 **degraded**（整规则无候选）：

1. `fs::read_dir(~/Library/Caches)` 失败（Mole / B4 探测方式）
2. `OrphanDeps::scan_installed_bundle_ids` 返回 `Err`（不可当成零安装）

不触发：规则 `disabled`、正常扫完零 orphan、单路径 whitelist/保护层 skip（那些仍走既有 per-path 闸口）。

### 4.2 机器可读

- `--json-stream`：plan 期间出现一条 `skipped`，`reason` 序列化为 `tcc_denied`（`#[serde(rename_all = "snake_case")]`）
- `--json` / `--plan-out`：`entries` 不含伪候选；**当次** `coverage_note` 在原有全局说明后追加固定警告句（见 §4.3.1）
- 全局静态函数 `coverage_note(enabled_rules)` **仍不**永久写死 FDA；追加只发生在「本 plan 发生 degraded」时由 CLI / plan 组装层完成

### 4.3 人读（stderr）

仅当同时满足：

- `vole clean --plan`
- 未使用 `--json` / `--json-stream`
- 本 plan 中 orphan 规则 `degraded`

**输出顺序（写死）**：

1. stdout：既有 plan 表（`print_human_plan` 主体）
2. stderr：空行 + 既有全局 `coverage` 文案（现状）
3. stderr：再输出一行 FDA/权限警告（§4.3.1）

不在 plan 表之前插警告，避免打乱现有视觉习惯。

### 4.3.1 固定警告文案（人读 stderr 与 json `coverage_note` 追加共用同一句）

```text
注意：orphaned-app-data 已跳过（无法读取 ~/Library/Caches 或安装扫描失败）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。
```

约束：

- 人读走 **stderr**；json 走 `coverage_note` 字符串拼接（stdout 仍是完整 plan JSON）
- 一行；无 spinner、无交互、无深链
- 文案固定中文；本里程碑不做 i18n
- 对「扫描失败」不假装一定是 FDA，但保留 FDA 操作指引作为常见修复路径

### 4.4 apply

本设计**不**新增 apply 阶段的 FDA 文案。用户应在授权后重新 `--plan`。apply 对已有 orphan 条目仍走既有重判。

## 5. 架构落点

```
crates/vole-core/src/rules/custom_handlers.rs
  select_custom(...) -> CustomSelectResult { paths, degrade: Option<CustomDegrade> }
  CustomDegrade::LibraryInaccessible

crates/vole-core/src/ops/plan.rs
  Plan { ..., notices: Vec<PlanNotice> }
  Custom 分支：degrade → emit Skipped{TccDenied} + push PlanNotice::OrphanLibraryInaccessible

crates/vole-cli/src/clean.rs
  若 plan.notices 含 OrphanLibraryInaccessible：
    - 组装 coverage_note = base + "\n" + WARN
    - human：在 print_human_plan 的 coverage 之后再 eprintln!(WARN)
    - json / plan-out：proto.coverage_note = 组装后的字符串
```

### 5.1 `Plan.notices`（写死）

```rust
pub struct Plan {
    pub generated_at: SystemTime,
    pub ttl: Duration,
    pub entries: Vec<PlanEntry>,
    pub notices: Vec<PlanNotice>,
}

pub enum PlanNotice {
    OrphanLibraryInaccessible,
}
```

- `plan_to_proto` **忽略** `notices`（proto Plan 无此字段）
- CLI 是 `notices` 的唯一产品消费者；机器侧并行依赖 `Skipped` 事件与 `coverage_note` 追加

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

非 orphan handler：`degrade: None`。

### 5.3 SkipReason 映射与语义

`CustomDegrade::LibraryInaccessible` → `SkipReason::TccDenied`。

**语义外延（写进实现注释 + findings）**：本变体在 clean plan 中表示「权限/TCC 类或同类不可访问导致的规则级跳过」，不仅限于既有 `EndpointSecurityCache` 路径校验。协议允许日后追加 `FdaDenied`；本里程碑不追加。

## 6. 非目标（写死）

- 不实现真 sudo / 系统域 orphan
- 不新增 `SkipReason` / 不 bump `schema_version`
- 不打开「系统设置」深链
- 不改 uninstall / optimize 的权限文案
- 不改 Mole 兼容 JSON plan **字段集**（只复用已有 `coverage_note`）
- 不对每个不可读的普通规则路径统一改文案（仅 orphan 整规则降级）

## 7. 测试策略

| 层 | 内容 |
|---|---|
| `select_custom` / orphan | Caches 不可读 / `scan_error` → `degrade = Some(...)`，paths 空 |
| `plan` | capture events：orphan + `TccDenied`；`notices` 含 `OrphanLibraryInaccessible`；entries 无 orphan |
| CLI / 组装 | degraded 时最终 `coverage_note` **包含**警告关键字；未 degraded 时**不含**该警告 |
| 回归 | 其它 custom handler；正常 orphan 选出时无 `TccDenied` orphan skip、无 notice |

CI：注入 `FakeOrphanDeps`，不依赖真机 FDA。

## 8. 安全与兼容

- 不降低删除闸口；degraded 时更少候选
- `--json` 消费者：`coverage_note` 变长但字段兼容；多一份信息属向后兼容
- stream 消费者：多一条可能的 `skipped`；兼容

## 9. 验收

1. 三条通道（stream / human stderr / json `coverage_note`）在 degraded 时均有信号  
2. 未 degraded 时不出现警告句  
3. `--json` / `--json-stream` 不把中文警告刷到 **stdout 的非 coverage 位置**（stream 的 skipped 事件除外）  
4. 文案不声称扫描失败「一定是」FDA  
5. 其它 custom handler 回归绿；CI 绿  
6. PATCH/MINOR 发版说明提一句「orphaned 权限降级响亮提示」

## 10. 设计评审摘要（本次）

| 严重度 | 原问题 | 处理 |
|---|---|---|
| Important | 纯 `--json` 仍静默，与「响亮」目标矛盾 | **已改**：当次 `coverage_note` 追加 |
| Important | 扫描失败文案硬绑 FDA 会误导 | **已改**：中性触发描述 + FDA 为可选修复指引 |
| Important | `TccDenied` 语义 overloaded | **接受并写死外延**；不新增 enum |
| Minor | stderr 与 coverage 输出顺序未定义 | **已写死** §4.3 |
| Minor | Mole 只 probe Caches，文案写 Library | **已改为**触发写 Caches，指引仍提 FDA |

## 11. 下一步

审阅通过修订稿后写实施计划：`docs/wukong-code/plans/YYYY-MM-DD-HHmm-orphan-fda-loud-hint.md`，再实施。
