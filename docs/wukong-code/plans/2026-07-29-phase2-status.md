# Phase 2：`status` 命令 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现只读 `status` 子命令——指标采集、健康分、TUI 实时面板、`--json` 与 `--json-stream` 双消费端，并落地信号处理与终端恢复基础设施。

**Architecture:** `vole-proto` 承载与 mole 对齐的 `StatusSnapshot` JSON 类型；`vole-sys` 扩展 `Metrics` 与各 macOS 采集后端（sysinfo + IOKit/子进程）；`vole-core::status` 组装快照、健康分与历史环缓冲；`vole-cli::tui` 用 ratatui 立即模式渲染；采集线程经 `crossbeam-channel` 向 TUI 与 NDJSON 流各发一份相同快照（设计 5.3 / 5.6）。

**Tech Stack:** Rust 1.97.1、`ratatui` 0.30、`crossterm` 0.28、`sysinfo` 0.39、`objc2-io-kit` 0.3、`signal-hook` 0.3、`crossbeam-channel` 0.5。

**参照设计文档：** `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` 第 8 节 **Phase 2**、5.3、5.6、5.8。Phase 1 结论：`sysinfo` 磁盘 I/O **可用**（见 `docs/findings/2026-07-phase1-sysinfo-disk-io.md`）。

## Global Constraints

- 许可证：**GPL-3.0-only**。
- 平台：仅 macOS；非 macOS target `compile_error!`。
- `unsafe` 只允许在 `vole-sys`；其余 crate `#![forbid(unsafe_code)]`。
- crate 依赖单向：`vole-cli` → `vole-core` → `vole-sys` → `vole-proto`。
- 不引入 `tokio`（设计 5.3）。
- `SysCommand` 每个方法必须带超时参数（设计 5.8）。
- Mole 兼容 JSON：**mole 字段集是 Vole 的子集**；同名字段类型与嵌套结构一致；Vole 可追加字段。
- 协议 **定型不等于冻结**；冻结时点 Phase 4 结束。
- `status` 只读，**不需要** `clean.lock`；偏好文件写入用已有 `try_lock_config("status-prefs")`。
- 提交粒度：每个 Task 至少一次提交。

---

## File Structure

Phase 2 结束时的增量形态：

```
vole/
├── conformance/fixtures/
│   └── status-health-score.json          # 从 metrics_health_test.go 抽取
├── crates/
│   ├── vole-proto/src/
│   │   └── status.rs                     # StatusSnapshot 及嵌套类型
│   ├── vole-sys/src/
│   │   ├── traits.rs                     # Metrics trait 扩展
│   │   └── macos/
│   │       ├── metrics.rs                # 完整采集实现
│   │       ├── battery.rs                # IOKit 电源
│   │       └── gpu.rs                    # ioreg / system_profiler 解析
│   ├── vole-core/src/
│   │   └── status/
│   │       ├── mod.rs
│   │       ├── health.rs                 # calculate_health_score
│   │       ├── ring.rs                   # 环缓冲（网络历史等）
│   │       └── collector.rs              # 组装 StatusSnapshot
│   └── vole-cli/src/
│       ├── main.rs                       # Status 子命令
│       ├── terminal.rs                   # RAII + panic hook
│       ├── signals.rs                    # SIGINT/SIGTERM
│       └── tui/
│           ├── mod.rs
│           ├── theme.rs
│           ├── widgets.rs
│           └── status_view.rs
├── scripts/
│   ├── verify-status-json.sh             # 与 mo status --json 对照
│   └── verify-status-tty.exp             # 终端恢复（expect）
└── docs/protocol.md                      # 增补 status NDJSON 行格式
```

---

## Task 1: `vole-proto` StatusSnapshot 类型

对齐 `third_party/mole-1.48.1/cmd/status/metrics.go` 的 `MetricsSnapshot` 及嵌套 struct JSON 字段名。

**Files:**
- Create: `crates/vole-proto/src/status.rs`
- Modify: `crates/vole-proto/src/lib.rs`

