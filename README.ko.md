<div align="center">

<img src="images/vole.webp" alt="Vole mascot" width="180" />

# Vole

**macOS용 정리·모니터링 CLI**  
먼저 확인 · 기본은 휴지통 · 설치하면 바로 사용

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/github/v/tag/wukongnotnull/vole?label=version)](https://github.com/wukongnotnull/vole/releases)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-black.svg)](https://github.com/wukongnotnull/vole)
[![Download](https://img.shields.io/github/downloads/wukongnotnull/vole/total.svg)](https://github.com/wukongnotnull/vole/releases/latest)
[![Stars](https://img.shields.io/github/stars/wukongnotnull/vole?style=social)](https://github.com/wukongnotnull/vole/stargazers)

</div>

**언어:** [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

> 캐시, 로그, 잔여물, 설치 파일, 빌드 찌꺼기… Vole이 찾아 주고 **미리 본 뒤 정리**합니다. 기본은 휴지통이라 잘못 지워도 복구할 수 있습니다.

---

**바로가기**
[화면 미리보기](#화면-미리보기) · [할 수 있는 일](#할-수-있는-일) · [설치](#설치) · [사용법](#사용법) · [안전](#안전) · [FAQ](#faq) · [데스크톱](#gui를-선호한다면) · [소개](#소개) · [감사의-말](#감사의-말) · [라이선스](#라이선스)

---

## 화면 미리보기

<p align="center">
  <img src="images/tui/home.png" alt="Vole 대화형 홈" width="720" />
</p>

터미널에서 `vole`을 실행하면 대화형 홈이 열립니다. 방향키로 이동, Enter로 선택.

---

## 할 수 있는 일

| 기능 | 얻는 것 |
|------|-------------|
| **정리** | 캐시·로그·잔여물을 스캔하고 확인 후 정리 |
| **제거** | 앱을 지우고 잔여 파일도 최대한 제거 |
| **최적화** | 안전한 범위의 시스템 유지보수(캐시 새로고침 등) |
| **정화** | 오래된 프로젝트 빌드 산출물 등 큰 용량 항목 정리 |
| **설치 파일** | 디스크에 남은 `.dmg` / `.pkg` 찾기 |
| **분석** | 어떤 폴더·큰 파일이 공간을 쓰는지 확인 |
| **기록** | 과거 정리·삭제 내역 확인 |
| **상태** | CPU·메모리·디스크 건강 상태를 실시간으로 |

터미널에 `vole`을 입력하면 대화형 홈이 열리고 방향키로 고를 수 있습니다. 약 **540**개의 정리 규칙이 내장되어 **추가 도구가 필요 없습니다**.

터미널은 쓸 수 있지만 「한 번에 전부 삭제」는 싫은 분께 맞습니다. 창 UI가 필요하면 아래 데스크톱을 보세요.

---

## 설치

**macOS 12 이상** 필요.

현재 공개 버전: **[v2.16.0](https://github.com/wukongnotnull/vole/releases/tag/v2.16.0)** (Developer ID 서명 및 Apple 공증). Apple Silicon·Intel 모두 제공.

### 방법 1: 다운로드 (권장)

1. [최신 Release](https://github.com/wukongnotnull/vole/releases/latest) 열기
2. 칩에 맞는 압축 파일 받기:
   - Apple Silicon (M 시리즈): `…-aarch64-apple-darwin.tar.gz`
   - Intel: `…-x86_64-apple-darwin.tar.gz`
3. `bin/vole`을 PATH에 두고(예: `~/.local/bin`), 함께 들어 있는 `share/vole/rules`도 유지

예 (Apple Silicon / v2.16.0; 파일명은 Release 페이지 기준):

```bash
curl -LO https://github.com/wukongnotnull/vole/releases/download/v2.16.0/vole-2.16.0-aarch64-apple-darwin.tar.gz
tar xzf vole-2.16.0-aarch64-apple-darwin.tar.gz
mkdir -p ~/.local/bin ~/.local/share/vole
install -m 755 vole-2.16.0-aarch64-apple-darwin/bin/vole ~/.local/bin/vole
cp -R vole-2.16.0-aarch64-apple-darwin/share/vole/rules ~/.local/share/vole/
```

`vole: command not found`가 나오면 `~/.zshrc`에 다음을 넣고 `source ~/.zshrc`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

### 방법 2: Homebrew

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
```

설치 후 `vole` 실행. brew가 실패하거나 버전이 맞지 않으면 위 다운로드를 사용하세요.

---

## 사용법

### 자주 쓰는 명령

```bash
# 일상
vole                           # 대화형 홈 (가장 쉬움)
vole status                    # 머신 상태
vole analyze                   # 디스크를 누가 쓰는지
vole clean                     # 스캔 → 확인 → 정리 (기본 휴지통)
vole uninstall                 # 대화형 앱 제거
vole optimize                  # 시스템 유지보수
vole history                   # 과거 작업 확인

# 미리보기만 — 아직 삭제하지 않음
vole clean --plan
vole uninstall --plan
vole optimize --plan
vole purge --plan
vole installer --plan

# 후보를 본 뒤 적용
vole clean --apply <plan.json>
vole uninstall --apply <plan.json>

# 기타 자주 씀
vole touchid status            # sudo Touch ID 상태
vole update                    # 새 버전으로 업그레이드
vole remove --dry-run          # Vole 자체 제거 미리보기
vole --help
vole --version
```

기본은 휴지통. 완전 삭제는 직접 선택할 때만입니다.

---

### 전체 명령

서브커맨드 없이 `vole`을 실행하면 대화형 홈이 열립니다.

| 명령 | 별칭 | 설명 |
|------|------|------|
| `vole` | — | 대화형 홈 (Clean / Uninstall / Optimize / Analyze / Status) |
| `vole clean` | — | 캐시·잔여물 정리 |
| `vole uninstall` | — | 앱 및 잔여물 제거 |
| `vole optimize` | `optimise` | 시스템 최적화·유지보수 |
| `vole status` | — | 실시간 건강 패널 (CPU / 메모리 / 디스크) |
| `vole analyze` | `analyse` | 디렉터리 용량 분석 (홈 폴더부터) |
| `vole history` | — | 작업 기록 및 삭제 로그 |
| `vole purge` | — | 오래된 프로젝트 빌드 산출물 정리 |
| `vole installer` | — | 설치 파일 찾아 정리 |
| `vole touchid` | — | sudo Touch ID 설정 (`status` / `enable` / `disable`) |
| `vole update` | — | 자동 업데이트 (실행할 때만 네트워크) |
| `vole remove` | — | Vole 자체 제거 |
| `vole completions` | `completion` | 셸 자동완성 생성 |
| `vole help` | — | 도움말 (`-h` / `--help`도 가능) |
| `vole --version` | `-V` | 버전 출력 |

## 안전

```
당신      ❯ vole clean → 후보 확인 → 승인

Vole      ❯ ✓ 후보를 먼저 보여 주고 바로 지우지 않음
            ✓ 기본은 휴지통(복구 가능)
            ✓ 적용 전 보호 경로 재검사
            ✓ 불확실하면 건너뜀 — 삭제 범위를 조용히 넓히지 않음
```

| 원칙 | 의미 |
|------|------|
| **미리 보고 실행** | 터미널이 확인을 받음. `--plan`이면 목록만 봄 |
| **기본은 복구 가능** | 개인 파일은 휴지통으로 |
| **명확한 보고** | 휴지통 vs 완전 삭제 구분 |
| **추적 가능** | `vole history`로 확인 |

일상 정리는 로컬에서만 이뤄집니다. `vole update`를 실행할 때만 네트워크 업데이트가 됩니다.

---

## FAQ

**Q: 묻지 않고 지우나요?**  
A: `clean` / `optimize`는 확인을 받습니다(기본 No). 불안하면 먼저 `--plan`으로 목록만 보세요.

**Q: 잘못 지웠어요.**  
A: 기본은 휴지통입니다. 휴지통에서 복구하세요. 완전 삭제를 선택했다면 되돌릴 수 없습니다.

**Q: 앱이 이미 없나요, 아직 있나요?**  
A: 제거 후 잔여물 → `vole clean`. 앱이 아직 있음 → `vole uninstall`.

**Q: `vole: command not found`?**  
A: 설치 경로가 PATH에 있는지 확인한 뒤(위 `~/.local/bin`), 터미널을 새로 여세요.

**Q: GUI가 필요해요.**  
A: [Vole for macOS](https://github.com/wukongnotnull/vole-macos)를 쓰세요 — 같은 정리 능력의 창 앱(Apple Silicon·Intel).

**Q: Mole과 어떤 관계인가요?**  
A: 규칙과 안전 아이디어는 [Mole](https://github.com/tw93/Mole)에서 영감을 받았습니다. Vole은 독립 오픈소스이며 Mole에 소속되지 않습니다.

---

## GUI를 선호한다면?

[Vole for macOS](https://github.com/wukongnotnull/vole-macos)는 동반 데스크톱입니다: 사이드바 Clean / Uninstall / Optimize / Purge / Installer / Analyze / History / Status, 전체 디스크 접근, 일부 시스템 경로용 선택적 Root 권한 도우미.

최신 데스크톱: [vole-macos Releases](https://github.com/wukongnotnull/vole-macos/releases/latest) (현재 **v0.2.0** Universal DMG).

```text
같은 정리 능력 · 같은 안전 습관 · 터미널 또는 창
```

---

## 소개

**悟空非空也 (Wukong)** — AI之道 창립자, 인디 개발자, 크리에이터.

| 플랫폼 | 링크 |
|------|------|
| 🌐 웹사이트 | [waytoai.cn](https://waytoai.cn) |
| 𝕏 Twitter | [悟空非空也](https://x.com/wukongnotnull) |
| 📺 Bilibili | [悟空非空也](https://space.bilibili.com/456634391) |
| ▶️ YouTube | [悟空非空也](https://www.youtube.com/@wukongnotnull) |
| 📕 샤오홍슈 | [悟空非空也](https://www.xiaohongshu.com/user/profile/5ca89c2f000000001100952b) |
| 💬 WeChat | 「悟空非空也」검색 |

---

## 감사의 말

macOS 정리 UX를 개척한 제품·오픈소스에 감사드립니다. Vole이 많은 것을 배웠습니다:

- [Mole](https://github.com/tw93/Mole) — 오픈소스 클리너. 규칙·안전의 주요 영감
- [CleanMyMac](https://macpaw.com/cleanmymac) — 세련된 데스크톱 정리 UX 참고
- [Tencent Lemon](https://lemon.qq.com/) — 중국어권에 익숙한 시스템 클리너 참고

Vole은 독립 오픈소스이며 위 제품과 소속·상업 관계가 없습니다.

---

## 라이선스

Vole은 [GPL-3.0](LICENSE)입니다.  
자체 제품으로 파생할 경우 혼동을 피하도록 이름을 바꾸고, Mole / Vole을 출처로 밝혀 주세요.

---

<div align="center">

GPL-3.0 license © [悟空非空也](https://github.com/wukongnotnull)

</div>
