# Phase 1：地基、协议定型与平台实测 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 定型 NDJSON 协议与 `vole-proto` 类型、落地超时/互斥/单位格式化/whitelist/oplog 等跨命令基础设施，并为 `vole-sys` 平台 trait 立好接口——使 Phase 2 `status` 能直接消费协议与平台层，而不按 TUI 形状长歪。

**Architecture:** 协议类型继续住在叶子 crate `vole-proto`；平台抽象在 `vole-sys`（唯一 `unsafe`）；跨命令业务逻辑（units、whitelist、oplog、超时配置、互斥、`ops` 编排骨架）在 `vole-core` module；`vole-cli` 本阶段不新增子命令，只加 `VOLE_NO_OPLOG` 等 env 透传测试钩子。TCC 完整矩阵与 Developer ID **延后**（用户决策 2026-07-29），本计划用 ad-hoc 子集 + 占位文档承接 Phase 1 验收第 3 条。

**Tech Stack:** Rust 1.97.1（`rust-toolchain.toml`）、`serde`/`serde_json`、`thiserror`、`anyhow`、`rustix` 1.1、`plist` 1.10、`rusqlite`（bundled）、`trash` 5.2、`sysinfo` 0.39、`crossbeam-channel` 0.5。

**参照设计文档：** `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md`（Phase 0.5 校准后，commit `1cc2ca0`）。本计划只覆盖第 8 节 **Phase 1**。

## Global Constraints

- 许可证：**GPL-3.0-only**。
- 平台：仅 macOS；非 macOS target `compile_error!`。
- `unsafe` 只允许在 `vole-sys`；其余 crate `#![forbid(unsafe_code)]`。
- crate 依赖单向：`vole-cli` → `vole-core` → `vole-sys` → `vole-proto`。
- 起步 4 crate；`rules`/`scan`/`ops`/`tui` 为 `vole-core` module，超 2500 行再拆。
- 不引入 `tokio`（设计 5.3）。
- `SysCommand` **每个方法必须带超时参数**，无无超时重载（设计 5.8）。
- 协议 **定型不等于冻结**；冻结时点 Phase 4 结束（设计 5.6）。
- Apple Developer ID：**本阶段不申请**；TCC 完整矩阵标为 deferred，见 Task 10。
- 提交粒度：每个 Task 至少一次提交。

---

## File Structure

Phase 1 结束时的增量形态（在 Phase 0 骨架之上）：

```
vole/
├── docs/
│   ├── protocol.md                     # NDJSON 协议 human-readable 规格
│   └── findings/
│       └── 2026-07-phase1-tcc-deferred.md
│       └── 2026-07-phase1-sysinfo-disk-io.md
├── crates/
│   ├── vole-proto/src/
│   │   ├── lib.rs                      # 重导出 + SCHEMA_VERSION
│   │   ├── events.rs                   # StreamEvent 枚举
│   │   ├── plan.rs                     # Plan / PlanEntry
│   │   └── report.rs                   # Report / SkipSummary
│   ├── vole-sys/src/
│   │   ├── lib.rs
│   │   ├── traits.rs                   # Fs, Plist, Sqlite, Trash, SysCommand, Metrics
│   │   ├── timeouts.rs                 # 集中超时表（对齐 mole timeouts.sh）
│   │   └── macos/                      # 各 trait 的 macOS 后端（本阶段最小可测实现）
│   │       ├── mod.rs
│   │       ├── fs.rs
│   │       ├── plist.rs
│   │       ├── sqlite.rs
│   │       ├── trash.rs
│   │       ├── syscommand.rs
│   │       └── metrics.rs
│   └── vole-core/src/
│       ├── lib.rs
│       ├── units.rs                    # 移植 internal/units
│       ├── whitelist.rs
│       ├── oplog.rs
│       ├── mutex.rs                    # flock 互斥
│       ├── cancel.rs                   # CancelToken + 通道类型别名
│       ├── ops/mod.rs                  # 编排骨架（本阶段无子命令调用）
│       └── spike_toctou.rs             # Phase 0.5 已有，保留
├── scripts/
│   ├── check-protocol-doc.sh           # protocol.md 与 vole-proto 字段一致性
│   ├── verify-oplog-mole.sh            # vole 写的 oplog 能被 mo history 读
│   └── tcc-adhoc-matrix.sh             # ad-hoc 签名最小 TCC 探测（非完整矩阵）
```

