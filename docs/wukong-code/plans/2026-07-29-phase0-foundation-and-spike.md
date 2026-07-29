# Phase 0 + 0.5：地基与风险 spike 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建成一套能对同一 fixture 双跑 mole 与 vole 并输出结构化 diff 的一致性测试基础设施，然后用它跑一次风险 spike，产出校准后的工期估算。

**Architecture:** 四个 Rust crate 的空骨架（`vole-proto` ← `vole-sys` ← `vole-core` ← `vole-cli`，依赖严格单向），加上一份打了「候选集 JSONL 输出」补丁的 mole v1.48.1 源码快照作为正确性基准。harness 在一次性 `HOME` fixture 上分别驱动两者，比对候选集合。本阶段的 vole 侧只需能输出一个空候选集——目的是打通链路，不是实现清理。

**Tech Stack:** Rust stable（`rust-toolchain.toml` 钉版本）、`clap`、`serde`、`serde_json`、`insta`、`bats-core`（跑 mole 基线）、GitHub Actions。

**参照设计文档：** `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md`（commit `d7f1355`）。本计划只覆盖该文档第 8 节的 Phase 0 与 Phase 0.5。

## Global Constraints

- 许可证：**GPL-3.0**。Vole 是 Mole（GPL-3.0）的衍生作品，无法双许可。
- 平台：仅 macOS。非 macOS target 必须 `compile_error!`。
- `unsafe` 只允许出现在 `vole-sys`。其余 crate 一律 `#![forbid(unsafe_code)]`。
- crate 依赖严格单向：`vole-cli` → `vole-core` → `vole-sys` → `vole-proto`。禁止反向与横向依赖。`vole-proto` 是叶子，不依赖任何 workspace 内 crate。
- 起步只建 4 个 crate。`rules` / `scan` / `ops` / `tui` 先做 module，满足拆分阈值（单 module > 2500 行、出现独立发布的外部消费者、编译成为迭代瓶颈）才拆。
- 上游基准钉死在 Mole v1.48.1 / commit `27123a964aa671d2e64222634d29d4bd2dc866ed`，不跟踪上游演进。
- 一致性测试的一切文件操作必须限定在 `VOLE_TEST_ROOT` 之下。越界即中止整个测试运行，不是警告。
- 交叉编译目标：`aarch64-apple-darwin` 与 `x86_64-apple-darwin`。
- 提交粒度：每个 Task 至少一次提交，Task 内的 RED→GREEN 各自成步。

---

## File Structure

本阶段结束时的仓库形态：

```
vole/
├── Cargo.toml                          # workspace 定义
├── rust-toolchain.toml                 # 钉住 Rust 版本
├── LICENSE                             # GPL-3.0（替换现有 Apache-2.0）
├── README.md                           # 含 Mole 归属声明
├── crates/
│   ├── vole-proto/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                  # PlanEntry / Candidate / SkipReason 的最小定义
│   ├── vole-sys/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                  # 平台守卫（compile_error!），本阶段仅此
│   ├── vole-core/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                  # 本阶段仅 re-export，占位分层
│   └── vole-cli/
│       ├── Cargo.toml
│       └── src/main.rs                 # clap 入口 + `clean --plan --json-stream` 空实现
├── conformance/
│   ├── Cargo.toml                      # 独立的 harness 二进制
│   ├── src/main.rs                     # 双跑 diff 主流程
│   ├── src/guard.rs                    # VOLE_TEST_ROOT 护栏
│   ├── src/fixture.rs                  # fixture 树构造
│   └── fixtures/
│       └── smoke.json                  # 第一个 fixture 定义
├── scripts/
│   ├── check-dep-direction.sh          # 分层规则的 CI 检查
│   └── setup-mole-baseline.sh          # vendor + 打补丁 + 保真度验证
├── third_party/
│   ├── mole-1.48.1/                    # 源码快照（pristine 一次提交，补丁另一次提交）
│   ├── mole-1.48.1.SOURCE              # 记录上游 SHA 与获取方式
│   └── patches/
│       └── 001-conformance-jsonl.patch # 候选集 JSONL 输出补丁
├── docs/
│   ├── wukong-code/specs/…             # 已有设计文档
│   ├── wukong-code/plans/…             # 本计划
│   └── findings/                       # Phase 0.5 spike 的结论文档
└── .github/workflows/ci.yml
```

职责边界说明：`conformance/` 是独立 crate 而非 `vole-cli` 的测试，因为它要以外部进程方式驱动两个二进制，不能是单元测试。`scripts/` 只放 CI 与一次性环境准备，不放业务逻辑。

---

## Task 1: 许可证与归属

`vole` 仓库当前是 Apache-2.0，与 GPL-3.0 的衍生作品要求冲突。这必须是第一个提交，早于任何实质代码。

**Files:**
- Modify: `LICENSE`（整体替换）
- Modify: `README.md`
- Create: `scripts/check-license.sh`
- Test: `scripts/check-license.sh`

**Interfaces:**
- Consumes: 无
- Produces: `scripts/check-license.sh`，退出码 0 表示合规，供 Task 3 的 CI 调用

- [ ] **Step 1: 写失败的检查脚本**

`scripts/check-license.sh`：

```bash
#!/usr/bin/env bash
# 校验许可证与归属声明存在。GPL-3.0 是硬要求，见设计文档第 2 节。
set -euo pipefail

fail=0

if ! grep -q 'GNU GENERAL PUBLIC LICENSE' LICENSE; then
    echo "FAIL: LICENSE 不是 GPL" >&2
    fail=1
fi

if ! grep -q 'Version 3, 29 June 2007' LICENSE; then
    echo "FAIL: LICENSE 不是 GPL-3.0" >&2
    fail=1
fi

if grep -q 'Apache License' LICENSE; then
    echo "FAIL: LICENSE 仍含 Apache 文本" >&2
    fail=1
fi

if ! grep -qi 'tw93/Mole' README.md; then
    echo "FAIL: README 缺少 Mole 归属" >&2
    fail=1
fi

if ! grep -qi 'GPL-3.0' README.md; then
    echo "FAIL: README 未声明 GPL-3.0" >&2
    fail=1
fi

[[ $fail -eq 0 ]] && echo "OK: 许可证与归属检查通过"
exit $fail
```

- [ ] **Step 2: 运行确认失败**

```bash
chmod +x scripts/check-license.sh && ./scripts/check-license.sh
```

预期：以非 0 退出，输出 `FAIL: LICENSE 不是 GPL`、`FAIL: LICENSE 仍含 Apache 文本`、`FAIL: README 缺少 Mole 归属`、`FAIL: README 未声明 GPL-3.0`。

- [ ] **Step 3: 替换 LICENSE**

```bash
curl -fsSL https://www.gnu.org/licenses/gpl-3.0.txt -o LICENSE
head -3 LICENSE
```

预期首三行包含 `GNU GENERAL PUBLIC LICENSE` 与 `Version 3, 29 June 2007`。

- [ ] **Step 4: 写 README**

`README.md` 全文替换为：

```markdown
# Vole

用 Rust 实现的 macOS 清理与监控命令行工具，目前处于设计阶段，尚无可用版本。

## 与 Mole 的关系

Vole 的清理规则知识、路径保护清单与安全校验语义来自 [tw93/Mole](https://github.com/tw93/Mole) v1.48.1，
是 Mole 的衍生作品。感谢 Mole 作者与贡献者多年积累的这份知识。

Vole 是一个独立项目，不隶属于 Mole，也不与 Mole 保持功能对齐。如果你想要一个成熟可用的工具，
请直接使用 Mole。

## 许可证

GPL-3.0。因为 Vole 是 GPL-3.0 作品的衍生作品，这是唯一可选的许可证——包括未来的桌面 app 在内，
本项目的所有部分都以 GPL-3.0 发布。

## 范围

计划中的 v1 只实现 Mole 十二个子命令中的三个：`status`、`analyze`、`clean`。
设计文档见 `docs/wukong-code/specs/`。
```

- [ ] **Step 5: 运行检查确认通过**

```bash
./scripts/check-license.sh
```

预期：`OK: 许可证与归属检查通过`，退出码 0。

- [ ] **Step 6: 提交**

```bash
git add LICENSE README.md scripts/check-license.sh
git commit -m "$(cat <<'EOF'
chore: relicense to GPL-3.0 and credit Mole as the source

Vole derives its cleanup rules and path protection semantics from Mole,
which is GPL-3.0, so GPL-3.0 is the only available license. Doing this
before any real code avoids needing per-contributor consent later.
EOF
)"
```

---

## Task 2: 工具链与 workspace 骨架

本机当前**没有安装 Rust**（已核实 `rustc` 与 `cargo` 均不存在），所以工具链安装是本任务的一部分。

