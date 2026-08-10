# T8：status 抛光 + `optimize --whitelist` + 文档收口

- 日期：2026-08-10 20:12
- 状态：已批准（父 design §3 / §4.3 T8；会话指令：父批准即本窄 design 批准，不另等 OK）
- Mole 钉版：`third_party/mole-1.48.1`（`cmd/status/{view,prefs,main}.go`；`bin/optimize.sh` + `lib/manage/whitelist.sh` optimize 模式）
- 父权威：[`2026-08-10-1514-tui-t5-home-menu-roadmap-design.md`](2026-08-10-1514-tui-t5-home-menu-roadmap-design.md) §3 T8 / §4.3
- T7 现况：[`docs/releases/v2.11.0.md`](../../releases/v2.11.0.md)（包线 **2.11.0**）
- 包版本意图：合入时 **MINOR**（相对当前 `2.11.0` → **2.12.0**）
- **不 bump** `schema_version`

## 1. 结论

T8 收口 T5–T7 长尾，交付三块：

1. **`vole optimize --whitelist`**：mole 独立清单（任务 id，非路径），与 `clean --whitelist` 双轨同形，配置文件与 mole 兼容。
2. **`vole status` 动画 cat + prefs**：ASCII mole 行走动画；`k` 隐藏/显示并持久化；`c` 循环 CPU 核心展示数（`2→4→8→all`）并持久化。诚实 footer 只声明已接线键。
3. **文档收口**：README「TUI 交互对齐」与对照表勾满 T5–T8；清除「有意未接线 / 余项 T8 / 首页数字菜单 / missing TTY」类陈旧措辞；发 `docs/releases/v2.12.0.md`。

硬约束：

- 删除 / apply 漏斗零破坏；optimize whitelist **只跳过任务**，不另开删除路径。
- footer **禁止**虚标未接线键（延续 T3–T7）。
- cat/`k`/`c` **整包交付或整包不做**：不做半接线（例如 footer 写了 `K` 却无动画）。本 design 判定成本可控，**纳入本波**。
- 不 bump `schema_version`。

## 2. `optimize --whitelist`

### 2.1 产品语义（对齐 mole）

| 项 | 约定 |
|---|---|
| 配置路径 | `~/.config/mole/whitelist_optimize`（与 mole 同路径，可互读） |
| 行语义 | **任务 action id**（如 `dock_refresh`），不是文件系统路径 |
| 目录 | 自 `optimize_catalog()` 投影：`display_name|task_id`；display 用 catalog `title`（对齐 mole `MOLE_OPTIMIZE_WHITELIST_NAMES`） |
| 默认 | 无配置文件时 **空列表**（mole `DEFAULT_OPTIMIZE_WHITELIST_PATTERNS` 亦为空） |
| 跳过时机 | **plan 阶段**不把已白名单任务放入可执行候选；若 apply 仍见该 `rule_id`，按 `SkipReason::Whitelisted` 跳过（防御） |

### 2.2 CLI 面

挂在 `Command::Optimize`（与 clean 同形）：

| Flag | 行为 |
|---|---|
| `--whitelist` | TTY：`PaginatedMultiSelect` 管理；非 TTY：stderr 指引 add/remove/list，exit ≠0 |
| `--whitelist-add <id>` | 追加任务 id（未知 id → 明确错误） |
| `--whitelist-remove <id>` | 移除 |
| `--whitelist-list` | 打印当前列表 |

门控：

- 任一 whitelist 系 flag → **不进** T6 确认轨，也不走普通 plan/apply。
- `--whitelist` 与 `--plan` / `--apply` / `--json*` / `--plan-out` / `--task` **互斥**（clap `conflicts_with_all`）。
- `gate_interactive` 增加「非 whitelist 系」条件。

### 2.3 实现落点

| 落点 | 动作 |
|---|---|
| `vole-core::whitelist` | 扩展 optimize 模式：load/save/add/remove/list + `build_optimize_whitelist_menu` + catalog 投影；复用 `patterns_equivalent`（对 id 即精确字符串） |
| `vole-core::ops::optimize_plan` | `build_optimize_plan` 接受 whitelist 切片；`allow(task_id)` 时排除白名单 id |
| `vole-core::ops::optimize_apply` | apply 前若 task id 在 whitelist → `SkipReason::Whitelisted`（双保险） |
| `vole-cli::optimize` / `main.rs` | flags、接线、TTY 菜单（复用 clean whitelist 交互壳） |
| 测试 | core：load/save/menu/plan 跳过；cli：help + 非 TTY flag 路径（`VOLE_TEST_NO_AUTH=1`） |

### 2.4 验收

