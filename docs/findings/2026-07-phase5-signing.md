# Phase 5：Developer ID 签名 / 公证 / Homebrew

**更新**：2026-07-30  
**Release**：v0.0.8（470 规则，Developer ID 签名）  
**Developer ID**：`Developer ID Application: Kong Wu (WCYC8XY4V2)`  
**Team ID**：`WCYC8XY4V2`

## 当前状态

| 项 | 状态 |
|---|---|
| 本机签名 | `bash scripts/check-signing.sh` → OK |
| 本机公证 | 待运行 `bash scripts/setup-notary-profile.sh` |
| GitHub CI secrets | 待运行 `bash scripts/setup-ci-secrets.sh` |
| Release | v0.0.8 已签名；v0.0.9+ 可启用公证 |

---

## 一、本机公证（Terminal.app）

### 方式 A：API Key（推荐，与 CI 共用）

1. 打开 [App Store Connect → 用户和访问 → 集成 → App Store Connect API](https://appstoreconnect.apple.com/access/integrations/api)
2. 创建 Key，角色选 **Developer** 或 **Admin**，下载 `AuthKey_XXXXXX.p8`（仅一次）
3. 记录 **Key ID**（10 位）和 **Issuer ID**（页面顶部 UUID）

```bash
cd ~/Documents/vole
bash scripts/setup-notary-profile.sh --api-key ~/Downloads/AuthKey_XXXXXX.p8
bash scripts/check-signing.sh   # 应显示 OK: notary profile 'vole-notary'
bash scripts/package-release.sh 0.0.9
```

### 方式 B：Apple ID + 专用密码

1. [appleid.apple.com](https://appleid.apple.com) → 登录 → 安全 → **App 专用密码**，生成一条
2. 存入钥匙串（一次性）：

```bash
security add-generic-password -a "YOUR_APPLE_ID@email.com" -s "AC_PASSWORD" -w
# 粘贴专用密码后回车
```

3. 创建 profile：

```bash
bash scripts/setup-notary-profile.sh
# 按提示输入 Apple ID；Keychain 项名默认 AC_PASSWORD
```

---

## 二、GitHub Actions Secrets

在 **终端.app** 运行（需已 `gh auth login`）：

```bash
cd ~/Documents/vole
bash scripts/setup-ci-secrets.sh
```

脚本会交互式写入以下 secrets（**不会**提交到 git）：

| Secret | 说明 |
|---|---|
| `VOLE_CODESIGN_IDENTITY` | `Developer ID Application: Kong Wu (WCYC8XY4V2)` |
| `APPLE_CERTIFICATE_BASE64` | 从 Keychain 导出的 `.p12` 做 base64 |
| `APPLE_CERTIFICATE_PASSWORD` | 导出 p12 时设置的密码 |
| `APPLE_API_KEY_BASE64` | （可选）`.p8` 文件 base64 |
| `APPLE_API_KEY_ID` | （可选）10 位 Key ID |
| `APPLE_API_ISSUER_ID` | （可选）Issuer UUID |

### 手动导出 p12

1. 钥匙串访问 → 我的证书 → 展开 `Developer ID Application: Kong Wu`
2. 选中 **证书 + 私钥** → 文件 → 导出 → PKCS #12 (.p12)
3. 设置导出密码（即 `APPLE_CERTIFICATE_PASSWORD`）

### 验证 CI

推送 tag 后 Actions 自动跑 release workflow：

```bash
git tag v0.0.9
git push origin v0.0.9
```

在 GitHub → Actions → Release 查看是否 codesign + notarize 成功。

---

## 三、本地仅签名（已完成）

```bash
cp scripts/signing.env.example scripts/signing.env
bash scripts/check-signing.sh
bash scripts/package-release.sh 0.0.8
```

`VOLE_NOTARY_PROFILE` 未设置时为 **sign-only**，与 v0.0.8 一致。

---

## 脚本索引

| 脚本 | 用途 |
|---|---|
| `scripts/check-signing.sh` | 验证 Developer ID + 公证 profile |
| `scripts/setup-notary-profile.sh` | 本机 notarytool 钥匙串 profile |
| `scripts/setup-ci-secrets.sh` | 一键写入 GitHub secrets |
| `scripts/sign-and-notarize.sh` | codesign + notary（profile 或 API key） |
| `scripts/package-release.sh` | 构建 tarball |

---

## Homebrew

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
export VOLE_RULES_DIR="$(brew --prefix vole)/share/vole/rules"
```

---

## 相关

- `docs/releases/v0.0.8.md`
- `docs/findings/2026-07-phase1-tcc-deferred.md`