**Files:**
- Create: `rust-toolchain.toml`
- Create: `Cargo.toml`
- Create: `crates/vole-proto/Cargo.toml`、`crates/vole-proto/src/lib.rs`
- Create: `crates/vole-sys/Cargo.toml`、`crates/vole-sys/src/lib.rs`
- Create: `crates/vole-core/Cargo.toml`、`crates/vole-core/src/lib.rs`
- Create: `crates/vole-cli/Cargo.toml`、`crates/vole-cli/src/main.rs`
- Create: `scripts/check-dep-direction.sh`

**Interfaces:**
- Consumes: 无
- Produces:
  - `vole_proto::SkipReason`（枚举，`serde` 可序列化，`snake_case` 重命名）
  - `vole_proto::Candidate { path: PathBuf, label: String }`
  - `vole_proto::SCHEMA_VERSION: u32 = 1`
  - 二进制 `vole`（`crates/vole-cli`）
  - `scripts/check-dep-direction.sh`，退出码 0 表示分层合规

- [ ] **Step 1: 安装工具链并钉版本**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default
source "$HOME/.cargo/env"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
rustc --version
```

把上一条命令输出的**实际版本号**写进 `rust-toolchain.toml`（不要照抄示例里的数字，用你机器上解析出的那个）：

```toml
[toolchain]
channel = "1.XX.Y"          # 替换为 rustc --version 的实际版本
components = ["rustfmt", "clippy"]
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
```

- [ ] **Step 2: 写 workspace 与四个 crate**

`Cargo.toml`：

```toml
[workspace]
resolver = "2"
members = ["crates/vole-proto", "crates/vole-sys", "crates/vole-core", "crates/vole-cli"]

[workspace.package]
version = "0.0.1"
edition = "2021"
license = "GPL-3.0-only"
repository = "https://github.com/wukong/vole"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

`crates/vole-proto/Cargo.toml`：

```toml
[package]
name = "vole-proto"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
```

`crates/vole-proto/src/lib.rs`：

```rust
//! 前端与 vole 之间的协议类型。
//!
//! 本 crate 是依赖图的叶子，不得依赖任何 workspace 内 crate，
//! 外部依赖也要压到最少——将来第三方前端只依赖它即可，不必背上整个 vole。
#![forbid(unsafe_code)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// NDJSON 协议版本。Phase 4 结束时冻结 v1，在那之前可自由破坏性修改。
pub const SCHEMA_VERSION: u32 = 1;

/// 一条规则未产出删除目标的原因。
///
/// 序列化字符串在 Phase 4 结束时随协议冻结，此后只能追加变体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    NeedsPrivilege,
    AppRunning,
    Whitelisted,
    DbLocked,
    PathVanished,
    TccDenied,
    Timeout,
}

/// 一个待删除的候选目标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub path: PathBuf,
    pub label: String,
}
```

`crates/vole-sys/Cargo.toml`：

```toml
[package]
name = "vole-sys"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
vole-proto = { path = "../vole-proto" }
```

`crates/vole-sys/src/lib.rs`：

```rust
//! 平台抽象与 macOS 后端。这是 workspace 内唯一允许出现 `unsafe` 的 crate。

#[cfg(not(target_os = "macos"))]
compile_error!("Vole 目前只支持 macOS。平台边界已是 trait，加其他平台请实现对应后端而非放宽此断言。");

/// 重导出协议类型，让上层 crate 无需直接依赖 vole-proto 即可使用。
pub use vole_proto;
```

`crates/vole-core/Cargo.toml`：

```toml
[package]
name = "vole-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
vole-sys = { path = "../vole-sys" }
```

`crates/vole-core/src/lib.rs`：

```rust
//! 路径校验、保护判定、文件操作、操作日志、配置与单位格式化。
//!
//! `rules` / `scan` / `ops` 目前是本 crate 的 module，达到拆分阈值再独立成 crate。
#![forbid(unsafe_code)]

pub use vole_sys::vole_proto;
```

`crates/vole-cli/Cargo.toml`：

```toml
[package]
name = "vole-cli"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "vole"
path = "src/main.rs"

[dependencies]
vole-core = { path = "../vole-core" }
clap.workspace = true
serde_json.workspace = true
```

`crates/vole-cli/src/main.rs`：

```rust
//! vole 命令行入口。本阶段只有能让一致性 harness 打通链路的最小骨架。
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vole", version, about = "macOS cleanup and monitoring")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 清理缓存与残留文件。
    Clean {
        /// 只产出候选集，不改动任何文件。
        #[arg(long)]
        plan: bool,
        /// 以 NDJSON 事件流输出到 stdout。
        #[arg(long = "json-stream")]
        json_stream: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Clean { plan, json_stream } => cmd_clean(plan, json_stream),
    }
}

fn cmd_clean(plan: bool, json_stream: bool) {
    // 规则引擎属于 Phase 4。本阶段候选集恒为空，目的是让 harness 有可驱动的对象。
    if plan && json_stream {
        println!(
            r#"{{"schema_version":{},"type":"done","candidates":0}}"#,
            vole_core::vole_proto::SCHEMA_VERSION
        );
    }
}
```

- [ ] **Step 3: 写分层检查脚本**

`scripts/check-dep-direction.sh`：

```bash
#!/usr/bin/env bash
# 固化设计文档 5.1 的分层规则：vole-cli → vole-core → vole-sys → vole-proto。
# 不加这道检查，方向一定会在某次「顺手 import 一下」中被破坏。
set -euo pipefail

fail=0

# 每个 crate 允许直接依赖的 workspace 内 crate。
allowed_for() {
    case "$1" in
        vole-proto) echo "" ;;
        vole-sys)   echo "vole-proto" ;;
        vole-core)  echo "vole-sys" ;;
        vole-cli)   echo "vole-core" ;;
        *)          echo "__unknown__" ;;
    esac
}

for manifest in crates/*/Cargo.toml; do
    crate=$(basename "$(dirname "$manifest")")
    allowed=$(allowed_for "$crate")

    # 抓 [dependencies] 里指向 workspace 内 crate 的 path 依赖。
    deps=$(grep -oE '^vole-[a-z]+ = \{ path' "$manifest" | awk '{print $1}' || true)

    for dep in $deps; do
        if [[ " $allowed " != *" $dep "* ]]; then
            echo "FAIL: $crate 不得依赖 $dep（允许：${allowed:-无}）" >&2
            fail=1
        fi
    done
done

[[ $fail -eq 0 ]] && echo "OK: crate 依赖方向合规"
exit $fail
```

- [ ] **Step 4: 验证构建、分层与平台守卫**

```bash
cargo build --workspace
chmod +x scripts/check-dep-direction.sh && ./scripts/check-dep-direction.sh
cargo build --target aarch64-apple-darwin --workspace
cargo build --target x86_64-apple-darwin --workspace
cargo run -p vole-cli -- clean --plan --json-stream
```

预期依次为：构建成功；`OK: crate 依赖方向合规`；两个 target 均构建成功；最后一条输出 `{"schema_version":1,"type":"done","candidates":0}`。

- [ ] **Step 5: 验证分层检查确实会失败**

检查脚本本身必须被证伪过一次，否则它可能永远返回 0。临时给 `vole-proto` 加一条非法依赖：

```bash
printf 'vole-sys = { path = "../vole-sys" }\n' >> crates/vole-proto/Cargo.toml
./scripts/check-dep-direction.sh; echo "退出码: $?"
```

预期：输出 `FAIL: vole-proto 不得依赖 vole-sys（允许：无）`，退出码 1。

然后还原：

```bash
git checkout crates/vole-proto/Cargo.toml 2>/dev/null || sed -i '' '$ d' crates/vole-proto/Cargo.toml
./scripts/check-dep-direction.sh
```

预期恢复为 `OK: crate 依赖方向合规`。

- [ ] **Step 6: 提交**

```bash
cat > .gitignore <<'EOF'
/target
EOF
git add Cargo.toml Cargo.lock rust-toolchain.toml crates/ scripts/check-dep-direction.sh .gitignore
git commit -m "$(cat <<'EOF'
feat: scaffold four-crate workspace with enforced dependency direction

Starts with four crates instead of eight because getting crate boundaries
wrong costs more than splitting late; rules/scan/ops/tui stay as modules
until they hit the documented thresholds.

vole-proto is the leaf so a future third-party frontend can depend on the
protocol alone, and a CI check keeps that direction from eroding.
EOF
)"
```

---

## Task 3: CI

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `scripts/check-license.sh`、`scripts/check-dep-direction.sh`
- Produces: 一个在 macOS runner 上跑 fmt / clippy / test / 双架构交叉编译 / 两个检查脚本的工作流

- [ ] **Step 1: 写工作流**