---

## Task 1: 扩展 `vole-proto` 事件流与 Report

当前 `vole-proto` 只有 `Candidate` 与 `SkipReason`。Phase 1 需定型设计 5.6 的全部 NDJSON 事件与 `Report`。

**Files:**
- Create: `crates/vole-proto/src/events.rs`
- Create: `crates/vole-proto/src/plan.rs`
- Create: `crates/vole-proto/src/report.rs`
- Modify: `crates/vole-proto/src/lib.rs`
- Modify: `crates/vole-proto/Cargo.toml`（加 `serde_json` 用于测试）

**Interfaces:**
- Consumes: 现有 `SkipReason`、`SCHEMA_VERSION`
- Produces:
  - `events::StreamEvent`（`Progress` / `Candidate` / `Skipped` / `Done` / `Aborted`）
  - `plan::PlanEntry { id, path, label, size, rule_id, skip_reason, dev, ino, mtime }`
  - `plan::Plan { schema_version, created_at, ttl_secs, entries }`
  - `report::Report { succeeded, skipped, failed, skipped_by_reason, trashed_bytes, deleted_bytes }`
  - `report::SkipSummary { reason, count, rule_ids }`

- [ ] **Step 1: 写失败的序列化测试**

`crates/vole-proto/src/events.rs` 底部 `#[cfg(test)]`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn progress_event_serializes_with_snake_case_type() {
        let e = StreamEvent::Progress {
            scanned: 100,
            current: "~/Library/Caches".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "progress");
        assert_eq!(v["scanned"], 100);
    }

    #[test]
    fn done_event_wraps_report() {
        let e = StreamEvent::Done {
            report: Report {
                succeeded: 1,
                skipped: 2,
                failed: 0,
                skipped_by_reason: vec![],
                trashed_bytes: 0,
                deleted_bytes: 0,
            },
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "done");
        assert_eq!(v["report"]["succeeded"], 1);
    }
}
```

同时在 `lib.rs` 加 `mod events; mod plan; mod report;` 但不实现类型——先让测试编译失败。

- [ ] **Step 2: 运行确认失败**

```bash
cargo test -p vole-proto
```

预期：编译错误，`StreamEvent` / `Report` 未定义。

- [ ] **Step 3: 实现类型**

`events.rs`：

```rust
use serde::{Deserialize, Serialize};

use crate::report::Report;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Progress {
        scanned: u64,
        current: String,
    },
    Candidate {
        id: String,
        path: String,
        label: String,
        size: u64,
        rule_id: String,
    },
    Skipped {
        rule_id: String,
        reason: crate::SkipReason,
    },
    Done {
        report: Report,
    },
    Aborted {
        reason: String,
    },
}

impl StreamEvent {
    pub fn with_schema(self, schema_version: u32) -> serde_json::Value {
        let mut v = serde_json::to_value(self).expect("StreamEvent 可序列化");
        if let Some(obj) = v.as_object_mut() {
            obj.insert("schema_version".into(), schema_version.into());
        }
        v
    }
}
```

`plan.rs`：

```rust
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::SkipReason;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    pub id: String,
    pub path: PathBuf,
    pub label: String,
    pub size: u64,
    pub rule_id: String,
    pub skip_reason: Option<SkipReason>,
    pub dev: u64,
    pub ino: u64,
    pub mtime: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub schema_version: u32,
    pub created_at: SystemTime,
    pub ttl_secs: u64,
    pub entries: Vec<PlanEntry>,
}
```

`report.rs`：

```rust
use serde::{Deserialize, Serialize};