**Interfaces:**
- Produces:
  - `status::StatusSnapshot` — 字段与 mole `MetricsSnapshot` 同名（`collected_at`, `host`, `platform`, `hardware`, `cpu`, `memory`, `disks`, `disk_io`, `network`, `health_score`, …）
  - 嵌套：`HardwareInfo`, `CpuStatus`, `MemoryStatus`, `DiskStatus`, `DiskIoStatus`, `NetworkStatus`, `BatteryStatus`, `ThermalStatus`, `ProcessInfo`, …
  - 所有字段 `serde` snake_case；`Option` 用于 Vole 暂缺的可选 mole 字段

- [ ] **Step 1: 写序列化测试**

```rust
#[test]
fn status_snapshot_roundtrip_snake_case() {
    let snap = StatusSnapshot {
        host: "mac".into(),
        health_score: 85,
        ..Default::default()
    };
    let v = serde_json::to_value(&snap).unwrap();
    assert_eq!(v["health_score"], 85);
    assert!(v.get("host").is_some());
}
```

- [ ] **Step 2: 实现类型**（mirror `metrics.go` 62–120 行字段集）

- [ ] **Step 3: 提交**

```bash
git add crates/vole-proto/
git commit -m "$(cat <<'EOF'
feat(proto): add StatusSnapshot types aligned with mo status JSON

Defines the mole-compatible metrics object so CLI and sidecar share
one serde contract before Phase 2 collectors land.
EOF
)"
```

---

## Task 2: 健康分算法与 fixture 测试

移植 `cmd/status/metrics_health.go` 的 `calculateHealthScore` 及 `metrics_health_test.go` 关键用例。

**Files:**
- Create: `crates/vole-core/src/status/health.rs`
- Create: `crates/vole-core/src/status/mod.rs`
- Create: `conformance/fixtures/status-health-score.json`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Consumes: `vole_proto::status::{CpuStatus, MemoryStatus, DiskStatus, DiskIoStatus, ThermalStatus, BatteryStatus}`
- Produces:
  - `status::health::calculate_health_score(...) -> (i32, String)`
  - 常量权重与阈值与 Go 源文件逐行对齐（`healthCPUWeight = 30.0` 等）

**Fixture 格式** (`status-health-score.json`):

```json
[
  {
    "name": "perfect",
    "input": { "cpu": {"usage": 10}, "memory": {"used_percent": 20, "pressure": "normal"}, ... },
    "want_score": 100,
    "want_msg_contains": "Excellent"
  },
  ...
]
```

从 Go 测试抽取至少：`TestCalculateHealthScorePerfect`、`TestCalculateHealthScoreDetectsIssues`、`TestCalculateHealthScoreCapsFailingSMARTAt44`、`TestCalculateHealthScoreMonotonicInCPU`（表驱动子集，不必移植 lipgloss 颜色测试）。

- [ ] **Step 1: 写 fixture 与失败测试**

- [ ] **Step 2: 实现 `calculate_health_score`**

- [ ] **Step 3: `cargo test -p vole-core status::health` 全绿**

- [ ] **Step 4: 提交**

```bash
git add crates/vole-core/src/status/ conformance/fixtures/status-health-score.json
git commit -m "$(cat <<'EOF'
feat(core): port mo status health score algorithm

Table-driven fixtures from metrics_health_test.go lock scoring behavior
so TUI labels stay consistent with mole.
EOF
)"
```

---

## Task 3: 环缓冲与采集调度骨架

**Files:**
- Create: `crates/vole-core/src/status/ring.rs`
- Create: `crates/vole-core/src/status/collector.rs`

**Interfaces:**
- Produces:
  - `ring::RingBuffer<f64>` — 对齐 Go `RingBuffer`（`Add`, `slice`  chronological）
  - `collector::CollectionMode` — `Fast | Process | Full`（对齐 mole `collectionMode`）
  - `collector::StatusCollector::collect(mode) -> Result<StatusSnapshot, CollectError>`

本 Task 只实现调度骨架与 `Fast` 路径占位（返回最小合法 `StatusSnapshot`）；真实指标在 Task 4–5 填入。

- [ ] **Step 1: RingBuffer 单元测试**（Add 5 项后 Slice 顺序正确）