`.github/workflows/ci.yml`：

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    # macOS runner 是必需的：vole-sys 对非 macOS target 会 compile_error!。
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: License and attribution
        run: ./scripts/check-license.sh

      - name: Crate dependency direction
        run: ./scripts/check-dep-direction.sh

      - name: Format
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Test
        run: cargo test --workspace

      - name: Cross-compile aarch64
        run: |
          rustup target add aarch64-apple-darwin
          cargo build --workspace --target aarch64-apple-darwin

      - name: Cross-compile x86_64
        run: |
          rustup target add x86_64-apple-darwin
          cargo build --workspace --target x86_64-apple-darwin

  # 一致性测试的 apply 阶段用例故意不在 CI 里跑。
  # 它们会真实删除文件，只应在设计文档 7.0 描述的一次性环境中执行，
  # 跑完回滚快照。不要把它们加进上面的 job。
  conformance-plan-only:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: Placeholder until Task 8 lands the harness
        run: echo "harness 在 Task 8 接入"
```

- [ ] **Step 2: 本地复现 CI 的全部检查**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-license.sh
./scripts/check-dep-direction.sh
```

预期：全部通过。若 `cargo fmt --check` 报错，先跑 `cargo fmt --all` 再重跑。

- [ ] **Step 3: 提交**

```bash
git add .github/workflows/ci.yml
git commit -m "$(cat <<'EOF'
ci: run fmt, clippy, tests and both darwin targets on macOS runners

Notes in the workflow why destructive conformance cases must stay out of
CI, so nobody enables them later without reading the design doc.
EOF
)"
```

---

## Task 4: vendor mole 基准快照

设计文档要求把 v1.48.1 的源码 vendor 进仓库而不是用 pinned clone。理由是整个项目的正确性基准都建立在这份快照上，若上游仓库变更或消失，pinned clone 会失效，而这个项目要跑数月。两者同为 GPL-3.0，vendor 无法律障碍。

**Files:**
- Create: `third_party/mole-1.48.1/`（约 56k 行）
- Create: `third_party/mole-1.48.1.SOURCE`

**Interfaces:**
- Consumes: 无
- Produces: `third_party/mole-1.48.1/`，pristine 状态；`bin/clean.sh` 的 `safe_clean` 定义在第 879 行

- [ ] **Step 1: 取快照并剥离 git 元数据**

```bash
mkdir -p third_party
git clone https://github.com/tw93/Mole.git /tmp/mole-vendor
cd /tmp/mole-vendor
git checkout 27123a964aa671d2e64222634d29d4bd2dc866ed
git rev-parse HEAD
cd -
rsync -a --exclude '.git' /tmp/mole-vendor/ third_party/mole-1.48.1/
rm -rf /tmp/mole-vendor
```

预期 `git rev-parse HEAD` 输出 `27123a964aa671d2e64222634d29d4bd2dc866ed`。

- [ ] **Step 2: 记录来源**

`third_party/mole-1.48.1.SOURCE`：

```
Upstream:  https://github.com/tw93/Mole
Version:   v1.48.1
Commit:    27123a964aa671d2e64222634d29d4bd2dc866ed
Retrieved: 2026-07-29
License:   GPL-3.0

用途：作为 Vole 一致性测试的正确性基准。不参与 Vole 的构建，
也不用于跟踪上游演进——Vole 一次性取走规则知识后独立发展。

本目录内容为 pristine 上游代码。测试专用补丁在 third_party/patches/
下单独维护并作为独立 commit 应用，以便 `git show` 即可审查补丁 diff。
```

- [ ] **Step 3: 验证快照完整且关键位置符合预期**

```bash
test -f third_party/mole-1.48.1/bin/clean.sh && echo "clean.sh 存在"
sed -n '879p' third_party/mole-1.48.1/bin/clean.sh
grep -c 'safe_clean ' third_party/mole-1.48.1/lib/clean/*.sh third_party/mole-1.48.1/bin/clean.sh | awk -F: '{s+=$2} END {print "safe_clean 调用点总数:", s}'
grep -c '^@test' third_party/mole-1.48.1/tests/*.bats | awk -F: '{s+=$2} END {print "bats 用例总数:", s}'
```

预期：`clean.sh 存在`；第 879 行为 `safe_clean() {`；调用点总数 **547**；bats 用例总数 **1157**。这三个数字是设计文档第 3 节的依据，对不上说明取到了错误的版本，必须停下来查。

- [ ] **Step 4: 提交 pristine 快照**

```bash
git add third_party/mole-1.48.1 third_party/mole-1.48.1.SOURCE
git commit -m "$(cat <<'EOF'
chore: vendor pristine Mole v1.48.1 as the correctness baseline

Vendored rather than pinned-cloned because every conformance assertion in
this project is measured against this snapshot, and it needs to survive
upstream force-pushes or deletion over a multi-month effort.

Kept pristine in this commit so the test-only patch lands separately and
stays reviewable via git show.
EOF
)"
```

---

## Task 5: mole 一致性补丁与保真度验证

harness 需要 mole 以机器可读形式吐出候选集。mole 的 `clean --dry-run` 只有人类可读输出，所以要打补丁。

**补丁点已确定**：`bin/clean.sh` 的 `safe_clean` 里，`existing_paths` 累积循环结束处（第 973 行 `done` 之后）。那里的 `existing_paths` 正是「存在、未被 `should_protect_path` 拦下、未被 `is_path_whitelisted` 拦下、非 `holds_compiled_model_cache`」的集合——与 harness 要比对的候选集定义完全一致，且在此插入不需要碰任何控制流。

注意 `safe_clean` 是**变参**的：`safe_clean t1 t2 ... description`，最后一个参数是描述，前面全是目标。bats 测试里 `safe_clean() { echo "$1|$2"; }` 那种两参数假设只对常见情形成立，补丁不能沿用。

**Files:**
- Create: `third_party/patches/001-conformance-jsonl.patch`
- Modify: `third_party/mole-1.48.1/bin/clean.sh:973`
- Create: `scripts/verify-mole-patch.sh`

**Interfaces:**
- Consumes: Task 4 的 pristine 快照
- Produces: 打了补丁的 mole。设置 `VOLE_CONFORMANCE_OUT=<path>` 后，`mole clean --dry-run` 会向该文件追加 NDJSON，每行形如 `{"type":"candidate","path":"…","label":"…"}`

- [ ] **Step 1: 写补丁内容**

在 `third_party/mole-1.48.1/bin/clean.sh` 第 973 行的 `done` 之后、`debug_timer_end` 之前插入：

```bash
    # --- BEGIN vole conformance patch (test-only, not for distribution) ---
    # 只在 VOLE_CONFORMANCE_OUT 设置时输出，不改动任何控制流。
    # existing_paths 此刻正是通过全部 guard 的候选集。
    if [[ -n "${VOLE_CONFORMANCE_OUT:-}" ]]; then
        _vole_json_str() {
            local s=$1
            s=${s//\\/\\\\}
            s=${s//\"/\\\"}
            printf '"%s"' "$s"
        }
        local _vole_p
        for _vole_p in ${existing_paths[@]+"${existing_paths[@]}"}; do
            printf '{"type":"candidate","path":%s,"label":%s}\n' \
                "$(_vole_json_str "$_vole_p")" \
                "$(_vole_json_str "$description")" \
                >> "$VOLE_CONFORMANCE_OUT"
        done
    fi
    # --- END vole conformance patch ---
```

`_vole_json_str` 只转义反斜杠与双引号。路径里若含制表符或换行会产生非法 JSON——harness 在 Task 7 会断言 fixture 路径不含控制字符，把这个限制挡在上游。

- [ ] **Step 2: 导出为 patch 文件**

```bash
cd third_party/mole-1.48.1
git init -q && git add -A && git -c user.email=x -c user.name=x commit -qm base
# 此时手工编辑 bin/clean.sh 插入上面的代码块
git diff > ../patches/001-conformance-jsonl.patch
rm -rf .git
cd -
mkdir -p third_party/patches
wc -l third_party/patches/001-conformance-jsonl.patch
```

预期：patch 文件行数在 25–35 之间。远超此数说明误改了别处。

- [ ] **Step 3: 验证补丁保真度**

补丁改的是正确性基准本身，如果它改变了 mole 的行为，整套一致性测试的地基就是歪的。`scripts/verify-mole-patch.sh`：