use crate::SkipReason;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Report {
    pub succeeded: u64,
    pub skipped: u64,
    pub failed: u64,
    pub skipped_by_reason: Vec<SkipSummary>,
    pub trashed_bytes: u64,
    pub deleted_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSummary {
    pub reason: SkipReason,
    pub count: u64,
    pub rule_ids: Vec<String>,
}
```

`lib.rs` 重导出：`pub use events::StreamEvent; pub use plan::{Plan, PlanEntry}; pub use report::{Report, SkipSummary};`

`Cargo.toml` dependencies 加 `serde_json.workspace = true`。

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p vole-proto
```

预期：全部 passed。

- [ ] **Step 5: 提交**

```bash
git add crates/vole-proto/
git commit -m "$(cat <<'EOF'
feat(proto): add stream events, Plan and Report types

Shapes the NDJSON contract before Phase 2 status work so orchestration
grows around the protocol instead of the TUI.
EOF
)"
```

---

## Task 2: `docs/protocol.md` 与一致性检查

**Files:**
- Create: `docs/protocol.md`
- Create: `scripts/check-protocol-doc.sh`
- Modify: `.github/workflows/ci.yml`（加一步）

**Interfaces:**
- Consumes: Task 1 的全部 `vole-proto` 公开类型
- Produces: `scripts/check-protocol-doc.sh` 退出码 0 表示文档含关键字段名

- [ ] **Step 1: 写 `docs/protocol.md`**

内容须包含（可复制为骨架再填表）：

```markdown
# Vole NDJSON 协议（v1 定型，Phase 4 末冻结）

`schema_version` 当前为 **1**。破坏性变更递增版本号。

## 传输

- stdout：NDJSON，一行一个事件；**仅协议**。
- stderr：人类日志与诊断。

## 事件类型

| type | 字段 | 说明 |
|---|---|---|
| `progress` | `scanned`, `current` | 扫描进度 |
| `candidate` | `id`, `path`, `label`, `size`, `rule_id` | plan 阶段候选 |
| `skipped` | `rule_id`, `reason` | 规则级跳过 |
| `done` | `report` | 结束汇总 |
| `aborted` | `reason` | 取消或异常中止 |

`reason` 取值见 `vole_proto::SkipReason`（snake_case）。

## Plan 文件

JSON 对象：`schema_version`, `created_at`, `ttl_secs`（默认 900）, `entries[]`。
每条 entry：`id`, `path`, `label`, `size`, `rule_id`, `skip_reason`, `dev`, `ino`, `mtime`。

## Report

`succeeded`, `skipped`, `failed`, `skipped_by_reason[]`, `trashed_bytes`, `deleted_bytes`。

废纸篓语义见设计文档 5.7——不得用单一「freed」字段。

## 冻结时点

Phase 4 结束前可破坏性修改；之后只能追加字段/枚举变体。
```

- [ ] **Step 2: 写检查脚本**

`scripts/check-protocol-doc.sh`：

```bash
#!/usr/bin/env bash
# 确保 protocol.md 提到所有 StreamEvent type 与 Report 关键字段。
set -euo pipefail
DOC=docs/protocol.md
fail=0
for needle in progress candidate skipped done aborted trashed_bytes deleted_bytes schema_version; do
    if ! grep -q "$needle" "$DOC"; then
        echo "FAIL: $DOC 缺少 $needle" >&2
        fail=1
    fi
done
[[ $fail -eq 0 ]] && echo "OK: protocol.md 关键字段齐全"
exit $fail
```

- [ ] **Step 3: 接入 CI 并验证**

```bash
chmod +x scripts/check-protocol-doc.sh
./scripts/check-protocol-doc.sh
```

在 `.github/workflows/ci.yml` 的 `check` job 里 `License and attribution` 之后加：

```yaml
      - name: Protocol doc
        run: ./scripts/check-protocol-doc.sh
```

- [ ] **Step 4: 提交**

```bash
git add docs/protocol.md scripts/check-protocol-doc.sh .github/workflows/ci.yml
git commit -m "$(cat <<'EOF'
docs: add protocol.md and CI check for NDJSON contract fields

Keeps the human-readable spec next to vole-proto so sidecar consumers
can read one doc while implementers keep types in the leaf crate.
EOF
)"
```

---

## Task 3: 移植 `internal/units`

**Files:**
- Create: `crates/vole-core/src/units.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `units::bytes_si(i64) -> String`
  - `units::bytes_bin(u64) -> String`
  - `units::bytes_bin_short(u64) -> String`
  - `units::bytes_bin_compact(u64) -> String`

- [ ] **Step 1: 写失败测试（从 Go 测试表复制）**

`crates/vole-core/src/units.rs` 测试模块包含 `bytes_test.go` 的全部用例（见 `third_party/mole-1.48.1/internal/units/bytes_test.go`）。

- [ ] **Step 2: 运行确认失败**

```bash
cargo test -p vole-core units::
```

- [ ] **Step 3: 实现四个函数**

逻辑对齐 `third_party/mole-1.48.1/internal/units/bytes.go`（SI 用 1000 底，Bin 系列用 1024 底，边界 `>` vs `>=` 与 Go 一致）。

- [ ] **Step 4: 运行确认通过**

```bash
cargo test -p vole-core units::
```

预期：与 Go 表 100% 一致。

- [ ] **Step 5: 提交**

```bash
git add crates/vole-core/src/units.rs crates/vole-core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(core): port Mole byte formatting helpers and Go test table

SI vs binary conventions must match Finder and Activity Monitor
respectively, so status and analyze share one implementation.
EOF
)"
```

---

## Task 4: 超时配置与 `CancelToken` 定型

**Files:**
- Create: `crates/vole-sys/src/timeouts.rs`
- Create: `crates/vole-core/src/cancel.rs`
- Modify: `crates/vole-sys/src/lib.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Consumes: `vole_proto::SkipReason::Timeout`（已有）
- Produces:
  - `timeouts::DurationSec` 别名 + `timeouts::QUICK_DETECT` 等常量（对齐 `timeouts.sh` 数值）
  - `cancel::CancelToken`（`Arc<AtomicBool>` 包装 + `check()` / `is_cancelled()`）
  - `cancel::CancelGuard` 用于后台线程持有克隆

