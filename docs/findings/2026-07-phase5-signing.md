# Phase 5：Developer ID 签名 / 公证 / Homebrew（占位）

日期：2026-07-29

## 当前状态

| 项 | 状态 |
|---|---|
| Apple Developer Program（$99/yr） | **未购买** — 无 Developer ID Application 证书 |
| `codesign` Developer ID | 不可用；本机仅能 ad-hoc（见 Phase 1 TCC findings） |
| Notarization（`notarytool`） | 依赖 Developer ID + App Store Connect API key |
| Homebrew formula | 草稿见 `HomebrewFormula/vole.rb`（`brew install --HEAD`）；stable url/sha256 待首个 release tag |
| `install.sh` | 安装二进制 + `share/vole/rules`；见 README |
| GitHub Release | **v0.0.1** — `scripts/package-release.sh` 或 push tag `v0.0.1`（CI `release.yml`） |
| Release notes | `docs/releases/v0.0.1.md` |

## 有证书后的步骤（摘要）

1. 在钥匙串安装 **Developer ID Application** 证书。
2. 构建 release：`cargo build -p vole-cli --release`。
3. 运行 `scripts/sign-and-notarize.sh`（需环境变量，见脚本头注释）。
4. 上传 notarize；钉住 `stapler`。
5. 发布 GitHub Release（附 universal 或 arm64/x86_64 产物）。
6. 填入 `HomebrewFormula/vole.rb` 的 `url` / `sha256`，提交 tap 或 PR 到 homebrew-core（另议）。

## 无证书时的发布步骤（v0.0.1）

1. 合并 main（含 dual-run + release 脚本）。
2. macOS 上本地打包（可选）：`bash scripts/package-release.sh 0.0.1` → `dist/*.tar.gz`。
3. 打 tag 并推送（推荐，由 CI 上传 assets）：
   ```bash
   git tag -a v0.0.1 -m "Phase 4c v1: 150 clean rules"
   git push origin v0.0.1
   ```
4. 在 GitHub Release 页确认 `vole-*-aarch64-apple-darwin.tar.gz` 与 `x86_64` 附件。
5. 用户安装见 `README.md`；Gatekeeper 提示用 `xattr -cr` 或系统设置允许。

## 无证书时的验收口径（本 Task）

- 本文档存在且写明缺口。
- `scripts/sign-and-notarize.sh` 在缺证书时 **明确 skip/失败原因**，不假装成功。
- Homebrew / install 草稿不阻塞 history + protocol 合并。

## 与 Phase 1 TCC 的关系

完整 TCC 矩阵仍 deferred，见 `docs/findings/2026-07-phase1-tcc-deferred.md`。签名身份就绪后一并补测。
