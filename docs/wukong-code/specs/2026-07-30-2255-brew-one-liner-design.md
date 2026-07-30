# 一行安装：`brew install vole`

- 日期：2026-07-30
- 状态：**本仓已交付**；Homebrew Core PR [#296168](https://github.com/Homebrew/homebrew-core/pull/296168) **已因 notability 关闭**；短期安装 = 自建 tap 两行
- 依据：当前 `Formula/vole.rb` / `default_rules_dir`；`docs/findings/2026-07-brew-one-liner.md`；`docs/findings/2026-07-phase5-signing.md`

## 1. 结论

**长期目标**仍是裸 `brew install vole`（进 Homebrew Core）。  
**短期现实**：Core 因知名度门禁不可用；用户路径为自建 tap 两行，且**无需**手动 `export VOLE_RULES_DIR`。

已落地：

1. **零配置规则发现**：去掉 Homebrew 安装路径上对 `VOLE_RULES_DIR` 的硬性依赖与文档噪音。
2. **Core 尝试已关闭**：提交过源码构建 formula；维护者要求自建 tap，达标后再提。自建 tap 继续发 **Developer ID 预编译**（TCC 稳定身份）。

## 2. 已锁定决策

| 项 | 结论 |
|---|---|
| 短期成功标准 | `brew tap … && brew install vole` 装完即可运行；`clean --plan` 能发现 rules（无 env） |
| 长期成功标准 | Core 合并后 `brew install vole` 一行 |
| Core formula 形态 | **源码构建**（`cargo install` + `pkgshare` rules）；**不**把预编译 tarball 送进 core；`test` 用 `vole clean --plan` |
| 自建 tap | 继续维护 `Formula/vole.rb` 预编译 + Developer ID / 公证产物 |
| `VOLE_RULES_DIR` | 仍支持覆盖；**默认路径不再要求用户 export** |
| README 主路径 | **两行 tap + install**（无强制 env）；Core 合并后再改一行 |
| 版本 | 体验增强、向后兼容 → SemVer **MINOR** 候选；仅文档可为 docs-only |

## 3. 非目标

- 不强求本迭代内 Homebrew Core PR **一定被合并**（上游审核不可控）；本仓交付「可提交的 formula + 本地验证 + PR 尝试」；未达标则关闭并保留自建 tap
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

1. ~~本机：Cellar 布局下未设 env 可加载 rules~~ ✅  
2. ~~caveats 无强制 export~~ ✅  
3. ~~README / phase5 findings 与短期两行路径一致~~ ✅（并同步 releases / Release 页）  
4. ~~core 候选 + audit~~ ✅（`docs/homebrew/vole-homebrew-core.rb`）  
5. ~~开 Core PR~~ ✅ 后因 notability **关闭**；短期不阻塞本仓交付  

## 6. 下一步

- 短期：文档与 Release 保持自建 tap 两行；关注仓库 stars/forks/watchers  
- 达标后：带强化 `test` 的 Core 候选再提 PR；合并后 README 改一行主路径  
- 计划原文：`docs/wukong-code/plans/2026-07-30-2255-brew-one-liner.md`
