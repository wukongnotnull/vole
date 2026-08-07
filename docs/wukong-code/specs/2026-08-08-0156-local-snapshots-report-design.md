# 本地快照报告设计（W1）

- 日期：2026-08-08
- 状态：已批准（路线图写死 + 默认采纳 Condensed）
- 依据：[`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md) §2.2 W1；[`2026-08-08-0025-mole-system-sh-backlog-design.md`](2026-08-08-0025-mole-system-sh-backlog-design.md) §3.2；Mole `clean_local_snapshots`（`third_party/mole-1.48.1/lib/clean/system.sh`，仅 `listlocalsnapshots`）
- 包版本意图：**1.29.0**（MINOR）；规则数 **不变**（无新 clean 规则）

## 1. 结论

对齐 Mole `clean_local_snapshots` 的 **报告面**：

- 调用 `tmutil listlocalsnapshots /`，解析 `com.apple.TimeMachine.YYYY-MM-DD-HHMMSS` 数量
- 数量 &gt; 0 时提示 review：`tmutil listlocalsnapshots /`
- **禁止** `tmutil deletelocalsnapshots`；**禁止** 进入 `clean --apply` / plan 候选
- 优先挂 `vole status`（TUI + JSON）；可选进 `analyze` 提示行
- 可注入 deps；`tmutil` 失败或超时 → **不报假数据**（fail-closed，静默 / skip，不 invent count）

## 2. 问题与风险

1. **误报数量**：`listlocalsnapshots` 失败 / 超时 / 空输出 → `None` / quiet，禁止填 0 冒充「已查且无快照」以外的假成功叙事（与 Mole：空则 return 一致；失败同为空）。
2. **备份进行中干扰**：对齐 Mole 门控——`tmutil` 缺失、AutoBackup 非 `{0,1}`、Running / Unknown → 不列出假数量；Running / Unknown 可报 skip 文案（不是假 count）。
3. **误删进 clean**：本刀零 plan/apply 接线；W3「删本地快照」永不做。
4. **并行冲突**：不改 `tmbackup`、尽量只动 coverage「仍未移植」句中「本地快照报告」；规则数文案不动。
5. **协议**：`StatusSnapshot` 追加 **可选** 字段（`skip_serializing_if`）；不 bump `schema_version`；包版本 MINOR **1.29.0**。

## 3. 方案（已选）

| 方案 | 做法 | 结论 |
|---|---|---|
| **A. `localsnapshots` 模块 + status 字段（已选）** | 独立 deps；status 采集附加 tip；analyze 可选同一函数 | 低耦合；可测；不碰 clean |
| B. 塞进 `tmbackup` | 复用 TmDeps | 与可删路径缠在一起，拒绝 |
| C. 仅 coverage 文案 | 无运行时行为 | 不兑现 W1 |

**Condensed 要点（≤5）：**

1. 新模块 `vole-core::localsnapshots`，`LocalSnapshotDeps` 可注入。
2. 输出：`Quiet` / `Present { count }` / `SkippedBusy` / `SkippedUnknown`。
3. `StatusCollector` 全量/快采均可附加（短超时）；TUI 多一行 tip；JSON optional 字段。
4. `analyze` TUI/文本可选用同一 `format_tip`（不改 Analyze 核心协议亦可）。
5. coverage 去掉「本地快照报告」；仍保留「桌面 SMAppService…」；版本 1.29.0。

## 4. 产品行为

```text
vole status          # TUI：有 Present/Skip 时显示 tip 行
vole status --json   # 可选 local_snapshots 对象；quiet 时字段省略
vole analyze …       # 可选：底部/旁注同一 tip（非阻断）
vole clean --apply   # 不得删除本地快照（本刀零接线）
```

文案对齐 Mole（英文）：

- Present：`Time Machine local snapshots · {N} (review: tmutil listlocalsnapshots /)`
- SkippedUnknown：`Snapshot check · skipped (Time Machine status unknown)`
- SkippedBusy：`Snapshot check · skipped (backup in progress)`

## 5. 采集逻辑（写死）

1. `!tmutil_exists` → Quiet  
2. `!auto_backup_configured`（AutoBackup 非 0/1）→ Quiet  
3. `running_state == Unknown` → SkippedUnknown  
4. `running_state == Running` → SkippedBusy  
5. `listlocalsnapshots("/")`：命令失败 / 超时 / 非零退出 → Quiet（不报假数）  
6. 用正则 `com\.apple\.TimeMachine\.[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}` 计数  
7. count == 0 → Quiet；count &gt; 0 → Present { count }

**禁止**：任何 `deletelocalsnapshots` / `tmutil delete` 调用路径出现在本模块。

## 6. 协议形状

`vole-proto::status`：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub local_snapshots: Option<LocalSnapshotsInfo>,

pub struct LocalSnapshotsInfo {
    /// 仅 Present 时有值；Skip 时为 None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    pub message: String,
}
```

旧客户端忽略未知字段；omit 时向后兼容。

## 7. 覆盖与发版

- `coverage_note`「已落地」追加「本地快照报告（status/analyze · 仅 list）」；「仍未移植：」仅剩「桌面 SMAppService / 特权助手」
- 测试：断言 unported **不含**「快照」；**仍含** SMAppService
- 规则数句保持 533（或当前 main 值）；只改覆盖文案
- `docs/releases/v1.29.0.md`；README 成熟度一行可同步
- workspace `version = "1.29.0"`

## 8. 测试

1. Fake：无 tmutil / AutoBackup 坏 → Quiet  
2. Running → SkippedBusy；Unknown → SkippedUnknown  
3. 固定 stdout fixture → count=N；失败/超时 → Quiet  
4. status JSON：Present 含 count+message；Quiet 无字段  
5. grep 本模块无 `deletelocalsnapshots`  
6. 不改动 `tmbackup` 单测预期  

## 9. 非目标

- W2 任意轨；删除本地快照；改 `tm-failed-backups`
- 桌面 Helper；bump schema_version

## 10. 验收

1. 本机有本地快照时 `vole status` 显示数量与 review 提示  
2. `tmutil` 失败时不显示假数量  
3. clean plan/apply 无本地快照删除  
4. coverage / README / 1.29.0 与上文一致  
5. CI：`fmt` + 相关 `cargo test`（macOS）通过  