- [ ] **Step 2: Collector 骨架 + 1s 间隔常量**

- [ ] **Step 3: 提交**

```bash
git add crates/vole-core/src/status/
git commit -m "$(cat <<'EOF'
feat(core): add status ring buffer and collector skeleton

Mirrors mole's fast/process/full refresh tiers before macOS backends
wire in per-metric collectors.
EOF
)"
```

---

## Task 4: `vole-sys` 基础指标（CPU / 内存 / 磁盘 / 磁盘 I/O）

**Files:**
- Modify: `crates/vole-sys/src/traits.rs`
- Modify: `crates/vole-sys/src/macos/metrics.rs`
- Modify: `crates/vole-sys/Cargo.toml`（如需 `libc`）

**Interfaces:**
- Extends `Metrics` trait:
  - `fn collect_cpu(&self, full_sample: bool) -> CpuStatus`
  - `fn collect_memory(&self) -> MemoryStatus`
  - `fn collect_disks(&self) -> Vec<DiskStatus>`
  - `fn collect_disk_io(&self, prev: Option<&DiskIoSample>) -> (DiskIoStatus, DiskIoSample)`
- `DiskIoSample` 持有上次 `total_read/write` 与时间戳，用于速率 MB/s

实现要点：
- CPU：对齐 `metrics_cpu.go` 双采样窗口（100ms）；Apple Silicon  parked-core 修正可 Phase 2 后期优化，首版用 `sysinfo` + 双采样。
- 磁盘列表：`sysinfo::Disks`；用量百分比与 mole 一致用主卷 `/` 或 `Data` 卷。
- 磁盘 I/O：按 Phase 1 结论用 `Disk::usage()` delta / Δt。

- [ ] **Step 1: trait 扩展 + macOS 实现 + 单元测试**（mock 磁盘列表非空、IO 速率非负）

- [ ] **Step 2: 提交**

```bash
git add crates/vole-sys/
git commit -m "$(cat <<'EOF'
feat(sys): collect CPU memory disk metrics via sysinfo

Covers the core status panels; disk IO rates use sysinfo IOKit path
validated in Phase 1.
EOF
)"
```

---

## Task 5: 扩展指标（网络 / 电池 / 温度 / GPU / 蓝牙 / 进程）

**Files:**
- Create: `crates/vole-sys/src/macos/battery.rs`
- Create: `crates/vole-sys/src/macos/gpu.rs`
- Modify: `crates/vole-sys/src/macos/metrics.rs`
- Modify: `crates/vole-sys/src/macos/mod.rs`

**Interfaces:**
- `Metrics` 扩展：
  - `collect_network(prev) -> (Vec<NetworkStatus>, NetworkDelta)`
  - `collect_batteries() -> Vec<BatteryStatus>` — `IOPSCopyPowerSourcesInfo` via `objc2-io-kit`
  - `collect_thermal() -> ThermalStatus` — `ioreg` 或 sysctl，带超时
  - `collect_gpu() -> Vec<GpuStatus>` — `system_profiler SPDisplaysDataType` 子进程或 ioreg 解析
  - `collect_bluetooth() -> Vec<BluetoothDevice>` — 可降级为空数组
  - `collect_top_processes(n) -> Vec<ProcessInfo>` — `sysinfo::System` refresh

子进程调用走已有 `MacSysCommand` + `timeouts::SYS_CMD`。

缺数据时返回 mole 兼容的空/零值，**不 panic**。

- [ ] **Step 1: 逐项实现 + 烟雾测试**（`cargo test -p vole-sys`）

- [ ] **Step 2: 提交**

```bash
git add crates/vole-sys/src/macos/
git commit -m "$(cat <<'EOF'
feat(sys): add network battery thermal GPU and process collectors

Completes the status metrics surface behind the Metrics trait with
timeouts on every subprocess fallback.
EOF
)"
```

---

## Task 6: 组装完整 `StatusSnapshot`

**Files:**
- Modify: `crates/vole-core/src/status/collector.rs`
- Modify: `crates/vole-core/src/status/mod.rs`