`timeouts.sh` 默认值（秒）：

```bash
MOLE_TIMEOUT_QUICK_DETECT_SEC=2
MOLE_TIMEOUT_SHORT_QUERY_SEC=3
MOLE_TIMEOUT_MEDIUM_PROBE_SEC=5
MOLE_TIMEOUT_PKG_LIST_SEC=10
MOLE_TIMEOUT_HINT_SCAN_SEC=15
MOLE_TIMEOUT_PKG_CLEANUP_SEC=20
MOLE_TIMEOUT_DISK_VERIFY_SEC=30
```

- [ ] **Step 1: 写测试**

`cancel.rs`：`CancelToken::new()` 初始未取消；`cancel()` 后 `is_cancelled()` 为 true。

`timeouts.rs`：`QUICK_DETECT.as_secs()` 为 2。

- [ ] **Step 2–4: 实现并通过测试**

- [ ] **Step 5: 提交**

```bash
git commit -m "$(cat <<'EOF'
feat: add centralized timeout constants and CancelToken skeleton

Locks in design 5.3/5.8 decisions before status TUI wires real work
to background threads.
EOF
)"
```

---

## Task 5: `flock` 进程互斥

**Files:**
- Create: `crates/vole-core/src/mutex.rs`
- Modify: `crates/vole-core/Cargo.toml`（`rustix` 若尚未在 core——通过 `vole-sys` 暴露或 core 直接依赖 `rustix` 的 flock）

设计：`clean` 用 `~/.cache/vole/clean.lock`；配置写入用独立锁文件。

**Interfaces:**
- Produces:
  - `mutex::try_lock_clean() -> Result<CleanLock, MutexError>`
  - `mutex::try_lock_config(name: &str) -> Result<ConfigLock, MutexError>`
  - `MutexError::AlreadyRunning`

- [ ] **Step 1: 写测试**

临时目录下：第一次 `try_lock_clean` 成功；同进程第二次 `LOCK_NB` 失败；`drop` 锁后第三次成功。

- [ ] **Step 2–4: 用 `rustix::fs::fcntl_getlk/fcntl_setlk` 或 `flock` 实现**

路径：`dirs::cache_dir()` 或 `$HOME/.cache/vole/`（本阶段可用 `std::env::var("HOME")`）。

- [ ] **Step 5: 提交**

```bash
git commit -m "$(cat <<'EOF'
feat(core): add flock-based clean and config mutexes

Non-blocking clean lock prevents two deletes racing; flock releases
on kill -9 unlike pidfiles.
EOF
)"
```

---

## Task 6: Whitelist 配置读写

对齐 Mole：`~/.config/mole/whitelist`（clean）——Vole **读取 mole 路径**以保持迁移平滑，写入时也用同一路径（设计第 2 节兼容列表）。

**Files:**
- Create: `crates/vole-core/src/whitelist.rs`

