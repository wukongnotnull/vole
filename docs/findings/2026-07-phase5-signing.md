# Phase 5：Developer ID 签名 / 公证 / Homebrew

**更新**：2026-07-30  
**Release**：v0.0.7（470 规则）  
**Developer ID**：`Developer ID Application: Kong Wu (WCYC8XY4V2)`

## 当前状态

| 项 | 状态 |
|---|---|
| Apple Developer Program | **Kong Wu / WCYC8XY4V2** |
| 本机 Keychain | 需在 Mac 上安装 `.cer` + 私钥（`scripts/check-signing.sh` 验证） |
| GitHub Release | v0.0.1 – v0.0.7（ad-hoc，待重签 v0.0.8+） |
| 本地配置 | `cp scripts/signing.env.example scripts/signing.env` |
| CI secrets | 见下表（`VOLE_CODESIGN_IDENTITY` 用完整 identity 字符串） |

## 快速开始（本机 Mac）

```bash
# 1. 从 Apple Developer 下载 Developer ID Application 证书，双击安装到「登录」钥匙串
# 2. 配置环境（不提交 git）
cp scripts/signing.env.example scripts/signing.env

# 3. 验证钥匙串
bash scripts/check-signing.sh

# 4. 签名 release（可选公证见下）
bash scripts/package-release.sh 0.0.8
```

### 公证（可选）

```bash
# App 专用密码存入钥匙串后：
xcrun notarytool store-credentials "vole-notary" \
  --apple-id "YOUR_APPLE_ID" \
  --team-id "WCYC8XY4V2" \
  --password "@keychain:AC_PASSWORD"

echo 'export VOLE_NOTARY_PROFILE="vole-notary"' >> scripts/signing.env
bash scripts/package-release.sh 0.0.8
```

## GitHub Actions secrets

| Secret | 值 |
|---|---|
| `VOLE_CODESIGN_IDENTITY` | `Developer ID Application: Kong Wu (WCYC8XY4V2)` |
| `APPLE_CERTIFICATE_BASE64` | `base64 -i Certificates.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | 导出 p12 时的密码 |
| `VOLE_NOTARY_PROFILE` | `vole-notary`（CI 需改用 API key 方式，见 Apple 文档） |

导出 p12（在已安装证书的 Mac 上）：

1. 钥匙串访问 → 我的证书 → 展开 `Developer ID Application: Kong Wu`
2. 选中证书+私钥 → 导出 → PKCS #12 (.p12)

推送 tag `v0.0.8` 后 CI 在 secrets 就绪时自动签名。

## Homebrew

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
export VOLE_RULES_DIR="$(brew --prefix vole)/share/vole/rules"
```

发新版：

```bash
bash scripts/package-release.sh 0.0.8
bash scripts/update-homebrew-formula.sh 0.0.8
git add Formula/vole.rb && git commit -m "chore(homebrew): pin v0.0.8"
```

## 脚本

| 脚本 | 用途 |
|---|---|
| `scripts/check-signing.sh` | 验证 Keychain 中有 Developer ID |
| `scripts/sign-and-notarize.sh` | 单二进制 codesign + 可选 notarytool |
| `scripts/package-release.sh` | 构建 tarball；读取 `signing.env` |
| `scripts/update-homebrew-formula.sh` | 刷新 Formula sha256 |

## 相关

- `docs/findings/2026-07-phase1-tcc-deferred.md`
- `docs/findings/2026-07-spike-platform.md` § Homebrew
