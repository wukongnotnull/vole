# CLI `sudo -v` 凭证缓存设计（交互提权 · 第一刀）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：coverage「仍未移植：…交互提权 / 桌面特权助手」；既有 `PrivilegeBackend` / `SudoNoninteractive`（`sudo -n`）；v2 产品目标「CLI 与桌面提权双轨」（桌面 SMAppService **本期不落地**）
- 包版本意图：能力扩展 → **`1.26.0`**（SemVer MINOR）

## 1. 结论

在 **CLI `clean --apply`**（及走同一 `PrivilegeBackend` 的特权删除路径）上：

- **删除**仍只允许 **`sudo -n`**（非交互、可脚本、可 CI）
- 当 `probe_noninteractive()` 失败，且当前进程 **可交互** 时：至多调用一次 **`sudo -v`** 缓存管理员凭证，再重试 `probe`；成功后本轮后续特权删继续用 `sudo -n`
- **不可交互**（非 TTY / pipe / CI / `VOLE_TEST_NO_AUTH=1`）行为与今日完全一致：直接 `NeedsPrivilege` + 既有响亮提示

**采纳路径**：方案 A — TTY 下 `sudo -v` 再 `sudo -n`；不落地 SMAppService；不把删除改成交互 `sudo`。

## 2. 问题与风险

1. **误以为交互删更安全**：交互 `sudo /bin/rm` 会扩大面且难测。故 **删除永远 `-n`**，只缓存凭证。
2. **CI / 自动化回归**：若误在无 TTY 时弹 `sudo -v`，runner 会挂死。必须以「可交互」硬门控。
3. **多次弹窗**：一次 apply 可能有多条特权叶 → **整次 apply 至多一次** `sudo -v`（进程内闩）。
4. **桌面 app**：`vole-macos` 不经本路径；`PrivilegeBackend` 接缝注释保留，**零** SMAppService 实现。
5. **协议**：不 bump `schema_version`；不新增 skip reason（仍 `NeedsPrivilege`）。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. TTY `sudo -v` + 仍 `sudo -n` 删（已选）** | probe 失败且可交互 → 一次 `sudo -v` → 再 probe | 对齐用户「先缓存再删」心智；CI 安全 | 依赖 TTY/sudoers 时效 |
| B. 仅文案提示先手动 `sudo -v` | 不调 sudo | 改动最小 | 体验弱；coverage 难去掉「交互提权」 |
| C. TTY 下删除改交互 `sudo` | `sudo /bin/rm` | 少一步 | 不可脚本、测难、扩大攻击面 |

## 4. 产品行为

```bash
vole clean --apply plan.json          # TTY：可弹一次 sudo -v；其后 sudo -n 删
vole clean --apply plan.json < /dev/null   # 与今日同：NeedsPrivilege，不弹
echo '{}' | vole clean --apply -      # 非 TTY：同
VOLE_TEST_NO_AUTH=1 vole clean --apply …  # 强制无提权（测/CI）
```

- plan **永不**调用 `sudo` / `sudo -v`
- 特权删除：permanent + allowlist 形状闸口不变（Rosetta / diagnostic / private-var / …）
- 人读：stderr 可有一句「正在请求管理员权限以清理系统路径…」（失败则落到既有 `NeedsPrivilege` 响亮提示）
- `--json` / `--json-stream`：无新字段；成功删仍记入 report；失败仍 `NeedsPrivilege`

## 5. 实现

### 5.1 `PrivilegeBackend` 扩展（最小）

在 `crates/vole-core/src/privilege/`：

```rust
pub trait PrivilegeBackend {
    fn probe_noninteractive(&self) -> bool;
    /// 尝试交互缓存凭证（如 `sudo -v`）。默认 no-op / false。
    fn acquire_interactive(&self) -> bool { false }
    fn remove_permanent(&self, path: &Path) -> Result<(), PrivilegeError>;
}
```

- `SudoNoninteractive::acquire_interactive`：`Command::new("sudo").args(["-v"])`（参数分列）；仅当 `stdin().is_terminal()` 且未 `VOLE_TEST_NO_AUTH`；退出码 0 → true
- `NoPrivilege` / `RecordingPrivilege`：可测记录 `acquire` 调用次数

### 5.2 Apply 接线

在特权规则分支共用逻辑（抽出小 helper 亦可）：

1. `probe_noninteractive()`；若 true → 删
2. 否则若本 apply **尚未**尝试过 acquire，且 backend.acquire_interactive() → 再 probe；成功 → 删
3. 否则 `NeedsPrivilege`

「尚未尝试」用 `ApplyPlanContext` 上的 `bool` 闩（单次 apply）。

### 5.3 可交互判定

同时满足才允许 acquire：

- `std::io::stdin().is_terminal()`（可用 `std::io::IsTerminal`）
- 未设置 `VOLE_TEST_NO_AUTH=1`
- 可选：未设置 `VOLE_NONINTERACTIVE=1`（若已有同类环境则复用，否则本期可不增）

### 5.4 明确不改

- plan 候选 / TOML / allowlist / permanent
- SMAppService / vole-macos
- Install macOS\*.app 等高风险 system.sh 余项
- `schema_version`

## 6. 覆盖说明

- coverage：「交互提权（TTY 下 `sudo -v` 缓存后 `sudo -n`）」标为 **已落地**
- 仍未移植改为：`Install macOS*.app`（若确认仍为唯一 system.sh 余项）、**桌面 SMAppService / 特权助手**
- README Mole 对比句同步「CLI 可先 `sudo -v`」一句即可

## 7. 非目标

- 桌面 PrivilegedHelper / SMAppService
- 交互 `sudo rm`、图形化密码框、Touch ID
- plan 阶段任何 sudo
- 扩大 allowlist 或安装器清理

## 8. 测试与安全

1. Recording backend：probe false → acquire 一次 → probe true → remove 被记录；第二次 NeedsPrivilege 路径不二次 acquire
2. 非 TTY / `VOLE_TEST_NO_AUTH`：acquire 零调用
3. `sudo -v` 失败：NeedsPrivilege，无 remove
4. `safety::property` / 既有特权 apply 单测全绿
5. PR：**security-review**（凭证提示面、始终 `-n` 删除、闩）

## 9. 验收

1. 本机有 TTY、凭证过期：`clean --apply` 含特权叶时可弹一次密码，随后 `sudo -n` 删成功
2. CI / pipe：零交互、行为同 1.25.0
3. coverage / README；版本 **1.26.0**；**默认不打 tag**

## 10. 实现后文档

- `docs/releases/v1.26.0.md`、findings
- 桌面 SMAppService / Install macOS\*.app 另开 design
