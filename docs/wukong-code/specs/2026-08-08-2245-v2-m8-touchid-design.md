# Vole M8：`touchid` 高对齐设计

- 日期：2026-08-08 22:45
- 状态：已批准（会话默认批准；产品 v2 续篇权威规格下的专用 design）
- 包版本：本里程碑发版 **`2.3.0`**（规格 §5：M8 → MINOR）
- Mole 钉版：`third_party/mole-1.48.1`
- 依据：[`2026-08-08-2030-v2-cli-complete-design.md`](2026-08-08-2030-v2-cli-complete-design.md) §5 / §6.4；M4 findings [`../../findings/2026-08-v2-m4-cli-complete-spike.md`](../../findings/2026-08-v2-m4-cli-complete-spike.md) §3.4
- 对照：`bin/touchid.sh`

## 1. 结论

交付 **`vole touchid`**：PAM Touch ID 引导开关（对照 Mole `touchid.sh`）。  
编排在 `vole-core::ops`；`vole-cli` 薄前端。  
**等价两阶段**（非删除 ProtoPlan）：只读预览（`status` / `--plan` / `--dry-run`）→ 写入（`enable` / `disable`）。  
优先 **`sudo_local` + `pam_tid.so`**；legacy `/etc/pam.d/sudo` 有安全备份与回滚。  
测试 / CI：`VOLE_TEST_NO_AUTH` + 路径注入 / Fake writer；**禁止**验证路径触发真 Touch ID 或交互 sudo 挂起。  
功能就绪后 bump **`2.3.0`**；不得空 bump。

## 2. CLI 面

| 形态 | 行为 |
|---|---|
| `vole touchid status` | 只读：是否已配置 `pam_tid.so` |
| `vole touchid --plan` / `--dry-run` / `-n` | 只读：按当前状态或显式子命令预览将写入的目标与动作（不改文件） |
| `vole touchid enable` | 启用（可与 `--dry-run` 组合） |
| `vole touchid disable` | 禁用（可与 `--dry-run` 组合） |
| `vole touchid`（无子命令） | 交互：显示状态；Enter 切换 enable/disable；Q/ESC 退出（对齐 Mole 菜单） |
| `--json` | status / plan / apply 结果结构化输出 |

说明：

- **不**走 purge/installer 的 ProtoPlan 删除漏斗（PAM 编辑不是文件删除）。
- 「高对齐两阶段」= **plan/dry-run 预览 → enable/disable 写入**。
- `--plan` 与 `--dry-run`/`-n` 同义：永不写 PAM。
- 无子命令 + `--plan`：预览「若 Enter 将执行」的切换方向（已启用则预览 disable，否则 enable）。

交互菜单（裸 `vole`）增加 `touchid` 入口（调用 `touchid status` 或进入 touchid 子菜单；建议菜单项跑 `touchid status` 后提示用户可 `vole touchid`）。Shell 补全随 clap `Command::Touchid` 自动生成。

## 3. PAM 策略（钉死）

### 3.1 路径与常量

| 项 | 默认 | 测试注入 |
|---|---|---|
| sudo PAM | `/etc/pam.d/sudo` | `VOLE_PAM_SUDO_FILE` |
| sudo_local | `<dirname(sudo)>/sudo_local` | `VOLE_PAM_SUDO_LOCAL_FILE` |
| tid 行 | `auth       sufficient     pam_tid.so`（与 Mole 字节级一致） | — |
| legacy 备份后缀 | `<sudo>.vole-backup` | 同路径旁 |

### 3.2 已配置判定（`is_touchid_configured`）

1. 若 `sudo_local` 存在且含 `pam_tid.so` → 已配置  
2. 否则若 `sudo` 文件存在且含 `pam_tid.so` → 已配置  
3. 否则未配置  

### 3.3 启用路径（优先 sudo_local）

当 `sudo` 文件内容含 `sudo_local`（Sonoma+ 形态）：

1. 若 `sudo_local` 已含 tid：**noop 成功**；若 legacy `sudo` 仍含 tid → **清理 legacy**（写入去 tid 后的 sudo）  
2. 否则：创建或追加 `sudo_local`（新建时含 Mole 同款注释头 + tid 行；mode `444`、owner `root:wheel` 由特权安装体现）  
3. 若曾为 legacy 配置：启用成功后从 `sudo` 去掉 tid（迁移）

当 `sudo` **不含** `sudo_local`（legacy）：