**Interfaces:**
- `StatusCollector::collect_full() -> StatusSnapshot` 填充：
  - `hardware`（`sysctl`/`host` 信息：机型、RAM、OS 版本、磁盘总容量字符串）
  - 各 `Metrics` 子采集器结果
  - `health_score` / `health_score_msg` via `calculate_health_score`
  - `network_history` 环缓冲（至少 60 点）
  - `trash_size` via `trash` crate 或 mole 等价路径

- [ ] **Step 1: 集成测试** — `collect_full()` 返回 JSON 可序列化，瞬时字段在合法范围

```rust
let snap = collector.collect_full().unwrap();
assert!(snap.cpu.usage >= 0.0 && snap.cpu.usage <= 100.0);
assert!(snap.health_score >= 0 && snap.health_score <= 100);
```

- [ ] **Step 2: 提交**

```bash
git add crates/vole-core/src/status/
git commit -m "$(cat <<'EOF'
feat(core): assemble full StatusSnapshot from sys metrics

Single collector path feeds TUI, --json, and --json-stream with the
same snapshot shape mole expects.
EOF
)"
```

---

## Task 7: 终端 RAII、panic hook 与信号处理

设计 5.8：先恢复终端再打印 panic；SIGINT/SIGTERM → 取消 token → 恢复终端 → 130/143。

**Files:**
- Create: `crates/vole-cli/src/terminal.rs`
- Create: `crates/vole-cli/src/signals.rs`
- Modify: `crates/vole-cli/Cargo.toml`（`crossterm`, `signal-hook`）

**Interfaces:**
- `terminal::TerminalGuard` — 进入 alternate screen + raw mode；`Drop` 恢复
- `terminal::install_panic_hook()` — 先 `guard.restore()` 再默认 panic 输出
- `signals::install_handlers(cancel: CancelToken, guard: TerminalGuard)` — 130/143 退出码

`--json` / `--json-stream` 路径**不**创建 `TerminalGuard`。

- [ ] **Step 1: 单元测试** — guard drop 后 `is_raw_mode_enabled` 为 false（crossterm 测试辅助）

- [ ] **Step 2: 提交**

```bash
git add crates/vole-cli/src/terminal.rs crates/vole-cli/src/signals.rs crates/vole-cli/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(cli): terminal RAII guard and signal handlers for TUI

Ensures panic and Ctrl-C restore the tty before exit codes 130/143,
matching mole trap semantics for every future TUI command.
EOF
)"
```

---

## Task 8: TUI 主题与基础组件

**Files:**
- Create: `crates/vole-cli/src/tui/mod.rs`
- Create: `crates/vole-cli/src/tui/theme.rs`
- Create: `crates/vole-cli/src/tui/widgets.rs`
- Modify: `crates/vole-cli/Cargo.toml`（`ratatui` 0.30）

**Interfaces:**
- `theme::Theme` — 颜色与边框样式（对齐 mole lipgloss 语义：ok/warn/danger）
- `widgets::progress_bar(value, width)`
- `widgets::sparkline(samples, width)`
- `widgets::card(title, lines, width)`

首版不必复刻 mole ASCII cat；卡片布局对齐 `view.go` 两列/窄屏单列逻辑。

- [ ] **Step 1: 纯渲染测试**（固定 `StatusSnapshot` fixture → `render_status` 输出非空、含 health_score）

- [ ] **Step 2: 提交**

```bash
git add crates/vole-cli/src/tui/
git commit -m "$(cat <<'EOF'
feat(cli): ratatui theme and status dashboard widgets

Provides progress bars, sparklines, and cards for the status TUI
without coupling layout to the metrics collector.
EOF
)"
```

---

## Task 9: Status TUI 循环与采集线程

**Files:**
- Create: `crates/vole-cli/src/tui/status_view.rs`
- Modify: `crates/vole-cli/src/main.rs`

**Architecture:**
- 主线程：crossterm 事件循环，~30fps `ratatui::draw`
- 后台线程：`StatusCollector` 按 mole 间隔调度（1s fast/process，30s full）
- `crossbeam-channel` 传递 `StatusSnapshot`；主线程 `try_recv`
- `CancelToken`：Esc / Ctrl-C 取消；停止采集线程（带 2s 上限）

