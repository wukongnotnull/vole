# Phase 5：Developer ID 签名 / 公证 / Homebrew

**更新**：2026-07-30  
**Release**：v0.0.7（470 规则，ad-hoc）

## 当前状态

| 项 | 状态 |
|---|---|
| Apple Developer Program | **未购买** — 无 Developer ID |
| GitHub Release | **v0.0.1 – v0.0.7** tarball 已发布（ad-hoc） |
| `scripts/package-release.sh` | 构建 + 打包；若设 `VOLE_CODESIGN_IDENTITY` 则签名/公证 |
| `scripts/sign-and-notarize.sh` | 无证书时 **exit 3** + 明确提示 |
| `scripts/update-homebrew-formula.sh` | 刷新 `Formula/vole.rb` url/sha256 |
| Homebrew stable | **v0.0.7** — `brew tap wukongnotnull/vole` + `brew install vole` |
| Homebrew HEAD | 源码 `cargo install` |
| CI `release.yml` | 可选 secrets 导入 p12 + 签名/公证 |

## Homebrew 安装（stable v0.0.7）

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
export VOLE_RULES_DIR="$(brew --prefix vole)/share/vole/rules"
```

发新版后维护者运行：

```bash
bash scripts/package-release.sh 0.0.8
bash scripts/update-homebrew-formula.sh 0.0.8
git add Formula/vole.rb && git commit -m "chore(homebrew): pin v0.0.8"
```

## 有 Developer ID 时

### 本地

```bash
export VOLE_CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export VOLE_NOTARY_PROFILE="vole-notary"   # notarytool keychain profile
bash scripts/package-release.sh 0.0.8
```

### GitHub Actions secrets

| Secret | 用途 |
|---|---|
| `APPLE_CERTIFICATE_BASE64` | Developer ID p12（base64） |
| `APPLE_CERTIFICATE_PASSWORD` | p12 密码 |
| `VOLE_CODESIGN_IDENTITY` | 完整 identity 字符串 |
| `VOLE_NOTARY_PROFILE` | `notarytool store-credentials` 配置名 |

推送 tag `v*` 后 CI 自动签名 + 公证（secrets 就绪时）。

### 首次 notary 配置

```bash
xcrun notarytool store-credentials "vole-notary" \
  --apple-id "you@example.com" \
  --team-id "TEAMID" \
  --password "@keychain:AC_PASSWORD"
```

## 无证书验收（当前）

- Release tarball ad-hoc；用户 `xattr -cr` 或系统设置允许
- Formula 走 **Formula**（非 Cask），无强制公证要求（见 spike-platform.md）
- `sign-and-notarize.sh` 缺 identity 时非零退出，不假装成功

## 后续

1. 购买 Apple Developer Program
2. 配置 CI secrets → 下一 tag 自动签名/公证
3. 可选：独立 tap `wukongnotnull/homebrew-vole` 或 PR 到 homebrew-core

## 相关

- `docs/findings/2026-07-phase1-tcc-deferred.md`
- `docs/findings/2026-07-spike-platform.md` § Homebrew
