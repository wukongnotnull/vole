# optimize `disk_verify` 设计（闸控轨 G5 · 推翻默认）

- 日期：2026-08-08 19:23
- 状态：已批准（用户明确「推翻默认并批准执行轨 G5」；本会话 design 落盘后直接实现）
- 依据：[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) §3.3 P5；计划 [`../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md) Task G5；Mole `opt_disk_verify`（`tasks.sh` ≈1185–1216）；G2–G4 范例
- 包版本意图：**1.46.0**（MINOR；相对 `1.45.0`）
- **不 bump** `schema_version`

## 1. 结论

将 catalog **`disk_verify.in_m3: false` → `true`**，主路径 22 → **23**（optimize 长尾清空）：

| 阶段 | 行为 |
|---|---|
| **plan** | 仅当 `VOLE_ENABLE_DISK_VERIFY=1` 时出 1 条 action sentinel；未 opt-in → 空（对齐 Mole 默认 skip） |
| **apply** | 再查 opt-in；未启用 → Ok noop；启用则超时跑 `diskutil verifyVolume /`（**永不** `repairVolume`） |
| **失败** | 超时 / 无法启动 / 非 TestMode 不可用 → `Failed`；`VOLE_TEST_NO_AUTH` → 永不真跑 diskutil（空 plan / apply Skipped） |

## 2. 风险（为何规格曾默认拒绝升必做）

| 风险 | 说明 | 本轨对策 |
|---|---|---|
| **可能长时间卡住系统** | Mole 注释：`diskutil verifyVolume` 在 APFS 不一致时触发内核级 I/O，**SIGKILL 也可能杀不掉**，整机可冻住 | **默认不执行**：须 `VOLE_ENABLE_DISK_VERIFY=1`（对齐 `MOLE_ENABLE_DISK_VERIFY`）；文档与 release 明示风险 |
| **超时不可靠** | 用户态超时只能杀进程；内核卡住时仍可能挂起 | 默认超时 **30s**（`VOLE_TIMEOUT_DISK_VERIFY_SEC`，对齐 Mole `MOLE_TIMEOUT_DISK_VERIFY_SEC`）；超时 → **Failed**（不宣称 OK）；仍须 fail-closed 承认「超时 ≠ 已解除卡死」 |
| **误修卷** | `repairVolume` 可破坏数据 | **禁止**任何 `repairVolume` / `repairDisk`；issues 时仅诊断完成（Ok），由用户自行决定是否手动 repair |
| **测试误伤真机** | CI/单测若真跑 verify | `VOLE_TEST_NO_AUTH` / Fake deps：**零** Live `diskutil` |

**默认曾拒绝的原因（规格 §3.3 P5）：** 偏诊断、ROI 低，且存在不可中断卡死面；本轨在显式推翻默认后仍保留 Mole 级 opt-in，不把「进主路径」等同于「每次 optimize 都扫盘」。

## 3. 超时 / 取消 / fail-closed

| 点 | 决策 |
|---|---|
| 超时 | Live：`diskutil verifyVolume /` 包在有界 wait；默认 30s；env 可覆盖 |
| 取消 | 超时后 `kill` 子进程并 `Failed`；**不**保证解除内核 I/O |
| fail-closed | 未知输出且无明确 OK → 仍 Ok「complete」（对齐 Mole 第三分支）；**超时 / spawn 失败** → Failed；issues（error/corrupt/invalid）→ Ok（诊断成功，不自动修） |
| 取消用户选择 | 用户可不选 plan 条目；未 opt-in 根本不出候选 |

## 4. 采纳路径

| 点 | 决策 |
|---|---|
| catalog | `in_m3: true`；主路径 **23**；长尾空 → optimize plan 不再附「Skipped … long-tail」注 |
| 模块 | `optimize/tasks/disk_verify.rs`：`DiskVerifyDeps` + Live + Fake |
| opt-in | `VOLE_ENABLE_DISK_VERIFY=1`（大小写/true/yes 同 spotlight env 惯例可接受 `1`） |
| plan | `~/.vole-optimize-action/disk_verify`；label `Disk Health`；`task_id=disk_verify` |
| apply | Live：`diskutil verifyVolume /`；解析 OK / issues / other；**禁止** repair* |
| `VOLE_TEST_NO_AUTH` | Live 空 plan；apply Skipped |
| coverage / 版本 | 补一句（opt-in + 超时 + 禁 repair）；**1.46.0** + release / README / Formula |

## 5. Mole 对照

| Mole | Vole |
|---|---|
| `MOLE_ENABLE_DISK_VERIFY=1` 才跑 | `VOLE_ENABLE_DISK_VERIFY=1` |
| dry-run skip | plan 无候选（未 opt-in）/ plan 有哨兵但不在 apply 前真跑 |
| `run_with_timeout` + `diskutil verifyVolume /` | 同命令 + 同默认 30s |
| OK / issues 警告 + 建议手动 repair / complete | Ok 各语义；issues **不**自动 repair |
| 无 sudo | 无 PrivilegeBackend；不引入交互 sudo |

## 6. 测试策略

- Catalog：`main.contains("disk_verify")`；`main.len()==23`
- plan：未 opt-in → 空；opt-in + Fake → 1 sentinel；TestMode → 空
- apply：Fake OK / issues → Ok；Fake timeout/unavailable → Failed；TestMode → Skipped；Recording 断言 **零** `diskutil`/`repairVolume` 字面调用路径
- 禁止：单测真跑破坏性 diskutil（含 verify 在 Live 路径上，由 `VOLE_TEST_NO_AUTH` 挡住）

## 7. 非目标

- D1 / SMAppService；`repairVolume`；改 clean 规则；交互式 sudo 新体系；把 disk_verify 做成无 opt-in 的默认每次执行