```bash
#!/usr/bin/env bash
# 验证一致性补丁未改变 mole 行为：打补丁前后 bats 套件结果必须一致。
# 见设计文档第 7 节 A 类的「补丁保真度」要求。
set -euo pipefail

# 绝对路径先算好：下面要在子 shell 里 cd，相对路径会失效。
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MOLE_DIR="$REPO/third_party/mole-1.48.1"
PATCH="$REPO/third_party/patches/001-conformance-jsonl.patch"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

command -v bats >/dev/null || { echo "需要 bats：brew install bats-core" >&2; exit 1; }

# 基线：复制一份打了补丁的树，把补丁反向应用回 pristine。
rsync -a --exclude '.git' "$MOLE_DIR/" "$WORK/pristine/"
(cd "$WORK/pristine" && patch -p1 -R < "$PATCH" >/dev/null)

echo "=== 跑 pristine 基线 ==="
(cd "$WORK/pristine" && MOLE_TEST_NO_AUTH=1 ./scripts/test.sh) > "$WORK/before.log" 2>&1 || true
grep -oE '[0-9]+ tests?, [0-9]+ failures?' "$WORK/before.log" | tail -1 > "$WORK/before.summary"

echo "=== 跑打补丁版本 ==="
(cd "$MOLE_DIR" && MOLE_TEST_NO_AUTH=1 ./scripts/test.sh) > "$WORK/after.log" 2>&1 || true
grep -oE '[0-9]+ tests?, [0-9]+ failures?' "$WORK/after.log" | tail -1 > "$WORK/after.summary"

echo "补丁前: $(cat "$WORK/before.summary")"
echo "补丁后: $(cat "$WORK/after.summary")"

if diff -q "$WORK/before.summary" "$WORK/after.summary" >/dev/null; then
    echo "OK: 补丁未改变 bats 套件结果"
else
    echo "FAIL: 补丁改变了 bats 结果，基准不可信" >&2
    diff "$WORK/before.log" "$WORK/after.log" | head -50 >&2
    exit 1
fi
```

- [ ] **Step 4: 运行保真度验证**

```bash
brew install bats-core coreutils
chmod +x scripts/verify-mole-patch.sh
./scripts/verify-mole-patch.sh
```

预期：两行数字一致，输出 `OK: 补丁未改变 bats 套件结果`。

若两边都有相同数量的失败，那是环境问题（缺 `gtimeout`、缺 `fd` 等）而非补丁问题——**只要前后一致就算通过**，这正是用「前后对比」而非「全绿」作判据的原因。

- [ ] **Step 5: 验证补丁真的会输出**

```bash
export VOLE_TEST_ROOT=$(mktemp -d)
export HOME="$VOLE_TEST_ROOT/home"
mkdir -p "$HOME/Library/Caches/com.example.app"
dd if=/dev/zero of="$HOME/Library/Caches/com.example.app/blob" bs=1k count=64 2>/dev/null
out="$VOLE_TEST_ROOT/candidates.ndjson"
VOLE_CONFORMANCE_OUT="$out" MOLE_TEST_NO_AUTH=1 \
  third_party/mole-1.48.1/bin/clean.sh --dry-run >/dev/null 2>&1 || true
wc -l < "$out"
head -3 "$out"
jq -e . "$out" >/dev/null && echo "OK: 输出是合法 JSON"
```

预期：行数大于 0；每行形如 `{"type":"candidate","path":"/…/Library/Caches/com.example.app","label":"…"}`；`jq` 校验通过。

- [ ] **Step 6: 提交**

```bash
git add third_party/patches third_party/mole-1.48.1/bin/clean.sh scripts/verify-mole-patch.sh
git commit -m "$(cat <<'EOF'
test: emit candidate set as NDJSON from Mole's safe_clean

Hooks the point where existing_paths has passed every guard, which is
exactly the candidate set the conformance harness compares, and inserts
without touching control flow so the baseline stays trustworthy.

verify-mole-patch.sh proves that by diffing bats results before and after,
since a patch that changes behaviour would silently tilt every later
conformance assertion.
EOF
)"
```

---

## Task 6: VOLE_TEST_ROOT 护栏

一致性测试要驱动两个会真实删文件的程序，而 mole 的规则路径并不完全受 `$HOME` 约束。护栏必须先于 harness 存在。

**Files:**
- Create: `conformance/Cargo.toml`
- Create: `conformance/src/guard.rs`
- Create: `conformance/src/main.rs`
- Modify: `Cargo.toml`（加入 workspace members）

**Interfaces:**
- Consumes: 无
- Produces:
  - `guard::Guard::new(root: &Path, sentinels: &[PathBuf]) -> io::Result<Guard>`
  - `guard::Guard::assert_no_outside_changes(&self) -> Result<(), GuardViolation>`
  - `guard::GuardViolation { pub path: PathBuf, pub kind: ViolationKind }`
  - `guard::ViolationKind { Modified, Removed, Created }`

- [ ] **Step 1: 写失败的测试**

`conformance/src/guard.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// 护栏必须能发现根目录之外的改动，否则它形同虚设。
    #[test]
    fn detects_modification_outside_root() {
        let tmp = std::env::temp_dir().join(format!("vole-guard-{}", std::process::id()));
        let root = tmp.join("root");
        let outside = tmp.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let sentinel = outside.join("sentinel");
        std::fs::write(&sentinel, b"before").unwrap();

        let guard = Guard::new(&root, &[outside.clone()]).unwrap();

        // 模拟被测程序越界写入。
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&sentinel, b"after").unwrap();

        let violation = guard.assert_no_outside_changes().unwrap_err();
        assert_eq!(violation.path, sentinel);
        assert_eq!(violation.kind, ViolationKind::Modified);

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// 根目录内的改动是正常的，不得误报。
    #[test]
    fn allows_modification_inside_root() {
        let tmp = std::env::temp_dir().join(format!("vole-guard-ok-{}", std::process::id()));
        let root = tmp.join("root");
        let outside = tmp.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("sentinel"), b"x").unwrap();

        let guard = Guard::new(&root, &[outside.clone()]).unwrap();
        std::fs::write(root.join("scratch"), b"y").unwrap();

        assert!(guard.assert_no_outside_changes().is_ok());

        std::fs::remove_dir_all(&tmp).ok();
    }
}
```

- [ ] **Step 2: 运行确认失败**

`conformance/Cargo.toml`：

```toml
[package]
name = "conformance"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "conformance"
path = "src/main.rs"

[dependencies]
serde.workspace = true
serde_json.workspace = true
```

`Cargo.toml` 的 members 改为：

```toml
members = ["crates/vole-proto", "crates/vole-sys", "crates/vole-core", "crates/vole-cli", "conformance"]
```

`conformance/src/main.rs` 暂时：

```rust
#![forbid(unsafe_code)]

mod guard;

fn main() {
    eprintln!("harness 在 Task 7 实现");
    std::process::exit(1);
}
```

```bash
cargo test -p conformance
```

预期：编译失败，报 `cannot find struct Guard`、`ViolationKind` 未定义。

- [ ] **Step 3: 实现护栏**

`conformance/src/guard.rs` 顶部加入：

```rust
//! 一致性测试的越界护栏。
//!
//! 被测程序（mole 与 vole）都会真实删除文件，而 mole 的规则路径并不完全
//! 受 $HOME 约束。护栏对若干哨兵目录做改动前后的快照对比，任何根目录之外
//! 的变化都中止整个测试运行——不是警告。

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, PartialEq, Eq)]
pub enum ViolationKind {
    Modified,
    Removed,
    Created,
}

#[derive(Debug)]
pub struct GuardViolation {
    pub path: PathBuf,
    pub kind: ViolationKind,
}

pub struct Guard {
    root: PathBuf,
    /// 哨兵路径 → 初始 mtime。缺失表示当时不存在。
    snapshot: HashMap<PathBuf, Option<SystemTime>>,
}

impl Guard {
    /// `root` 是允许改动的唯一区域。`sentinels` 是要监视的根外目录，
    /// 递归一层收集条目——全盘快照太慢，哨兵覆盖 mole 实际会碰的位置即可。
    pub fn new(root: &Path, sentinels: &[PathBuf]) -> io::Result<Self> {
        let mut snapshot = HashMap::new();
        for dir in sentinels {
            collect(dir, &mut snapshot)?;
        }
        Ok(Guard {
            root: root.to_path_buf(),
            snapshot,
        })
    }

    pub fn assert_no_outside_changes(&self) -> Result<(), GuardViolation> {
        for (path, before) in &self.snapshot {
            if path.starts_with(&self.root) {
                continue;
            }
            let now = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            match (before, now) {
                (Some(_), None) => {
                    return Err(GuardViolation {
                        path: path.clone(),
                        kind: ViolationKind::Removed,
                    })
                }
                (None, Some(_)) => {
                    return Err(GuardViolation {
                        path: path.clone(),
                        kind: ViolationKind::Created,
                    })
                }
                (Some(a), Some(b)) if a != &b => {
                    return Err(GuardViolation {
                        path: path.clone(),
                        kind: ViolationKind::Modified,
                    })
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn collect(dir: &Path, out: &mut HashMap<PathBuf, Option<SystemTime>>) -> io::Result<()> {
    if !dir.exists() {
        out.insert(dir.to_path_buf(), None);
        return Ok(());
    }
    out.insert(
        dir.to_path_buf(),
        std::fs::metadata(dir).and_then(|m| m.modified()).ok(),
    );
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        out.insert(
            path.clone(),
            std::fs::metadata(&path).and_then(|m| m.modified()).ok(),
        );
    }
    Ok(())
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cargo test -p conformance
```

