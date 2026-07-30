# Vole SemVer 版本策略

- 日期：2026-07-30
- 状态：已批准（默认采纳推荐项）
- 背景：对外 tag 停在 `v0.0.11`，第三位被当发版序号；文档已宣告「v1 CLI 完成」，与包版本语义冲突；`Cargo.toml` workspace 仍为 `0.0.1`

## 1. 结论

采用严格 **SemVer `MAJOR.MINOR.PATCH`**。  
历史 `v0.0.1`–`v0.0.11` **不改写、不重标**；下一公开发版对齐为 **`1.0.0`**，作为「v1 CLI 产品目标完成」的包版本表达。

## 2. 三位含义（发版后强制）

| 位 | 何时递增 | 本仓库具体例子 |
|---|---|---|
| **MAJOR** | 破坏用户已依赖的 CLI / 协议 / 默认行为 | 删改子命令或稳定 flag；`schema_version` 破坏性升级；默认从废纸篓改为硬删等 |
| **MINOR** | 向后兼容的功能或能力扩展 | 新子命令（非破坏）；规则批次扩量；新 guard / handler；Homebrew/安装体验增强且兼容旧用法 |
| **PATCH** | 向后兼容的缺陷修复 | 误删修复、崩溃、文档/Formula sha 勘误、签名流水线 hotfix（行为契约不变） |

规则条数增加 → 默认 **MINOR**（能力扩展），不是 PATCH。  
仅修某条规则误报/误删且无新能力 → **PATCH**。

## 3. 与「v1」话术的关系

- **包版本**是唯一对外版本语言：`vole --version`、GitHub Release tag、`Formula`、`docs/releases/`、README 安装示例必须一致。
- 文档可写「1.0 起 v1 CLI 范围冻结」，**不再**并列「产品 v1 / 包 0.0.x」。
- 协议字段 `schema_version` **独立于**包版本；破坏性协议变更同时要求：bump `schema_version` **且** 视影响 bump 包 **MAJOR**（若仅桌面消费者、CLI 用户无感，仍至少 MINOR，并在 release notes 写明）。

## 4. 历史线与下一刀

| 区间 | 处理 |
|---|---|
| `0.0.1`–`0.0.11` | 视为前 SemVer 纪律阶段；Release notes 保留；**禁止** force 改 tag / 删 Release |
| **下一发版** | **`1.0.0`**：以当前 main（≥509 规则、签名+公证、v1 closeout 范围）为内容；主要为版本对齐，可无新功能 |
| `1.x` 之后 | 严格按 §2；v1.x backlog（Toolbox / Codex staging / plan 去重等）进 **MINOR** |

不采用「继续 `0.y.z` 再择机 1.0.0」：与已宣告的 closeout 重复叙事、延长双轨命名。

## 5. 单一事实来源

1. **Canonical**：`Cargo.toml` → `[workspace.package].version`
2. 发版前必须把该字段改到目标版本；`scripts/package-release.sh` / `scripts/update-homebrew-formula.sh` 以它（或显式参数）为准
3. Git tag 形如 `v${version}`（例：`v1.0.0`）
4. `docs/releases/v${version}.md` 与 GitHub Release body 同源
5. README / Formula 硬编码版本与上述同步（沿用现有 update 脚本）

目标：消灭「tag 已是 0.0.11、workspace 仍是 0.0.1」类漂移。

## 6. 非目标

- 不重写 `0.0.x` 历史 tag
- 不在本策略里实现桌面 app 独立版本线（另仓另议；若同仓再单独立项）
- 不把 Mole 上游版本号映射进 Vole SemVer

## 7. 验收（实施计划完成时）

- [x] 本策略文档合入 main
- [x] README 或 `docs/` 有一处简短「版本策略」指针（链到本 spec 或 findings 摘要）
- [x] 发版清单含：bump workspace version → tag → assets → Formula → README
- [x] 下一公开发版为 **`v1.0.0`**（或实施时若已有更新，不得再发 `0.0.12`）
