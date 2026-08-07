# Install macOS\*.app 清理设计（system.sh 高风险刀）

- 日期：2026-08-07
- 状态：待用户审阅 spec → writing-plans
- 依据：Mole `third_party/mole-1.48.1/lib/clean/system.sh`（`software_update_pending_or_unknown` + Installer 循环）；coverage「仍未移植：Install macOS\*.app」；既有 Privilege / `ensure_privilege_ready` / `sudo -n` 永久删
- 包版本意图：能力扩展 → **`1.27.0`**（SemVer MINOR）；规则 **531 → 532**

## 1. 结论

在 **`vole clean` plan → apply** 中落地一条自定义规则 `install-macos-apps`：

- 扫描 **`{apps_root}/Install macOS*.app`**（生产 `apps_root=/Applications`；测试 `VOLE_TEST_APPLICATIONS` 重映射）
- 选入门控对齐 Mole：**SWU fail-closed** → **运行中跳过** → **当前 macOS 大版本 keep** → **age ≥ 14 天**
- apply：**重判门控** → `ensure_privilege_ready` → **永久 `sudo -n` 删**（不废纸篓）
- **永不**删除 `/Library/Updates`、`/macOS Install Data`

**采纳路径**：方案 A — 自定义 handler（可测注入 root / SWU plist）；非 TOML 裸 glob。

## 2. 问题与风险

1. **误删进行中的系统更新安装器**：macOS 27 beta 上，在「无更新」误判下清 staged payload 曾导致无法启动。故 SWU 探测必须 **fail-closed**（状态未知 = 整规则不选入）。
2. **误删当前大版本恢复安装器**：`DTPlatformVersion` 大版本与 `sw_vers -productVersion` 大版本相同 → keep。
3. **删正在跑的 Installer**：路径命中进程 → 跳过该项。
4. **十几 GB root 属应用**：只用特权永久删；plan 阶段零 sudo。
5. **协议**：不 bump `schema_version`；无新 skip reason（门控失败 = 不进 plan / apply 重判 skip）。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 自定义 handler（已选）** | select 内置全部门控；apply recheck + privilege | 与 GPU Metal / Rosetta 刀法一致；可单测 | 比 TOML 规则多几文件 |
| B. TOML glob + apply 钩子补门控 | plan 先枚举 glob | 规则声明短 | plan 易脏；SWU 难注入 |
| C. 本版只 plan / NeedsPrivilege 不真删 | 零风险删 | 用户无空间收益 | 不兑现 coverage |

## 4. 产品行为

```bash
vole clean --plan …          # 仅当 SWU=[] 且候选过 age/version/running → 进 plan
vole clean --apply plan.json # TTY 可 sudo -v 一次；仍 sudo -n 永久删 Installer
```

- analyze / status **不**新增「macOS installer 大文件提示」（Mole `user.sh` 那截本期不做）
- 不扫描 `$HOME/Applications`
- 人读 / JSON：与既有特权叶一致；无新字段

## 5. 选入门控（顺序写死）

对每个 `{apps_root}/Install macOS*.app` 目录（要求存在且为目录；根为 symlink → 跳过该项）：

1. **SWU（整规则级）**：读 Software Update plist 的 `RecommendedUpdates`  
   - 默认路径：`/Library/Preferences/com.apple.SoftwareUpdate.plist`  
   - 测试：可注入 plist 路径或内存字节（实现任选，契约写死）  
   - **仅**解析成功且值为 JSON 空数组 `[]`（忽略空白）→ 放行继续扫  
   - 缺文件 / 不可读 / plutil·解析失败 / 键缺失 / 非空数组 → **本规则零候选**（对齐 Mole `break`）
2. **运行中**：进程指向该 `.app` 路径（或等价 cmdline 命中）→ 跳过该项  
3. **版本 keep**：取 `sw_vers -productVersion` 的 **主版本**（第一个 `.` 前）；读 `Contents/Info.plist` 的 `DTPlatformVersion` 主版本；两者均非空且相等 → keep  
   - 缺 plist / 缺键 / 读失败 → **不 keep**（继续年龄，对齐 Mole）  
   - 测试：可注入「当前主版本」字符串（如 `VOLE_TEST_MACOS_MAJOR` 或依赖注入），避免真机 `sw_vers` 脆测
4. **年龄**：bundle 根 `mtime` 距今 **&lt; 14 天** → keep；≥ 14 → 候选  
5. 通过 → plan 条目：`rule_id=install-macos-apps`，**特权 + permanent**

**硬非目标路径**：`/Library/Updates`、`/macOS Install Data` —— 本规则代码路径不得引用删除。

## 6. 实现草图

### 6.1 模块

- `vole-core`：`install_macos`（或 `ops/install_macos.rs` / `custom` 下）  
  - `software_update_pending_or_unknown(plist) -> bool`（true = 阻塞清理）  
  - `select_install_macos_apps(...) -> Vec<PlanEntry>`  
  - allowlist 谓词：绝对路径、无 `..`、前缀 `{apps_root}/`、叶名匹配 `Install macOS*.app`
- 接入既有 plan custom handler 表与 apply `rule_id` 分支（同其他特权自定义规则）

### 6.2 环境注入

| 变量 | 用途 |
|---|---|
| `VOLE_TEST_APPLICATIONS` | 替代 `/Applications` 扫描根 |
| （可选同测）SWU plist 路径注入 | 测 fail-closed / `[]` |
| （可选）`VOLE_TEST_MACOS_MAJOR` | 固定版本 keep 判定 |

生产永不读测试变量。

### 6.3 Apply

1. 形状 + allowlist  
2. **重跑** SWU / 运行中 / 版本 / 年龄（与 select 同语义）  
3. `ensure_privilege_ready`  
4. `mole_delete_verified(..., needs_sudo: true, permanent)`  
5. 失败 → 既有 skip / NeedsPrivilege

### 6.4 明确不改

- `schema_version`  
- Mole `installer` 子命令、analyze 大文件行  
- `/Library/Updates`、`/macOS Install Data`  
- 桌面 SMAppService  

## 7. 覆盖说明

- coverage：去掉「Install macOS\*.app」未移植；**落地**句简述门控  
- 仍未移植：**桌面 SMAppService / 特权助手**（及若仍有的 system 长尾）  
- README：Mole 对比可一句提及 Installer 清理（可选，不阻塞）

## 8. 测试与安全

1. SWU `[]` + 过期无关大版本 app → 进 plan  
2. SWU 缺文件 / 非空 / 坏 plist → 零条目  
3. age &lt; 14 → 不进；大版本匹配 → 不进；运行中 → 不进  
4. apply recheck：select 后 SWU 变「pending」→ skip、不删  
5. allowlist 拒 `/tmp/Install macOS Foo.app` 等越界路径  
6. PR：**security-review** 必过（fail-closed、version keep、永不碰 Updates/Install Data、永久 sudo -n）

## 9. 验收

1. 本机有过期且非当前大版本的 Installer、SWU 明确无推荐更新时，plan 含该项；apply 可特权删除  
2. SWU 状态未知时计划为空（本规则）  
3. coverage / 规则数 532 / 版本 **1.27.0**；**默认不打 tag**

## 10. 下一步

用户批准本 spec 文件后 → `writing-plans` 产出 `docs/wukong-code/plans/…-install-macos-apps.md` → 再实现。