**Interfaces:**
- Produces:
  - `whitelist::load_clean() -> io::Result<Vec<String>>`
  - `whitelist::save_clean(patterns: &[String]) -> io::Result<()>`
  - `whitelist::is_match(path: &Path, patterns: &[String]) -> bool`（glob 语义对齐 mole `patterns_equivalent` 的简化版：本阶段精确行匹配 + `*` 后缀）

- [ ] **Step 1: 写测试**

临时 HOME：写入两行 pattern，load 回来；`is_match` 对 whitelist 行命中。

- [ ] **Step 2–4: 实现**

文件头注释对齐 mole：

```text
# Mole Whitelist - Protected paths won't be deleted
```

- [ ] **Step 5: 提交**

```bash
git commit -m "$(cat <<'EOF'
feat(core): read and write Mole-compatible clean whitelist config

Uses ~/.config/mole/whitelist so users migrating from Mole keep
their protections without re-entering paths.
EOF
)"
```

---

## Task 7: 操作日志（oplog）

对齐 `lib/core/log.sh` 行格式：

```text
[YYYY-MM-DD HH:MM:SS] [clean] REMOVED /path (detail)
# ========== clean session started at ... ==========
```

环境变量：`MO_NO_OPLOG=1` 或 `VOLE_NO_OPLOG=1` 禁用（对齐 `MO_NO_OPLOG`）。

**Files:**
- Create: `crates/vole-core/src/oplog.rs`
- Create: `scripts/verify-oplog-mole.sh`

**Interfaces:**
- Produces:
  - `oplog::OperationLogger::new(command: &str) -> Self`
  - `log(&mut self, action: &str, path: &Path, detail: Option<&str>)`
  - `session_start` / `session_end(items, size_kb)`
  - 默认路径 `$HOME/Library/Logs/mole/operations.log`（与 mole 相同，便于 `mo history` 反向验证）

- [ ] **Step 1: 写单元测试**

写入两行 REMOVED，读文件断言格式；`VOLE_NO_OPLOG=1` 时不写。

- [ ] **Step 2–4: 实现**

- [ ] **Step 5: 写反向验证脚本**

`scripts/verify-oplog-mole.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail
REPO=$(cd "$(dirname "$0")/.." && pwd)
TMP=$(mktemp -d)
export HOME="$TMP/home"
mkdir -p "$HOME/Library/Logs/mole"
# 用 Rust 测试二进制或小型 cargo run 钩子写入 oplog（见 Step 3 暴露的 test-only 函数或 integration test 产出文件）
# 然后：
MOLE_TEST_NO_AUTH=1 "$REPO/third_party/mole-1.48.1/mo" history --json 2>/dev/null | head -5
```

实现路径：在 `oplog` 模块加 `#[cfg(test)]` integration 或 `cargo test -p vole-core oplog::writes_lines_mole_can_parse` 里调用 mole `mo history --json`（需 mole 在 PATH 或绝对路径）。

验收判据（设计 Phase 1 第 2 条）：**vole 写出的 oplog 能被 `mo history --json` 解析**——脚本检查 JSON 数组非空且含 `REMOVED`。

- [ ] **Step 6: 提交**

```bash
git add crates/vole-core/src/oplog.rs scripts/verify-oplog-mole.sh
git commit -m "$(cat <<'EOF'
feat(core): write Mole-compatible operations.log entries

Reverse-verified via mo history --json so we need not ship vole
history in Phase 1.
EOF
)"
```

---

## Task 8: `vole-sys` 平台 trait 骨架

**Files:**
- Create: `crates/vole-sys/src/traits.rs`
- Create: `crates/vole-sys/src/macos/mod.rs` + 各后端文件
- Modify: `crates/vole-sys/Cargo.toml`

依赖（版本对齐设计 5.2）：

```toml
plist = "1.10"
rusqlite = { version = "0.37", features = ["bundled"] }
trash = "5.2"
sysinfo = "0.39"
rustix = { version = "1", features = ["fs", "process"] }
thiserror = "2"
```

**Interfaces:**
- Produces trait 定义（方法均带 `timeout: Duration` 或 `timeouts::DurationSec`）：