1. `vole optimize --whitelist-add dock_refresh` 后 `--plan` / 确认轨扫出的条目不含 `optimize:*:dock_refresh`。
2. TTY `--whitelist` 取消不写盘；确认选择后写 `whitelist_optimize`。
3. 非 TTY 裸 `--whitelist` 报错指引；`--whitelist-list` 可读。
4. 与 `clean --whitelist` 配置文件互不污染。

## 3. status 动画 cat + `k` / `c`

### 3.1 行为（对齐 mole `cmd/status`）

| 项 | 约定 |
|---|---|
| 动画 | 移植 mole `moleBody` / `moleBodyMirror` + 水平往返位移；帧推进挂在既有 ~33ms 重绘环；速度可随 CPU usage 略加快（对齐 mole「Higher CPU = faster」精神即可，不必像素级） |
| 布局 | header 下、卡片上；`cat_hidden` 时省略整块（不留空白占位行也可，但高度变化需可测） |
| `k` | 切换显示/隐藏，写 prefs `cat_hidden=true\|false` |
| `c` | 循环 `cpu_cores`：`2 → 4 → 8 → 0(all) → 2…`；CPU 卡 per-core 行按该上限截断（今日硬编码 `take(2)` 改为 prefs） |
| 窗口过矮 | 可选：渲染前若放不下则降级 `smallerCPUCores`（mole 有）；**最低要求**是 prefs 循环 + 截断正确，自动降级为加分项 |
| prefs 路径 | `~/.config/mole/status_prefs`（`key=value` 多行；写单键保留他键；失败静默，不崩 TUI） |
| footer | 接线后：`K Cat \| C Cores \| Q/Esc/Ctrl+C Quit`（文案可微调，但必须诚实） |

### 3.2 实现落点

| 落点 | 动作 |
|---|---|
| 新 `tui/status_cat.rs`（或 `status_prefs.rs`） | 帧数据、位移、prefs load/save、`next_cpu_cores` 纯逻辑（单测） |
| `tui/status_view.rs` | header/cat 区；CPU 卡接受 `cpu_cores_limit`；footer 入参或改 `status_footer` |
| `main.rs` `cmd_status_tui` | `anim_frame`、键 `k`/`c`、prefs 读写；JSON 路径不变 |
| 测试 | prefs roundtrip；`next_cpu_cores`；footer 含 K/C；cat 隐藏时渲染不含 mole 字形 |

### 3.3 降级闸门（won't-do）

若实现中发现动画与窄屏布局冲突导致无法稳定验收，则：

- **整包撤销** cat 渲染与 `k`/`c`（含 footer 声明）；
- 在 `docs/releases/v2.12.0.md` **诚实标注「status 动画 cat / prefs：won't-do（本波）」**；
- **仍交付** optimize whitelist + 文档收口。

禁止：footer 写了键、行为未接线。

## 4. 文档与版本

| 文件 | 动作 |
|---|---|
| `README.md` | TUI 表补 `optimize --whitelist`；status 行注明 cat/`k`/`c`（若交付）；成熟度改 **2.12.0**；去掉「余项 T8 / 有意未接线 status cat / optimize --whitelist」 |
| `docs/releases/v2.12.0.md` | 新发版说明（中文） |
| 父 roadmap / 旧 release 注 | 不改历史正文；本波 README 收口即可 |
| `Cargo.toml` | `2.12.0` |
| `Formula/vole.rb` | 发版后 pin sha256（独立 chore PR，对齐 T7 #129） |

## 5. 不做

| 项 | 说明 |
|---|---|
| bump `schema_version` | 无协议字段变化 |
| bash 像素级光标重绘 | 仍用 ratatui 立即模式 |
| status 其它 mole 键（未列） | 本波仅 Q + K + C |
| clean whitelist 行为变更 | 不动 |
| 半接线 cat | 见 §3.3 |

## 6. 验收总表

1. `VOLE_TEST_NO_AUTH=1 cargo test -p vole-core -p vole-cli`（whitelist + status prefs/cat 相关）
2. `./scripts/check-command-surface.sh --enforce`
3. `cargo fmt --all -- --check`
4. README / 对照表无「T8 余项」「首页数字菜单」「missing TTY」陈旧缺口表述
5. TTY 手工：`vole status` 见 cat、`k`/`c` 生效且重启保留；`vole optimize --whitelist` 可管理任务并影响后续 plan

## 7. 与前波关系

| 波次 | 版本 | 状态 |
|---|---|---|
| T5 | 2.9.0 | 已交付 |
| T6 | 2.10.0 | 已交付 |
| T7 | 2.11.0 | 已交付 |
| **T8** | **2.12.0（本波）** | 本文件 |
