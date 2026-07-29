# Phase 5：Developer ID 签名 / 公证 / Homebrew（占位）

日期：2026-07-29

## 当前状态

| 项 | 状态 |
|---|---|
| Apple Developer Program（$99/yr） | **未购买** — 无 Developer ID Application 证书 |
| `codesign` Developer ID | 不可用；本机仅能 ad-hoc（见 Phase 1 TCC findings） |
| Notarization（`notarytool`） | 依赖 Developer ID + App Store Connect API key |
| Homebrew formula | 草稿见 `HomebrewFormula/vole.rb`（url/sha256 待首个 release） |
| `install.sh` | 仓库根目录最小说明脚本（本地 `cargo install` / 二进制路径） |

## 有证书后的步骤（摘要）

1. 在钥匙串安装 **Developer ID Application** 证书。
2. 构建 release：`cargo build -p vole-cli --release`。
3. 运行 `scripts/sign-and-notarize.sh`（需环境变量，见脚本头注释）。
4. 上传 notarize；钉住 `stapler`。
5. 发布 GitHub Release（附 universal 或 arm64/x86_64 产物）。
6. 填入 `HomebrewFormula/vole.rb` 的 `url` / `sha256`，提交 tap 或 PR 到 homebrew-core（另议）。

## 无证书时的验收口径（本 Task）

- 本文档存在且写明缺口。
- `scripts/sign-and-notarize.sh` 在缺证书时 **明确 skip/失败原因**，不假装成功。
- Homebrew / install 草稿不阻塞 history + protocol 合并。

## 与 Phase 1 TCC 的关系

完整 TCC 矩阵仍 deferred，见 `docs/findings/2026-07-phase1-tcc-deferred.md`。签名身份就绪后一并补测。