预期：`test guard::tests::detects_modification_outside_root ... ok` 与 `test guard::tests::allows_modification_inside_root ... ok`，共 2 passed。

第一个用例里有 1.1 秒的 sleep，因为 HFS+/APFS 的 mtime 粒度可能到秒级，写得太快前后 mtime 会相同而漏报。

- [ ] **Step 5: 提交**

```bash
git add conformance Cargo.toml Cargo.lock
git commit -m "$(cat <<'EOF'
test: add out-of-root guard for the conformance harness

Both programs under test delete files for real and Mole's rules reach
outside $HOME, so the harness needs a tripwire before it drives anything.

Includes a case that deliberately writes outside the root, because a guard
that has never failed might not work at all.
EOF
)"
```

---

## Task 7: 双跑 diff harness

**Files:**
- Create: `conformance/src/fixture.rs`
- Create: `conformance/fixtures/smoke.json`
- Modify: `conformance/src/main.rs`

**Interfaces:**
- Consumes: `guard::Guard`；Task 5 的 `VOLE_CONFORMANCE_OUT` 协议
- Produces:
  - `fixture::Fixture { pub id: String, pub entries: Vec<Entry> }`
  - `fixture::Fixture::materialize(&self, home: &Path) -> io::Result<()>`
  - harness 二进制：`conformance --fixture <path> --mole <path> --vole <path>`，diff 非空时退出码 1

- [ ] **Step 1: 写失败的测试**

`conformance/src/fixture.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materializes_dirs_and_files_with_mtime() {
        let home = std::env::temp_dir().join(format!("vole-fx-{}", std::process::id()));
        std::fs::remove_dir_all(&home).ok();

        let fx = Fixture {
            id: "t".into(),
            entries: vec![
                Entry::Dir {
                    path: "Library/Caches/com.example.app".into(),
                },
                Entry::File {
                    path: "Library/Caches/com.example.app/blob".into(),
                    size_kb: 4,
                },
            ],
        };
        fx.materialize(&home).unwrap();

        assert!(home.join("Library/Caches/com.example.app").is_dir());
        let blob = home.join("Library/Caches/com.example.app/blob");
        assert_eq!(std::fs::metadata(&blob).unwrap().len(), 4096);

        std::fs::remove_dir_all(&home).ok();
    }

    /// 补丁的 JSON 转义只处理反斜杠与引号，含控制字符的路径会产出非法 JSON。
    /// 把这个限制挡在 fixture 校验里，而不是等 harness 解析失败。
    #[test]
    fn rejects_control_characters_in_paths() {
        let fx = Fixture {
            id: "bad".into(),
            entries: vec![Entry::Dir {
                path: "Library/Ca\tches".into(),
            }],
        };
        let err = fx.validate().unwrap_err();
        assert!(err.contains("控制字符"), "实际错误: {err}");
    }
}
```

- [ ] **Step 2: 运行确认失败**

```bash
cargo test -p conformance fixture
```

预期：编译失败，`cannot find struct Fixture`。

- [ ] **Step 3: 实现 fixture**

`conformance/src/fixture.rs` 顶部：

```rust
//! fixture 树的声明与物化。
//!
//! fixture 用 JSON 声明而非脚本构造，这样从 Mole 的 bats 用例里
//! 半自动抽取出来的期望值可以直接落成数据（设计文档第 7 节 B 类）。

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    Dir { path: PathBuf },
    File { path: PathBuf, size_kb: u64 },
}

impl Entry {
    fn path(&self) -> &Path {
        match self {
            Entry::Dir { path } | Entry::File { path, .. } => path,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub entries: Vec<Entry>,
}

impl Fixture {
    pub fn validate(&self) -> Result<(), String> {
        for entry in &self.entries {
            let p = entry.path();
            let s = p.to_string_lossy();
            if s.chars().any(|c| c.is_control()) {
                return Err(format!("fixture {} 的路径含控制字符: {s:?}", self.id));
            }
            if p.is_absolute() || s.contains("..") {
                return Err(format!("fixture {} 的路径必须是不含 .. 的相对路径: {s:?}", self.id));
            }
        }
        Ok(())
    }

    pub fn materialize(&self, home: &Path) -> io::Result<()> {
        if let Err(e) = self.validate() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, e));
        }
        for entry in &self.entries {
            let full = home.join(entry.path());
            match entry {
                Entry::Dir { .. } => std::fs::create_dir_all(&full)?,
                Entry::File { size_kb, .. } => {
                    if let Some(parent) = full.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&full, vec![0u8; (*size_kb as usize) * 1024])?;
                }
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cargo test -p conformance fixture
```

预期：2 passed。

- [ ] **Step 5: 实现 harness 主流程**

`conformance/fixtures/smoke.json`：

```json
{
  "id": "smoke",
  "entries": [
    { "kind": "dir", "path": "Library/Caches/com.example.app" },
    { "kind": "file", "path": "Library/Caches/com.example.app/blob", "size_kb": 64 },
    { "kind": "dir", "path": "Library/Logs/com.example.app" },
    { "kind": "file", "path": "Library/Logs/com.example.app/run.log", "size_kb": 8 }
  ]
}
```

`conformance/src/main.rs`：

```rust
//! 一致性 harness：在同一 fixture 上分别驱动 mole 与 vole，比对候选集。
//!
//! 本阶段 vole 侧候选集恒为空（规则引擎属于 Phase 4），所以 diff 必然非空。
//! 这是预期结果——目的是证明链路通了，而不是证明两者一致。
#![forbid(unsafe_code)]

mod fixture;
mod guard;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CandidateLine {
    #[serde(rename = "type")]
    kind: String,
    path: Option<String>,
    label: Option<String>,
}

/// 比对单元，路径已归一化成 `$HOME/…` 形式。
///
/// 与 `vole_proto::Candidate` 同名容易混淆，故另起名字：那个用 `PathBuf` 且是
/// 协议类型，这个是 harness 内部的可比形式。
///
/// 设计文档 7A 要求比对路径、标签、归属规则、体积、跳过原因五个维度；
/// 本阶段只有前两个可得（mole 补丁只吐这两项），其余随 Phase 4 的规则引擎接入。
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedCandidate {
    path: String,
    label: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fixture_path = arg(&args, "--fixture").expect("需要 --fixture <path>");
    let mole = arg(&args, "--mole").expect("需要 --mole <path到 bin/clean.sh>");
    let vole = arg(&args, "--vole").expect("需要 --vole <path到 vole 二进制>");

    let root = std::env::var("VOLE_TEST_ROOT")
        .expect("必须设置 VOLE_TEST_ROOT。见设计文档 7.0：不要在开发机上跑。");
    let root = PathBuf::from(root);

    let fx: fixture::Fixture =
        serde_json::from_reader(std::fs::File::open(&fixture_path).expect("打不开 fixture"))
            .expect("fixture 不是合法 JSON");

    let real_home = PathBuf::from(std::env::var("HOME").expect("HOME 未设置"));
    let sentinels = vec![
        real_home.join("Library/Caches"),
        real_home.join("Library/Logs"),
        real_home.join("Desktop"),
        real_home.join("Documents"),
    ];
    let guard = guard::Guard::new(&root, &sentinels).expect("护栏初始化失败");

    let mole_set = run_mole(&mole, &root, &fx);
    let vole_set = run_vole(&vole, &root, &fx);

    if let Err(v) = guard.assert_no_outside_changes() {
        eprintln!("护栏触发：{:?} 发生 {:?}，中止。", v.path, v.kind);
        std::process::exit(2);
    }

    report_diff(&mole_set, &vole_set);
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn fresh_home(root: &Path, tag: &str, fx: &fixture::Fixture) -> PathBuf {
    let home = root.join(format!("{}-{}", fx.id, tag));
    std::fs::remove_dir_all(&home).ok();
    std::fs::create_dir_all(&home).expect("建 HOME 失败");
    fx.materialize(&home).expect("物化 fixture 失败");
    home
}

fn run_mole(mole: &str, root: &Path, fx: &fixture::Fixture) -> BTreeSet<NormalizedCandidate> {
    let home = fresh_home(root, "mole", fx);
    let out = home.join("candidates.ndjson");

    let status = Command::new(mole)
        .arg("--dry-run")
        .env("HOME", &home)
        .env("VOLE_CONFORMANCE_OUT", &out)
        .env("MOLE_TEST_NO_AUTH", "1")
        .env("MO_NO_OPLOG", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("启动 mole 失败");
    // mole 的退出码在部分环境下非 0（缺 gtimeout 等），不作为判据。
    eprintln!("mole 退出码 {:?}", status.code());

    parse_ndjson(&out, &home)
}

fn run_vole(vole: &str, root: &Path, fx: &fixture::Fixture) -> BTreeSet<NormalizedCandidate> {
    let home = fresh_home(root, "vole", fx);
    let out = home.join("candidates.ndjson");

    let output = Command::new(vole)
        .args(["clean", "--plan", "--json-stream"])
        .env("HOME", &home)
        .output()
        .expect("启动 vole 失败");
    std::fs::write(&out, &output.stdout).expect("写 vole 输出失败");

    parse_ndjson(&out, &home)
}

fn parse_ndjson(path: &Path, home: &Path) -> BTreeSet<NormalizedCandidate> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let home_str = home.to_string_lossy().to_string();
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<CandidateLine>(l).ok())
        .filter(|c| c.kind == "candidate")
        .map(|c| NormalizedCandidate {
            // 两侧 HOME 不同目录，必须归一化后才可比。
            path: c.path.unwrap_or_default().replace(&home_str, "$HOME"),
            label: c.label.unwrap_or_default(),
        })
        .collect()
}

fn report_diff(mole: &BTreeSet<NormalizedCandidate>, vole: &BTreeSet<NormalizedCandidate>) {
    let only_mole: Vec<_> = mole.difference(vole).collect();
    let only_vole: Vec<_> = vole.difference(mole).collect();

    println!("mole 候选 {} 项，vole 候选 {} 项", mole.len(), vole.len());
    if !only_mole.is_empty() {
        println!("\n仅 mole 有（{} 项）：", only_mole.len());
        for c in &only_mole {
            println!("  - {} | {}", c.path, c.label);
        }
    }
    if !only_vole.is_empty() {
        println!("\n仅 vole 有（{} 项）：", only_vole.len());
        for c in &only_vole {
            println!("  + {} | {}", c.path, c.label);
        }
    }

    if only_mole.is_empty() && only_vole.is_empty() {
        println!("\nOK: 候选集一致");
    } else {
        println!("\nDIFF: 候选集不一致");
        std::process::exit(1);
    }
}
```

