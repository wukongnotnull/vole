# Vole M7：`installer` 高对齐设计

- 日期：2026-08-08 22:18
- 状态：已批准（会话默认批准；产品 v2 续篇权威规格下的专用 design）
- 包版本：本里程碑发版 **`2.2.0`**（规格 §5：M7 → MINOR）
- Mole 钉版：`third_party/mole-1.48.1`
- 依据：[`2026-08-08-2030-v2-cli-complete-design.md`](2026-08-08-2030-v2-cli-complete-design.md) §5 / §6；M4 findings [`../../findings/2026-08-v2-m4-cli-complete-spike.md`](../../findings/2026-08-v2-m4-cli-complete-spike.md) §3.3
- 对照：`bin/installer.sh`；bats `installer.bats` / `installer_fd.bats` / `installer_zip.bats`

## 1. 结论

交付 **`vole installer`** 高对齐主路径：扫描安装包 → `--plan` → `--apply`；JSON；保护层；默认废纸篓（可 `--permanent`）；交互菜单与 shell 补全。  
编排在 `vole-core::ops`；`vole-cli` 薄前端。镜像 **purge** 的 ProtoPlan 漏斗，**禁止**平行 `rm -rf`。  
功能就绪后 bump **`2.2.0`**（Cargo / Formula / README / releases）；不得空 bump。

## 2. CLI 面

| Flag | 行为 |
|---|---|
| （默认）/`--plan`/`--dry-run`/`-n` | 只产出候选，不删 |
| `--apply PLAN` | TTL + TOCTOU（identity）+ 漏斗删除 |
| `--permanent` | 仅与 `--apply`；永久删 |
| `--json` / `--json-stream` / `--plan-out` | 同 purge / uninstall |
| 测试注入 | `VOLE_INSTALLER_SCAN_ROOTS`（`:` 分隔绝对路径）或 Options 覆盖扫描根；可选 `VOLE_INSTALLER_SCAN_MAX_DEPTH`（默认 2） |

交互菜单增加 `installer --plan`。Shell 补全随 clap `Command::Installer` 自动生成。

退出语义（对齐 Mole `INSTALLER_EXIT_INCOMPLETE` 精神）：apply 中有失败条目时，报告仍输出；进程非零（建议 exit 1），人类文案标明 incomplete。

## 3. 扫描与候选

### 3.1 扩展名（钉死）

直接候选（非符号链接文件）：

- `.dmg` / `.pkg` / `.mpkg` / `.iso` / `.xip`

ZIP：仅当可读且「安装包载荷」探测通过——列出前 **50** 条条目（对齐 `MAX_ZIP_ENTRIES`），任一条匹配 `*.app` / `*.pkg` / `*.dmg` / `*.xip`（含路径内片段）则纳入。列表优先用系统 `zipinfo -1`，否则 `unzip -Z -1`；二者皆无则 **跳过 zip**（fail-closed，coverage 注明）。

### 3.2 扫描根（Mole `INSTALLER_SCAN_PATHS` 精神）

相对 `$HOME` 或绝对，**存在才扫**：

| 根 | 说明 |
|---|---|
| `$HOME/Downloads` | 主路径优先 |
| `$HOME/Desktop` | 主路径优先 |
| `$HOME/Documents` | |
| `$HOME/Public` | |
| `$HOME/Library/Downloads` | |
| `/Users/Shared` | 绝对 |
| `/Users/Shared/Downloads` | 绝对 |
| `$HOME/Library/Caches/Homebrew` | |
| `$HOME/Library/Mobile Documents/com~apple~CloudDocs/Downloads` | iCloud |
| `$HOME/Library/Containers/com.apple.mail/Data/Library/Mail Downloads` | Mail |
| `$HOME/Library/Application Support/Telegram Desktop` | |
| `$HOME/Downloads/Telegram Desktop` | |

默认 `max_depth = 2`（对齐 `INSTALLER_SCAN_MAX_DEPTH_DEFAULT`）。  
测试可用注入根替代整表，避免触碰真实 Downloads。

**禁区（永不候选 / 永不 apply）：**

- 本地 Time Machine / APFS 快照删除路径（本命令不涉及快照 apply）
- `/Library/Updates`、`/macOS Install Data`（即使误扫到也须被保护层拒绝）
- 符号链接跳过（对齐 Mole `[[ -L ]] && return`）

### 3.3 Plan 契约

- `schema_version`：**不 bump**（复用现有）
- `rule_id`：`installer:{ext}`（ext 小写无点，如 `installer:dmg` / `installer:zip`）
- `ttl_secs`：900（同 purge）
- 每条目捕获 `PlanEntryIdentity`（dev/ino/mtime）+ 字节大小写入 plan 既有字段；apply 时 `verify_plan_entry_for_apply`（**immutable delete-plan 校验精神**）
- `coverage_note`：诚实记录长尾——TTY 分页多选 UI、fd 专用扫描路径、部分冷门根在 CI/沙箱不可用等
- 人类/JSON 可附 `source` 友好名（Downloads / Desktop / Homebrew / …），字段放 entry 已有 metadata 或 human-only；不强制 bump schema

## 4. Apply

- 独立 `apply_installer_plan`：仅接受 `rule_id` 前缀 `installer:`
- 默认 `DeleteMode::Trash`；`--permanent` → Permanent
- 每条目：`verify_plan_entry_for_apply` → `mole_delete_verified`
- 大小/身份与 plan 不一致 → skip + 计入 failed/incomplete（对齐 Mole `changed since scan`）
- oplog `command = "installer"`；互斥 `try_lock_config("installer")`（或 `try_lock_installer`）
- **禁止**平行 `rm -rf` / 旁路漏斗

## 5. 明确不做（本里程碑）

- TTY 分页多选 / alt-screen 选择器（用 `--plan` JSON + 全量候选 apply；用户可用外部过滤 plan）
- 依赖 `fd` 的专用扫描分支（Rust `walkdir`/`std::fs` 即可）
- touchid / update / remove
- 空 bump：仅功能就绪后改 `2.2.0`

## 6. 测试与验收

- 单元：`installer_plan` 在 temp 根下发现 dmg/pkg；跳过 symlink；zip 含 `.app` 才入选；保护路径拒绝
- 单元：`installer_apply` 拒绝非 `installer:`；identity 变化 skip；trash 成功
- CLI：`installer --plan --json`（注入扫描根）产出条目；`--apply` + `MOLE_TEST_TRASH_DIR`
- 菜单文案含 `installer`；`--help` 列出命令
- 版本：workspace / Formula / README / `docs/releases/v2.2.0.md` = **2.2.0**

## 7. 文件落点（预期）

```
crates/vole-core/src/ops/installer_plan.rs
crates/vole-core/src/ops/installer_apply.rs
crates/vole-core/src/ops/mod.rs
crates/vole-core/src/mutex.rs              # try_lock_installer
crates/vole-cli/src/installer.rs
crates/vole-cli/src/main.rs               # Command::Installer
crates/vole-cli/src/interactive.rs
crates/vole-cli/tests/installer_cli.rs
docs/releases/v2.2.0.md
Cargo.toml / Formula/vole.rb / README.md
```

## 8. 文档阶段验收

- [x] 扫描扩展名与 INSTALLER_SCAN_PATHS 精神写死
- [x] plan/apply + immutable identity + 漏斗 + 废纸篓
- [x] 禁区与禁止平行删除写死
- [x] 主路径 / 长尾分界写死
- [x] `2.2.0` 绑定本里程碑、禁止空 bump
