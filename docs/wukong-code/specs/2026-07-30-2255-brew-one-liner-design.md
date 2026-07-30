# 一行安装：`brew install vole`

- 日期：2026-07-30
- 状态：已批准（用户「要推进」；Condensed brainstorming）
- 依据：当前 `Formula/vole.rb` / `default_rules_dir`；`docs/findings/2026-07-phase5-signing.md`；mole 的 homebrew-core 路径

## 1. 结论

目标用户体验：**在已装 Homebrew 的 macOS 上，只需 `brew install vole` 即可装好并直接运行**（无需 tap URL、无需手动 `export VOLE_RULES_DIR`）。

实现拆成两轨：

1. **零配置规则发现（本仓立刻可做）**：去掉 Homebrew 安装路径上对 `VOLE_RULES_DIR` 的硬性依赖与文档噪音。
2. **进入 Homebrew Core（真正一行 `brew install vole`）**：提交源码构建 formula；自建 tap 保留 **Developer ID 预编译** 作为 TCC 稳定身份备选。

## 2. 已锁定决策

| 项 | 结论 |
|---|---|
| 成功标准 | `brew install vole`（core 合并后）装完即可 `vole --help` / `vole clean --plan` 找到 rules |
| Core formula 形态 | **源码构建**（`cargo install` + 安装 `data/rules`），对齐 mole / Homebrew 惯例；**不**把预编译 tarball 送进 core |
| 自建 tap | 继续维护 `Formula/vole.rb` 预编译 + Developer ID / 公证产物，供「稳定 TCC 身份」用户 |
| `VOLE_RULES_DIR` | 仍支持覆盖；**默认路径不再要求用户 export**（相对 `bin/vole` 的 `../share/vole/rules` 已足够） |
| README 主路径 | Core 合并前：两行（tap + install）且无 env；合并后：一行 `brew install vole` |
| 版本 | 体验增强、向后兼容 → SemVer **MINOR** 候选；若仅文档/caveats 可 docs-only 或随下一发版 |

## 3. 非目标

- 不强求本迭代内 Homebrew Core PR **一定被合并**（上游审核不可控）；本仓交付「可提交的 formula + 本地验证 + PR 已开」
- 不新建 `homebrew-vole` 独立 tap 仓库（不能换来裸 `brew install vole`，性价比低于冲 core）
- 不把预编译二进制强行塞进 core（会被拒）
- 不改 TCC / 签名策略本身（core bottle ≠ Developer ID）

## 4. 风险

| 风险 | 应对 |
|---|---|
| Core 以「与 mole 重复」拒收 | desc/homepage 强调 Rust 重写、协议兼容与独立发版；准备差异说明 |
| Core bottle 与 Developer ID 身份分裂 | README 写明两轨；需要稳定 FDA 时用自建 tap 预编译 |
| 相对路径发现失败（罕见布局） | 保留 `VOLE_RULES_DIR`；测试覆盖 Cellar 布局 |

## 5. 验收

1. 本机：按 Cellar 布局安装后，**未设置** `VOLE_RULES_DIR` 时 `vole clean --plan`（或等价）能加载 `share/vole/rules`
2. `Formula/vole.rb` caveats **不再**要求 export rules dir（Gatekeeper 提示可保留）
3. README / phase5 findings 安装说明与上一致
4. 存在可审计的 homebrew-core 候选 formula（源码构建）+ `brew audit` 本地结论记录
5. 向 `Homebrew/homebrew-core` 开出（或准备好）new formula PR；合并后 README 改为一行主路径

## 6. 下一步

实施计划：`docs/wukong-code/plans/2026-07-30-2255-brew-one-liner.md`