1. 已配置 → noop  
2. 否则：若尚无 `<sudo>.vole-backup`，先复制当前 sudo 为备份；失败则 **abort**（不改 sudo）  
3. 在首段注释后插入 tid 行（对齐 Mole awk 语义）；`secure_install` 失败则尝试从备份恢复（见 §4）

### 3.4 禁用路径

1. 若 `sudo_local` 含 tid：去掉 tid 行并安装；若 `sudo` 仍含 tid → 一并清理  
2. 否则若 `sudo` 含 tid：确保有备份后去掉 tid 行并安装  
3. 都找不到 → 错误（与 `is_touchid_configured` 真时不应发生）

### 3.5 硬件支持（软提示）

生产 enable：可探测 `bioutil -r` / `uname -m`（arm64 视为支持）；不支持时**警告并要求确认**（交互）或在非交互下继续但 JSON/`coverage_note` 标明。  
测试：注入 `TouchIdSupport::{Supported, Unsupported, Unknown}`；**不**调用真 `bioutil`。

## 4. 安全回滚（写死）

| 场景 | 行为 |
|---|---|
| legacy enable 前备份失败 | **不修改** sudo；返回失败 |
| `secure_install` 失败且本轮创建了备份 | 若目标文件与备份不一致，用备份内容 `secure_install` 回滚；回滚失败 → 失败码 + 明确文案指向备份路径 |
| `secure_install` 失败且备份本轮未新建 | **不**覆盖既有备份；返回失败 |
| sudo_local 写入失败 | **不**半迁移清理 legacy（保持旧态）；返回失败 |
| dry-run / `VOLE_TEST_NO_AUTH` Live | **零写入** |

特权写入抽象：`PamInstall` trait（`install_file(src, dst)` + 可选 `copy_for_backup`）。  
Live：`sudo -n install -m 444 -o root -g wheel`（及备份用 `sudo -n cp`）；**禁止**无 `-n` 的交互 sudo；**禁止**为验证触发 Touch ID。  
Fake：直接写测试目录文件。

## 5. `VOLE_TEST_NO_AUTH` / 可测性

| 条件 | 行为 |
|---|---|
| `VOLE_TEST_NO_AUTH=1` + Live writer | enable/disable **短路**：不调用 sudo；返回 `Skipped` / 清晰人类文案（「test no auth」）；status/plan 仍可读注入路径 |
| 单元测试 | 默认 Fake writer + temp PAM 文件；完整 enable/disable/迁移/回滚 |
| CI | 永不挂起真 Touch ID / 密码框 |

## 6. 明确不做（本里程碑）

- ProtoPlan / 废纸篓 / 删除漏斗（无关）
- 非 sudo 的其它 PAM 服务扩展
- 真机 Touch ID 演示自动化（coverage 注明）
- update / remove
- 空 bump

## 7. 测试与验收

- 单元：sudo_local 优先 enable/disable；legacy 插入/备份；legacy→local 迁移清理；dry-run 零写；`VOLE_TEST_NO_AUTH` Live 零 sudo  
- CLI：`touchid status --json`；`enable --dry-run`；注入 PAM 路径下 Fake/`VOLE_TEST_NO_AUTH`  
- 菜单文案含 `touchid`；`--help` 列出命令；补全含 `touchid`  
- `scripts/check-command-surface.sh`：`touchid` 不再 MISSING  
- 版本：workspace / Formula / README / `docs/releases/v2.3.0.md` = **2.3.0**

## 8. 文件落点（预期）

```
crates/vole-core/src/ops/touchid.rs          # 状态 / plan / enable / disable
crates/vole-core/src/ops/mod.rs
crates/vole-cli/src/touchid.rs
crates/vole-cli/src/main.rs                 # Command::Touchid
crates/vole-cli/src/interactive.rs
crates/vole-cli/tests/touchid_cli.rs
docs/releases/v2.3.0.md
Cargo.toml / Formula/vole.rb / README.md
```

## 9. 文档阶段验收

- [x] 优先 sudo_local + pam_tid.so 写死
- [x] 等价两阶段（plan/dry-run → enable/disable）写死
- [x] 安全回滚与备份后缀写死
- [x] `VOLE_TEST_NO_AUTH` / 注入路径 / 禁真授权挂起写死
- [x] 菜单/补全 + **2.3.0** 非空 bump 写死
- [x] 明确不做与 Mole 对照入口写死
