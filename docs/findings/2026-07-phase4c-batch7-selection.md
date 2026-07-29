# Phase 4c Batch 7 selection

Target: **40** rules (190 → **230**)  
**Actual: 40** (app-caches +20, user-devtools +20).

## Block A — `data/rules/app-caches.toml` (+20)

| proposed_id | label | path |
|---|---|---|
| klee-desktop-cache | Klee desktop cache | `~/Library/Caches/klee_desktop/*` |
| podcasts-streamed-media | Podcasts streamed media | `~/Library/Containers/com.apple.podcasts/Data/tmp/StreamedMedia` |
| podcasts-artwork-cache | Podcasts artwork cache | `~/Library/Containers/com.apple.podcasts/Data/tmp/*.heic` |
| podcasts-image-cache | Podcasts image cache | `~/Library/Containers/com.apple.podcasts/Data/tmp/*.img` |
| podcasts-download-temp | Podcasts download temp | `~/Library/Containers/com.apple.podcasts/Data/tmp/*CFNetworkDownload*.tmp` |
| qbittorrent-cache | qBittorrent cache | `~/Library/Caches/com.qbittorrent.qBittorrent` |
| downie-cache | Downie cache | `~/Library/Caches/com.downie.Downie-*` |
| folx-cache | Folx cache | `~/Library/Caches/com.folx.*/*` |
| steam-cache | Steam cache | `~/Library/Caches/com.valvesoftware.steam/*` |
| epic-games-cache | Epic Games cache | `~/Library/Caches/com.epicgames.EpicGamesLauncher/*` |
| battle-net-cache | Battle.net cache | `~/Library/Caches/com.blizzard.Battle.net/*` |
| ea-origin-cache | EA Origin cache | `~/Library/Caches/com.ea.*/*` |
| gog-galaxy-cache | GOG Galaxy cache | `~/Library/Caches/com.gog.galaxy/*` |
| riot-games-cache | Riot Games cache | `~/Library/Caches/com.riotgames.*/*` |
| cleanshot-cache | CleanShot cache | `~/Library/Caches/com.cleanshot.*` |
| camo-cache | Camo cache | `~/Library/Caches/com.reincubate.camo` |
| xnip-cache | Xnip cache | `~/Library/Caches/com.xnipapp.xnip` |
| youdao-dictionary-cache | Youdao Dictionary cache | `~/Library/Caches/com.youdao.YoudaoDict` |
| eudict-cache | Eudict cache | `~/Library/Caches/com.eudic.*` |
| bob-translation-cache | Bob Translation cache | `~/Library/Caches/com.bob-build.Bob` |

## Block B — `data/rules/user-devtools.toml` (+20)

| proposed_id | label | path |
|---|---|---|
| antigravity-webgpu-cache | Antigravity WebGPU cache | `~/Library/Application Support/Antigravity/DawnWebGPUCache/*` |
| prisma-cache | Prisma cache | `~/.cache/prisma/*` |
| opencode-cache | OpenCode cache | `~/.cache/opencode/*` |
| playwright-browsers | Playwright browsers | `~/Library/Caches/ms-playwright/*` |
| filo-code-cache | Filo code cache | `~/Library/Application Support/Filo/production/Code Cache/*` |
| insomnia-cache | Insomnia cache | `~/Library/Caches/com.konghq.insomnia/*` |
| unity-cache | Unity cache | `~/Library/Caches/com.unity3d.*/*` |
| figma-cache | Figma cache | `~/Library/Caches/com.figma.Desktop/*` |
| github-desktop-cache | GitHub Desktop cache | `~/Library/Caches/com.github.GitHubDesktop/*` |
| antigravity-cache | Antigravity cache | `~/Library/Application Support/Antigravity/Cache/*` |
| antigravity-code-cache | Antigravity code cache | `~/Library/Application Support/Antigravity/Code Cache/*` |
| antigravity-gpu-cache | Antigravity GPU cache | `~/Library/Application Support/Antigravity/GPUCache/*` |
| antigravity-dawn-cache | Antigravity Dawn cache | `~/Library/Application Support/Antigravity/DawnGraphiteCache/*` |
| gradle-notifications-cache | Gradle notifications cache | `~/.gradle/notifications/*` |
| gradle-daemon | Gradle daemon | `~/.gradle/daemon/*` |
| gradle-workers | Gradle workers | `~/.gradle/workers/*` |
| simulator-runtime-cache | Simulator runtime cache | `~/Library/Developer/CoreSimulator/Profiles/Runtimes/*/Contents/Resources/RuntimeRoot/System/Library/Caches/*` |
| sentry-crash-reports | Sentry crash reports | `~/Library/Caches/SentryCrash/*` |
| kscrash-reports | KSCrash reports | `~/Library/Caches/KSCrash/*` |
| crashlytics-data | Crashlytics data | `~/Library/Caches/com.crashlytics.data/*` |

## Excluded

- mole 注释：CocoaPods / Flutter / Dart Pub
- `user.sh` 广域
- guard / custom / sudo
- **保护层冲突**（mole 有 explicit safe_clean 但 v1 保护层 skip）：Navicat / DBeaver / MongoDB Compass / Redis Insight / Paw — 待保护层 refine 后再移植

## Milestone

Phase 4c+ Batch 7 — enabled rules **230**；库存 `ported` **225/513**（≈44%）。