```rust
pub trait Fs: Send + Sync {
    fn metadata(&self, path: &Path, timeout: Duration) -> Result<Metadata, FsError>;
    // 本阶段最小集；Phase 2+ 扩展 openat 等
}

pub trait SysCommand: Send + Sync {
    fn run(&self, argv: &[&str], timeout: Duration) -> Result<Output, SysCommandError>;
}

pub trait Plist { fn read_bool(&self, path: &Path, key: &str, timeout: Duration) -> ... }
pub trait Sqlite { fn query_count(&self, path: &Path, sql: &str, timeout: Duration) -> ... }
pub trait Trash { fn trash_path(&self, path: &Path, timeout: Duration) -> ... }
pub trait Metrics { fn disk_usage_root(&self) -> ... }  // sysinfo 探针
```

`macos/mod.rs` 导出 `MacOsBackend { fs, plist, ... }` 聚合实现。

- [ ] **Step 1: 写 trait 编译测试**

`traits.rs` 仅定义 + `MacOsBackend::new()` 可构造。

- [ ] **Step 2: 实现 macOS 最小后端**

`SysCommand::run` 用 `std::process::Command` + `rustix` 杀进程组（超时后）；`Plist` 用 `plist` crate 读测试 fixture；`Metrics::disk_usage_root` 调 `sysinfo::Disks`。

- [ ] **Step 3: 测试**

```bash
cargo test -p vole-sys
```

- [ ] **Step 4: 提交**

```bash
git commit -m "$(cat <<'EOF'
feat(sys): scaffold platform traits with minimal macOS backends

Every SysCommand path carries a timeout from day one so Phase 3
analyze cannot accidentally add blocking subprocess calls.
EOF
)"
```

---

## Task 9: `vole-core::ops` 编排骨架

**Files:**
- Create: `crates/vole-core/src/ops/mod.rs`
- Modify: `crates/vole-core/src/lib.rs`
- Modify: `crates/vole-core/Cargo.toml`（`anyhow`, `thiserror`, `crossbeam-channel`）

**Interfaces:**
- Produces:
  - `ops::Orchestrator` 持有 `CancelToken` + 可选 `crossbeam_channel::Sender<StreamEvent>`
  - `ops::Orchestrator::emit(&self, event: StreamEvent)` — 若有 sender 则 `send`，否则忽略
  - `ops::Orchestrator::check_cancel(&self) -> Result<(), OpsError>` — 取消时 `OpsError::Cancelled`

本阶段**不**接子命令；只证明编排层能发 NDJSON 事件。

- [ ] **Step 1: 测试**

channel 收到 `Progress` 事件；cancel 后 `check_cancel` 报错。

- [ ] **Step 2–4: 实现并测试**

- [ ] **Step 5: 提交**

```bash
git commit -m "$(cat <<'EOF'
feat(core): add ops orchestrator skeleton with event channel

CLI, TUI and conformance harness will share this module instead of
each command wiring its own progress plumbing.
EOF
)"
```

---

## Task 10: TCC 实测（deferred 文档 + ad-hoc 子集）

用户决策：**暂不购买 Developer ID**。完整矩阵推迟，但 Phase 1 验收要求「TCC 结论已文档化」。

**Files:**
- Create: `docs/findings/2026-07-phase1-tcc-deferred.md`
- Create: `scripts/tcc-adhoc-matrix.sh`

- [ ] **Step 1: 写 deferred 文档**

`docs/findings/2026-07-phase1-tcc-deferred.md` 须写明：

- 完整矩阵项（设计 4.1 表格）与 **deferred 原因**（无 Developer ID）
- Phase 0.5 已测项（ad-hoc 读 Containers 退出码 0）
- Phase 1 执行的 **ad-hoc 子集**结果（见 Step 2）
- **触发补测条件**：购买 Developer ID 后第一个 Sprint 跑完整矩阵

- [ ] **Step 2: ad-hoc 子集脚本**

`scripts/tcc-adhoc-matrix.sh` 跑：

```bash
cargo build -p vole-cli
codesign -s - -f target/debug/vole 2>/dev/null || true
# 探测项 1：读 Containers
ls "$HOME/Library/Containers" >/dev/null 2>&1; echo "containers: $?"
# 探测项 2：读 Caches
ls "$HOME/Library/Caches" >/dev/null 2>&1; echo "caches: $?"
# 探测项 3：重编译 cdhash
touch crates/vole-cli/src/main.rs && cargo build -q -p vole-cli && codesign -s - -f target/debug/vole
codesign -dv target/debug/vole 2>&1 | grep -i CDHash | head -1 || echo "no-cdhash-line"
```