- [ ] **Step 1: 手动冒烟** — `cargo run -p vole-cli -- status` 显示 CPU/内存/健康分

- [ ] **Step 2: 提交**

```bash
git add crates/vole-cli/
git commit -m "$(cat <<'EOF'
feat(cli): interactive status TUI with background collector

Validates the Phase 1 concurrency model: one snapshot stream feeding
ratatui at ~30fps from a dedicated metrics thread.
EOF
)"
```

---

## Task 10: `--json` 与管道自动检测

对齐 `mo status --json`；`shouldUseJSONOutput` 逻辑移植（`stdout` 非 tty 时自动 JSON）。

**Files:**
- Modify: `crates/vole-cli/src/main.rs`
- Create: `scripts/verify-status-json.sh`

**Interfaces:**
- `fn should_use_json(force: bool) -> bool`
- `fn cmd_status_json(snapshot: &StatusSnapshot)` — 单行 pretty 或 compact JSON 与 mole 一致（mole 用标准 `json.Marshal`）

**verify-status-json.sh**:

```bash
#!/usr/bin/env bash
set -euo pipefail
REPO=$(cd "$(dirname "$0")/.." && pwd)
MOLE_JSON=$(env HOME="$HOME" "$REPO/third_party/mole-1.48.1/mo" status --json)
VOLE_JSON=$(cargo run -q -p vole-cli -- status --json)
python3 - "$MOLE_JSON" "$VOLE_JSON" <<'PY'
import json, sys
m, v = json.loads(sys.argv[1]), json.loads(sys.argv[2])
# 静态字段精确相等
for key in ("host", "platform"):
    assert m.get(key) == v.get(key), key
# hardware 子集
for key in ("model", "total_ram", "os_version"):
    assert m["hardware"].get(key) == v["hardware"].get(key), key
# 瞬时字段范围
assert 0 <= v["cpu"]["usage"] <= 100
assert 0 <= v["memory"]["used_percent"] <= 100
assert 0 <= v["health_score"] <= 100
print("OK: status JSON static fields and ranges")
PY
```

- [ ] **Step 1: 实现 + 跑脚本**

- [ ] **Step 2: 提交**

```bash
git add crates/vole-cli/src/main.rs scripts/verify-status-json.sh
git commit -m "$(cat <<'EOF'
feat(cli): status --json with mole-compatible field layout

Pipe detection mirrors mo status; verify script checks static fields
and sane ranges without brittle cross-process value equality.
EOF
)"
```

---

## Task 11: `--json-stream` 双消费端与协议文档

对齐 mole `--watch`（每行一个 JSON 快照）；Vole 统一 flag 名 `--json-stream`（与 `clean` 一致）。

**Files:**
- Modify: `crates/vole-cli/src/main.rs`
- Modify: `docs/protocol.md`

**行为:**
- `vole status --json-stream`：stdout NDJSON，每行 `StatusSnapshot` + `schema_version` 包装可选
- 采集线程与 TUI 共用同一 channel 类型（设计 5.6 双消费端验证）
- 取消时 flush 并输出 `{"schema_version":1,"type":"aborted","reason":"cancelled"}`（扩展 `StreamEvent` 或 status 专用行——Phase 2–3 可破坏性修改）

`docs/protocol.md` 增补：

```markdown
## Status 流（Phase 2）

每行一个 JSON 对象，字段集同 `mo status --watch`。取消时最后一行可为 `aborted` 事件。
```

- [ ] **Step 1: 实现流模式**

- [ ] **Step 2: 双跑** — 同时 `status` TUI 与 `status --json-stream | head -3` 无死锁

- [ ] **Step 3: 提交**

```bash
git add crates/vole-cli/ docs/protocol.md
git commit -m "$(cat <<'EOF'
feat(cli): status --json-stream NDJSON watch mode

One metrics thread feeds both TUI and stdout NDJSON, proving the
sidecar consumption model on the simplest read-only command.
EOF
)"
```

---

## Task 12: 终端恢复 expect 脚本