- [ ] **Step 6: 跑通链路**

```bash
cargo build --workspace
export VOLE_TEST_ROOT=$(mktemp -d)
cargo run -p conformance -- \
  --fixture conformance/fixtures/smoke.json \
  --mole third_party/mole-1.48.1/bin/clean.sh \
  --vole target/debug/vole
echo "退出码: $?"
```

预期：打印 `mole 候选 N 项，vole 候选 0 项`（N > 0），列出「仅 mole 有」的条目，最后 `DIFF: 候选集不一致`，退出码 1。

**这个退出码 1 是本任务的成功标志**，不是失败。它证明 harness 能真实驱动两侧、能解析两种输出、能归一化路径、能算出差集。vole 侧为空是因为规则引擎在 Phase 4。

- [ ] **Step 7: 验证护栏在 harness 里生效**

```bash
VOLE_TEST_ROOT=/tmp/nonexistent-root cargo run -p conformance -- \
  --fixture conformance/fixtures/smoke.json \
  --mole third_party/mole-1.48.1/bin/clean.sh \
  --vole target/debug/vole 2>&1 | tail -3
```

预期：因 `VOLE_TEST_ROOT` 不存在而在建 HOME 时 panic，或护栏报错退出码 2。两者都可接受——要点是不会静默地在真实 `HOME` 上跑。

若发现它居然跑成功了，说明有路径没经过 root，必须停下来修。

- [ ] **Step 8: 接入 CI 并提交**

把 `.github/workflows/ci.yml` 的 `conformance-plan-only` job 替换为：

```yaml
  conformance-plan-only:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build
        run: cargo build --workspace
      - name: Run plan-stage conformance
        # 只跑 plan 阶段：不删任何文件。apply 阶段用例留给一次性环境。
        run: |
          export VOLE_TEST_ROOT=$(mktemp -d)
          # 本阶段 vole 候选集恒为空，diff 必然非空，退出码 1 是预期。
          # Phase 4 规则引擎接入后把 `|| true` 去掉。
          cargo run -p conformance -- \
            --fixture conformance/fixtures/smoke.json \
            --mole third_party/mole-1.48.1/bin/clean.sh \
            --vole target/debug/vole || true
```

```bash
git add conformance .github/workflows/ci.yml Cargo.lock
git commit -m "$(cat <<'EOF'
test: add dual-run diff harness for mole versus vole candidate sets

Normalizes each side's HOME before comparing so the two throwaway trees
are diffable, and refuses to run without VOLE_TEST_ROOT set.

Exits 1 right now because vole's rule engine lands in Phase 4; the point
of this task is proving the plumbing works, not that the sets match.
EOF
)"
```

---

## Task 8: 一次性测试环境文档

harness 现在能跑，但设计文档 7.0 要求它在一次性环境里跑。这一步产出可复现的环境准备说明。

**Files:**
- Create: `docs/testing-environment.md`
- Create: `scripts/new-test-user.sh`

**Interfaces:**
- Consumes: Task 7 的 harness
- Produces: 可复现的一次性环境准备流程

- [ ] **Step 1: 写环境文档**

`docs/testing-environment.md`：

```markdown
# 一致性测试的执行环境

一致性测试驱动两个会真实删除文件的程序，且 Mole 的规则路径不完全受 `$HOME`
约束（系统级路径、`/private/var`、废纸篓、`defaults` 域）。**不要在日常开发机上跑。**

容器不可用：macOS 的 TCC、`launchctl`、废纸篓语义在 Linux 容器里无法复现，
用容器等于测了个假东西。

## 方案一：一次性本地用户账户（推荐，最轻）

```bash
./scripts/new-test-user.sh voletest
su - voletest
```

跑完后删除账户与家目录。适合 plan 阶段用例与日常迭代。

## 方案二：macOS VM（apply 阶段用例必须用这个）

用 [Tart](https://tart.run) 起一台 macOS VM，跑前打快照、跑完回滚：

```bash
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest vole-test
tart run vole-test
# 在 VM 内 clone 仓库并跑 harness
# 跑完：tart delete vole-test && tart clone ... 重建
```

## 分层规定

| 用例类型 | 环境 | 进 CI |
|---|---|---|
| plan 阶段（只读，不删文件） | 一次性用户账户或 CI runner | 是 |
| B 类表驱动（规则引擎单测） | 任意，只碰 `VOLE_TEST_ROOT` | 是 |
| C 类 property / fuzz | 任意 | 是 |
| apply 阶段（真实删除） | **仅 VM，跑完回滚快照** | **否** |

CI 的 macOS runner 本身是一次性的，适合前三类。apply 阶段用例不进 CI——
这条规定也写在 `.github/workflows/ci.yml` 的注释里。

## 护栏

harness 强制要求 `VOLE_TEST_ROOT`，并在每次调用前后对若干根外哨兵目录
做 mtime 快照对比。任何越界改动会以退出码 2 中止整个运行，不是警告。
护栏实现在 `conformance/src/guard.rs`，其自身的有效性由
`detects_modification_outside_root` 用例保证。
```

- [ ] **Step 2: 写账户创建脚本**

`scripts/new-test-user.sh`：

```bash
#!/usr/bin/env bash
# 创建一次性本地测试账户。需要管理员权限。
# 见 docs/testing-environment.md。
set -euo pipefail

user=${1:?用法: $0 <username>}

if id "$user" >/dev/null 2>&1; then
    echo "用户 $user 已存在。删除请用：sudo sysadminctl -deleteUser $user" >&2
    exit 1
fi

echo "将创建标准（非管理员）账户 $user。"
read -r -p "继续？[y/N] " ok
[[ "$ok" == "y" ]] || exit 1

sudo sysadminctl -addUser "$user" -fullName "Vole Test" -password - 

echo
echo "已创建。切换：su - $user"
echo "用完删除：sudo sysadminctl -deleteUser $user"
echo
echo "注意：新账户的 TCC 授权是全新的，首次跑 clean 会弹一批权限对话框。"
echo "这正是 Phase 0.5 要观测的行为之一。"
```

- [ ] **Step 3: 验证脚本可执行且拒绝重复创建**

```bash
chmod +x scripts/new-test-user.sh
./scripts/new-test-user.sh "$(whoami)"
```

预期：报 `用户 <你> 已存在`，退出码 1。不要真的创建账户来测试——这一步只验证前置检查。

- [ ] **Step 4: 提交**

```bash
git add docs/testing-environment.md scripts/new-test-user.sh
git commit -m "$(cat <<'EOF'
docs: define the throwaway environment conformance tests must run in

Records why containers cannot substitute for a macOS host and which case
categories are allowed into CI, so the destructive ones do not drift in.
EOF
)"
```

---

## Phase 0.5：风险 spike

以下四个 Task 的产出是**结论文档，不是代码**。设计文档第 10 节的工期建立在未验证假设上，这四步的目的是击穿最不确定的几项，然后校准估算。