把输出贴进 findings 文档。

- [ ] **Step 3: 更新设计文档 4.1**

在 `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` 4.1 末加一句：**Phase 1 完整矩阵 deferred，见 `docs/findings/2026-07-phase1-tcc-deferred.md`。**

- [ ] **Step 4: 提交**

```bash
git add docs/findings/2026-07-phase1-tcc-deferred.md scripts/tcc-adhoc-matrix.sh docs/wukong-code/specs/
git commit -m "$(cat <<'EOF'
docs: record deferred TCC matrix and ad-hoc probe results

Full matrix waits on Apple Developer ID; ad-hoc subset documents what
we can learn without it so Phase 1 acceptance stays honest.
EOF
)"
```

---

## Task 11: `sysinfo` 磁盘 I/O 实测

**Files:**
- Create: `docs/findings/2026-07-phase1-sysinfo-disk-io.md`
- Create: `crates/vole-sys/examples/disk_io_probe.rs`（一次性探针，可不进产品路径）

- [ ] **Step 1: 写探针**

```rust
// 打印 sysinfo::Disks 上每块的 read_bytes/write_bytes（若 API 存在）
// 对比 mole status 用的 gopsutil 字段是否可替代
```

- [ ] **Step 2: 运行并记录**

```bash
cargo run -p vole-sys --example disk_io_probe
```

结论写入 findings：**可用 / 不可用 / 需 IOKit**。若不可用，Phase 2 `status` 对磁盘 I/O 速率字段标为「仅存在性，数值待 IOKit」。

- [ ] **Step 3: 提交**

```bash
git add docs/findings/2026-07-phase1-sysinfo-disk-io.md crates/vole-sys/examples/
git commit -m "$(cat <<'EOF'
docs: probe sysinfo disk IO on macOS for status parity

Decides whether Phase 2 can rely on sysinfo or needs an IOKit path
for read/write bytes per disk.
EOF
)"
```

---

## Task 12: Phase 1 验收与收口

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
```

- [ ] **Step 2: 验收清单对照**

| 设计 Phase 1 验收项 | 本计划对应 |
|---|---|
| units Go 测试 100% 移植 | Task 3 |
| oplog 反向 `mo history` | Task 7 |
| TCC 结论文档化 | Task 10（deferred + ad-hoc） |
| `docs/protocol.md` 与 `vole-proto` 一致 | Task 1–2 |
| CI 依赖方向检查 | Phase 0 已有 |

- [ ] **Step 3: 更新 README**

在 `README.md`「范围」段落后加一句：Phase 1 进行中/完成后的能力边界（尚无 `status`/`analyze`/`clean` 功能，仅有基础设施）。

- [ ] **Step 4: 提交**

```bash
git add README.md
git commit -m "$(cat <<'EOF'
docs: note Phase 1 foundation scope in README

Clarifies the repo still has no user-facing commands beyond the
Phase 0 clean stub.
EOF
)"
```

---

## 完成判据

Plan 2（Phase 1）全部完成时：

- [ ] `cargo test --workspace` 与 CI 全绿。
- [ ] `docs/protocol.md` 存在且 `check-protocol-doc.sh` 通过。
- [ ] `internal/units` 测试表 100% 在 Rust 通过。
- [ ] `verify-oplog-mole.sh` 证明 mole 能解析 vole 写的 oplog。
- [ ] `vole-sys` 六个 trait 有 macOS 最小实现且可单元测试。
- [ ] `CancelToken`、超时表、`flock` 互斥、whitelist、oplog、`ops` 骨架均存在。
- [ ] TCC 与 sysinfo 结论写入 `docs/findings/`。
- [ ] 设计文档 4.1 指向 deferred 文档。

**不在本计划范围内**：`status`/`analyze`/`clean` 功能、TUI、规则引擎、真实删除、Developer ID 签名流水线。这些属于 Plan 3（Phase 2）及之后。

---

## 建议执行顺序

Tasks 1–2（协议）→ 3（units）→ 4–5（超时/互斥）→ 6–7（whitelist/oplog）→ 8（sys traits）→ 9（ops）→ 10–11（实测文档）→ 12（收口）。

预估净工期：**3 周**（与设计文档一致）；单人每周 4 有效日。