移植 `tests/timeout_tty_restore.exp` 思路，针对 `vole status` TUI。

**Files:**
- Create: `scripts/verify-status-tty.exp`
- Create: `scripts/verify-status-tty-fixture.sh`（启动 status 后被 timeout SIGINT / 人为 panic 路径）
- Modify: `.github/workflows/ci.yml`（若 CI 有 expect；无则文档注明本地验收）

三种路径：
1. **SIGINT**（Ctrl-C / timeout 杀进程组）
2. **SIGTERM**
3. **panic**（`VOLE_STATUS_PANIC=1` 测试钩子触发 `panic!`）

验收：expect 脚本在超时后仍能 `read` 用户输入并回显 `typed-after`。

- [ ] **Step 1: 写 fixture + expect**

- [ ] **Step 2: 本地跑通** — `./scripts/verify-status-tty.exp`

- [ ] **Step 3: 提交**

```bash
git add scripts/verify-status-tty.exp scripts/verify-status-tty-fixture.sh
git commit -m "$(cat <<'EOF'
test: verify status TUI restores terminal after panic and signals

Automates the design 5.8 tty recovery requirement before analyze
inherits the same terminal infrastructure.
EOF
)"
```

---

## Task 13: Phase 2 验收与收口

- [ ] **Step 1: 全量 CI 本地复现**

```bash
./scripts/check-license.sh
./scripts/check-dep-direction.sh
./scripts/check-protocol-doc.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --target aarch64-apple-darwin
cargo build --workspace --target x86_64-apple-darwin
./scripts/verify-status-json.sh
./scripts/verify-status-tty.exp "$(pwd)"   # 若已装 expect
```

- [ ] **Step 2: 验收清单对照**

| 设计 Phase 2 验收项 | 本计划对应 |
|---|---|
| mole 字段集是 Vole 子集 | Task 1, 10 |
| 静态字段精确相等 | Task 10 `verify-status-json.sh` |
| 瞬时字段存在性与范围 | Task 6, 10 |
| 健康分 Go 测试表驱动 | Task 2 |
| 终端恢复三路径 | Task 7, 12 |
| TUI + `--json-stream` 双消费端 | Task 9, 11 |

- [ ] **Step 3: 更新 README** — Phase 2 完成：`vole status` 可用；`analyze`/`clean` 仍不可用

- [ ] **Step 4: 提交**

```bash
git add README.md
git commit -m "$(cat <<'EOF'
docs: note Phase 2 status command in README scope

Documents that vole status is the first user-facing monitoring
command; analyze and clean remain future phases.
EOF
)"
```

---

## 完成判据

Plan 3（Phase 2）全部完成时：

- [ ] `cargo test --workspace` 与 CI 全绿
- [ ] `vole status` TUI 可交互退出且终端正常
- [ ] `vole status --json` 与 `mo status --json` 静态字段一致
- [ ] `vole status --json-stream` 连续输出合法 JSON 行
- [ ] 健康分 fixture 全过
- [ ] `verify-status-tty.exp` 三路径通过（或 documented skip 原因）
- [ ] `docs/protocol.md` 含 status 流说明

**不在本计划范围内**：`analyze`、`clean`、规则引擎、plan/apply、Developer ID 签名流水线。

---

## Self-Review（计划自检）

| 设计 Phase 2 要求 | 任务 |
|---|---|
| TUI 主题与组件 | Task 8 |
| 信号与终端恢复 | Task 7, 12 |
| 指标采集全套 | Task 4, 5, 6 |
| `--json` 对齐 | Task 10 |
| 双消费端 | Task 9, 11 |
| 健康分测试 | Task 2 |
| 验收判据 1–5 | Task 10, 12, 13 |

**已知简化（Phase 2 可接受，Phase 2 末文档化）**：
- mole ASCII cat 动画可省略或占位
- `process_watch` / `process_alerts` 可返回空配置与空列表（mole 子集仍成立）
- CPU parked-core 修正（#1237）可 follow-up PR

**sysinfo 结论已纳入**：Task 4 磁盘 I/O 用 `Disk::usage()` delta，无需新 IOKit crate。