每份结论写进 `docs/findings/`，并在 Task 12 汇总回设计文档。

---

## Task 9: spike A — 20 条规则的移植速率

这是全局最大不确定项。设计文档给 Phase 4c 的区间是 3–5 周，依据是猜测。

**Files:**
- Create: `docs/findings/2026-07-spike-rule-throughput.md`
- Create: `rules/spike/*.toml`（20 条规则，一次性产物，可在 Phase 4 重写）

**Interfaces:**
- Consumes: Task 7 的 harness
- Produces: 移植速率数据（条/天）与外推后的 Phase 4c 工期

- [ ] **Step 1: 选取 20 条代表性规则**

不要挑最简单的 20 条，那会得出乐观得毫无用处的数字。按设计文档 3.3 的策略分布配比：

| 类型 | 条数 | 从哪挑 |
|---|---|---|
| 纯路径（无策略） | 8 | `lib/clean/app_caches.sh` |
| `keep_newest_by_mtime` | 4 | `clean_dev_ai_agents`、`clean_dev_jetbrains_toolbox` |
| `not_running` guard | 3 | `clean_xcode_xctest_devices` 等 |
| symlink 保护 | 3 | `clean_dev_ai_agents` 的 active symlink 分支 |
| custom 候选 | 2 | `clean_xcode_simulator_runtime_volumes` |

记录挑选依据，Phase 4 分批移植时按同样配比分批。

- [ ] **Step 2: 逐条移植并记录耗时**

对每一条：写 TOML → 构造 fixture → 跑 harness 直到 diff 为空 → 记录净耗时（分钟）。

用一个表累积：

```markdown
| # | rule_id | 类型 | 净耗时(min) | 卡在哪 |
|---|---|---|---|---|
| 1 | chrome-cache | 纯路径 | 12 | — |
```

**必须记录「卡在哪」**，这一列比耗时更有价值——它会暴露策略集有没有抽象错。

- [ ] **Step 3: 外推并判定**

```
中位耗时 × 547 ÷ (每天有效分钟数) = Phase 4c 的天数
```

用中位数而非平均数，长尾会把平均数拉偏。

判定门槛（来自设计文档第 10 节止损判据）：

- 外推后 Phase 4c **≤ 6 周** → 按原计划推进。
- **> 6 周** → 进入 Phase 1 之前先调整方案：缩减规则范围（只做 Top N），或重新设计策略集。

- [ ] **Step 4: 写结论并提交**

`docs/findings/2026-07-spike-rule-throughput.md` 需包含：20 条的选取依据与配比、逐条耗时表含「卡在哪」、中位数与外推结果、`custom` 实际占比（若 20 条里超过 1 条就说明 5% 上限有风险）、以及对策略集枚举的修改建议。

```bash
git add docs/findings/2026-07-spike-rule-throughput.md rules/spike
git commit -m "$(cat <<'EOF'
docs: measure rule porting throughput on 20 representative rules

Samples across all four strategy classes rather than the easy cases, since
an optimistic number here would propagate into the whole Phase 4 estimate.
EOF
)"
```

---

## Task 10: spike B — 不可信 plan 校验的成本

设计文档 5.6 的 plan 威胁模型给 Phase 4d 估了 1 周。若明显超出，正确应对是退回单阶段 `clean` 而不是降低校验。

**Files:**
- Create: `docs/findings/2026-07-spike-untrusted-plan.md`
- Create: `crates/vole-core/src/spike_toctou.rs`（原型，Phase 4 重写）

**Interfaces:**
- Consumes: 无
- Produces: `openat` + `O_NOFOLLOW` + `(dev, ino, mtime)` 校验的可行性结论与工作量估计

- [ ] **Step 1: 写三个攻击用例**

三个都必须被拒绝。在 `crates/vole-core/src/spike_toctou.rs` 写：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// 每个用例一个独立根目录，避免相互干扰。
    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vole-toctou-{}-{}", tag, std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn identity_of(p: &std::path::Path) -> Identity {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(p).unwrap();
        Identity { dev: m.dev(), ino: m.ino() }
    }

    /// 攻击一：plan 里塞入一条当前文件系统上不存在的路径。
    /// verify 必须报错，调用方据此报错退出而非静默跳过。
    #[test]
    fn rejects_path_that_does_not_exist() {
        let root = scratch("nonexistent");
        std::fs::create_dir_all(root.join("Caches")).unwrap();

        let fake = Identity { dev: 1, ino: 999_999 };
        let err = verify(&root, std::path::Path::new("Caches/never-existed"), &fake).unwrap_err();
        assert!(err.contains("statat 失败"), "实际错误: {err}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// 攻击二：plan 生成后把末段换成指向敏感目录的 symlink。
    /// inode 不再匹配，必须拒绝。
    #[test]
    fn rejects_symlink_swapped_leaf() {
        let root = scratch("leaf");
        let cache = root.join("Caches");
        std::fs::create_dir_all(&cache).unwrap();
        let target = cache.join("blob");
        std::fs::write(&target, b"real").unwrap();

        // plan 生成时记录真实身份。
        let expect = identity_of(&target);
        assert!(verify(&root, std::path::Path::new("Caches/blob"), &expect).is_ok());

        // 攻击者把它换成 symlink。
        let sensitive = root.join("sensitive");
        std::fs::write(&sensitive, b"do not delete").unwrap();
        std::fs::remove_file(&target).unwrap();
        std::os::unix::fs::symlink(&sensitive, &target).unwrap();

        let err = verify(&root, std::path::Path::new("Caches/blob"), &expect).unwrap_err();
        assert!(err.contains("inode 不匹配"), "实际错误: {err}");
        assert!(sensitive.exists(), "敏感文件必须仍在");

        std::fs::remove_dir_all(&root).ok();
    }

    /// 攻击三：把路径的中间段换成 symlink。
    /// 这是最常见的绕过点——只检查末段的实现会在这里失守。
    #[test]
    fn rejects_symlink_swapped_intermediate() {
        let root = scratch("intermediate");
        let real_mid = root.join("Caches");
        std::fs::create_dir_all(real_mid.join("app")).unwrap();
        let target = real_mid.join("app/blob");
        std::fs::write(&target, b"real").unwrap();

        let expect = identity_of(&target);
        assert!(verify(&root, std::path::Path::new("Caches/app/blob"), &expect).is_ok());

        // 攻击者把中间段 Caches/app 换成 symlink，指向别处。
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("blob"), b"decoy").unwrap();
        std::fs::remove_dir_all(real_mid.join("app")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, real_mid.join("app")).unwrap();

        let err = verify(&root, std::path::Path::new("Caches/app/blob"), &expect).unwrap_err();
        assert!(
            err.contains("可能是 symlink"),
            "必须在打开中间段时就拒绝，实际错误: {err}"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
```

注意攻击二和攻击三的断言指向**不同**的拒绝原因：末段替换靠 inode 比对拦下，中间段替换靠 `O_NOFOLLOW` 在 `openat` 时就拦下。如果攻击三的错误信息里出现的是「inode 不匹配」而不是「可能是 symlink」，说明逐段遍历没生效——那是个真实缺陷，要在结论里写明。

- [ ] **Step 2: 用 rustix 实现最小校验原型**

给 `crates/vole-core/Cargo.toml` 的 `[dependencies]` 加 `rustix = { version = "1", features = ["fs"] }`，并在 `crates/vole-core/src/lib.rs` 加一行 `pub mod spike_toctou;`，然后在 `spike_toctou.rs` 的 `mod tests` **之前**写入：

```rust
//! plan 的 TOCTOU 校验原型。Phase 4 会重写，这里只为量出工作量。

use std::path::Path;

use rustix::fs::{Mode, OFlags};

/// plan 里记录的目标身份。
pub struct Identity {
    pub dev: u64,
    pub ino: u64,
}

/// 逐段打开路径，每一段都禁止跟随 symlink，最后比对身份。
///
/// 关键点：不用绝对路径字符串重新解析，那样等于把 TOCTOU 窗口又打开一次。
pub fn verify(root: &Path, relative: &Path, expect: &Identity) -> Result<(), String> {
    let mut dir = rustix::fs::open(root, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty())
        .map_err(|e| format!("打不开 root: {e}"))?;

    let mut components: Vec<_> = relative.components().collect();
    let leaf = components.pop().ok_or("空相对路径")?;

    for comp in components {
        let next = rustix::fs::openat(
            &dir,
            comp.as_os_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|e| format!("路径段 {comp:?} 打开失败（可能是 symlink）: {e}"))?;
        dir = next;
    }

    let st = rustix::fs::statat(&dir, leaf.as_os_str(), rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|e| format!("statat 失败: {e}"))?;

    if st.st_dev as u64 != expect.dev {
        return Err("跨设备，拒绝".into());
    }
    if st.st_ino as u64 != expect.ino {
        return Err("inode 不匹配，目标已被替换".into());
    }
    Ok(())
}
```

- [ ] **Step 3: 跑三个用例**

```bash
cargo test -p vole-core spike_toctou
```

预期：3 passed。若某个用例无法让 `verify` 拒绝，那就是真实的安全缺口，必须在结论里明确写出。

- [ ] **Step 4: 记录耗时并判定**

记录从 Step 1 到 Step 3 的净耗时。判定：

- 若 ≤ 2 天 → Phase 4d 的 1 周估算成立。
- 若 > 3 天 → 在结论里建议 v1 退回**单阶段交互式 `clean`**（扫描与删除同进程、不落 plan 文件），把 plan/apply 推到 v2。这不影响一致性测试，因为 A 类比对只需要 plan 阶段的输出，单阶段的 `--dry-run` 同样能提供。

- [ ] **Step 5: 写结论并提交**

`docs/findings/2026-07-spike-untrusted-plan.md` 需包含：三个攻击用例的实现与结果、`rustix` 的 `openat` 逐段遍历在 macOS 上是否按预期工作、净耗时、以及对 Phase 4d 是否保留两阶段的建议。

```bash
git add crates/vole-core/src/spike_toctou.rs crates/vole-core/Cargo.toml Cargo.lock docs/findings/2026-07-spike-untrusted-plan.md
git commit -m "$(cat <<'EOF'
docs: prototype TOCTOU verification to size the plan/apply split

Covers the intermediate-segment symlink swap alongside the leaf case,
since that is the easier one to miss and the more common bypass.
EOF
)"
```

---

## Task 11: spike C — 平台行为核实

四项待核实，全部只需观测，不需写产品代码。

**Files:**
- Create: `docs/findings/2026-07-spike-platform.md`

**Interfaces:**
- Consumes: 无
- Produces: TCC、签名、SQLite WAL、废纸篓四项的实测结论

- [ ] **Step 1: SQLite WAL 行为**

开着 Chrome，对其中一个 DB 走设计文档 5.2 的三条路径：

```bash
db="$HOME/Library/Application Support/Google/Chrome/Default/History"
ls -la "$db"* 2>/dev/null    # 看 -wal 是否存在且非空

# 路径一：immutable=1
sqlite3 "file:$db?immutable=1" 'select count(*) from urls;' 2>&1 | head -2
# 路径二：普通只读
sqlite3 "file:$db?mode=ro" 'select count(*) from urls;' 2>&1 | head -2
```

记录：`-wal` 是否存在、两条路径分别成功还是失败、若都成功则计数是否不同（不同即证实 `immutable=1` 读到过期快照）。

- [ ] **Step 2: 废纸篓口径**

```bash
mkdir -p "$VOLE_TEST_ROOT/trashtest" && dd if=/dev/zero of="$VOLE_TEST_ROOT/trashtest/blob" bs=1m count=50 2>/dev/null
df -k / | tail -1                        # 移动前可用空间
# 用 trash crate 的等价操作移入废纸篓（或先用 osascript 验证语义）
osascript -e "tell application \"Finder\" to delete POSIX file \"$VOLE_TEST_ROOT/trashtest/blob\"" >/dev/null
df -k / | tail -1                        # 移动后可用空间——预期几乎不变
ls -la ~/.Trash | tail -3
```

记录：移入废纸篓后可用空间是否变化（预期不变，这就是 5.7 那条口径规定的实证）。另外确认废纸篓内的文件如何计体积——这决定 `trashed_bytes` 怎么算。

- [ ] **Step 3: TCC 与签名（最小子集）**

只跑设计文档 4.1 矩阵的两行，重点是「重编译是否重弹窗」，因为它直接影响开发迭代效率：

```bash
# 未签名
cargo build -p vole-cli
codesign -dv target/debug/vole 2>&1 | head -3
ls "$HOME/Library/Containers" >/dev/null 2>&1; echo "未签名读 Containers 退出码: $?"

# ad-hoc 签名
codesign -s - -f target/debug/vole
codesign -dv target/debug/vole 2>&1 | grep -i 'signature\|hash'
ls "$HOME/Library/Containers" >/dev/null 2>&1; echo "ad-hoc 读 Containers 退出码: $?"

# 改一行代码重编译 + 重签，看 cdhash 是否变化
touch crates/vole-cli/src/main.rs && cargo build -p vole-cli && codesign -s - -f target/debug/vole
codesign -dv target/debug/vole 2>&1 | grep -i 'CandidateCDHash\|CDHash' | head -2
```

记录：cdhash 是否每次编译都变（若是，且 TCC 按 cdhash 授权，则开发期会反复弹窗，需要在 Phase 1 想办法，例如固定一个已授权的 wrapper 脚本）。

- [ ] **Step 4: Homebrew 签名政策**

查证 Homebrew 现行文档对 CLI 二进制的签名与公证要求，记录出处链接与结论。这一项是设计文档 5.5 明确标为待核实的。

- [ ] **Step 5: 写结论并提交**

`docs/findings/2026-07-spike-platform.md` 记录四项的实测数据与结论，并明确写出哪些验证了设计文档的假设、哪些推翻了。

```bash
git add docs/findings/2026-07-spike-platform.md
git commit -m "$(cat <<'EOF'
docs: verify SQLite WAL, trash accounting, TCC and Homebrew assumptions

Replaces four claims the design doc had marked as unverified with measured
behaviour, including whether recompiling changes cdhash enough to retrigger
TCC prompts during development.
EOF
)"
```

---

## Task 12: 校准估算并写回设计文档

spike 的最终产出。

**Files:**
- Modify: `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md`（第 4.1、5.2、5.5、5.6、10、11 节）
- Create: `docs/findings/2026-07-spike-summary.md`

**Interfaces:**
- Consumes: Task 9、10、11 的三份结论
- Produces: 校准后的工期表；关闭或更新的开放问题；下一份实施计划（Plan 2 / Phase 1）的输入

- [ ] **Step 1: 汇总三份结论**

`docs/findings/2026-07-spike-summary.md`：一页纸，三项结论各一段，然后一张「假设 → 实测 → 影响」的表。

- [ ] **Step 2: 更新设计文档的工期表**

用 Task 9 的外推结果替换 Phase 4c 的 3–5 周区间，重算净合计与含 buffer 预期。**必须在第 10 节的修订轨迹表里追加一行**，写明这次的修正原因——那张表的价值在于显示偏差方向，断了就没意义了。

- [ ] **Step 3: 更新待核实项与开放问题**

- 4.1：填入 TCC 实测结论（Task 11 Step 3）。
- 5.2 的 WAL 小节：把三条路径的预期换成实测行为。
- 5.5：把 Homebrew 签名从待核实降级为结论或明确的否定结论。
- 5.6：若 Task 10 判定 plan/apply 超预算，改为 v1 单阶段并说明理由。
- 第 11 节：关闭已回答的开放问题 2、3（`sysinfo` 那条留到 Phase 2），把答案移入「已关闭的问题」。

- [ ] **Step 4: 做出继续或调整的决策**

按设计文档第 10 节的止损判据明确写下结论，三选一：

1. **按原计划推进** Phase 1 → 写 Plan 2。
2. **调整方案后推进**（缩减规则范围 / 退回单阶段 clean）→ 先改设计文档，再写 Plan 2。
3. **收缩到只读范围**（只做 Phase 0–3，`clean` 继续用 mole）。

- [ ] **Step 5: 提交**

```bash
git add docs/
git commit -m "$(cat <<'EOF'
docs: recalibrate the estimate from spike measurements

Replaces the guessed Phase 4c range with the measured rule throughput and
appends to the revision trail, since three upward corrections in a row is
itself the most useful signal in that table.
EOF
)"
```

---

## 完成判据

Plan 1 全部完成时：

- [ ] `./scripts/check-license.sh` 与 `./scripts/check-dep-direction.sh` 均通过。
- [ ] `cargo fmt --check`、`cargo clippy -D warnings`、`cargo test --workspace` 均通过。
- [ ] 两个 darwin target 均能构建。
- [ ] `./scripts/verify-mole-patch.sh` 证明补丁未改变 mole 的 bats 结果。
- [ ] harness 能在一次性环境里驱动双跑并输出结构化 diff，退出码 1（vole 侧为空，符合预期）。
- [ ] 护栏被证伪过一次（`detects_modification_outside_root` 用例存在且通过）。
- [ ] `docs/findings/` 下有三份 spike 结论与一份汇总。
- [ ] 设计文档第 10 节的工期已按实测校准，修订轨迹表已追加一行。
- [ ] 已对「继续 / 调整 / 收缩」做出书面决策。

**不在本计划范围内**：任何规则引擎、任何指标采集、任何 TUI、任何真实删除。这些分别属于 Plan 2 及之后。
