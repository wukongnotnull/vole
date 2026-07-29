# Phase 4c Batch 5 selection

Target: **24** rules (v1 cap batch — lands on design Top **150**)  
**Actual: 24** (app-caches +12, user-devtools +12). Total enabled rules in tree ≈ **150**.

> Batch size below usual [30,50] interval by design: sized to complete v1 Top 100–150 target without overshooting.

## Block A — `data/rules/app-caches.toml` (+12)

| proposed_id | label | path |
|---|---|---|
| miaoyan-cache | MiaoYan cache | `~/Library/Caches/com.tw93.MiaoYan/*` |
| klee-cache | Klee cache | `~/Library/Caches/com.klee.desktop/*` |
| quark-video-cache | Quark video cache | `~/Library/Application Support/Quark/Cache/videoCache/*` |
| folo-cache | Folo cache | `~/Library/Containers/is.follow/Data/Library/Application Support/Folo/Cache/Cache_Data/*` |
| plex-cache | Plex cache | `~/Library/Caches/tv.plex.player.desktop` |
| netease-music-cache | NetEase Music cache | `~/Library/Caches/com.netease.163music` |
| qq-music-cache | QQ Music cache | `~/Library/Caches/com.tencent.QQMusic/*` |
| qq-music-mac-cache | QQ Music Mac cache | `~/Library/Caches/com.tencent.QQMusicMac/*` |
| iina-cache | IINA cache | `~/Library/Caches/com.colliderli.iina` |
| vlc-cache | VLC cache | `~/Library/Caches/org.videolan.vlc` |
| bilibili-cache | Bilibili cache | `~/Library/Caches/tv.danmaku.bili/*` |
| stremio-cache | Stremio cache | `~/Library/Caches/smart.stremio*/*` |

## Block B — `data/rules/user-devtools.toml` (+12)

| proposed_id | label | path |
|---|---|---|
| android-build-cache | Android build cache | `~/.android/build-cache/*` |
| android-sdk-cache | Android SDK cache | `~/.android/cache/*` |
| xcode-ib-cache | Xcode Interface Builder cache | `~/Library/Developer/Xcode/UserData/IB Support/*` |
| expo-native-modules-cache | Expo native modules cache | `~/.expo/native-modules-cache/*` |
| expo-schema-cache | Expo schema cache | `~/.expo/schema-cache/*` |
| expo-template-cache | Expo template cache | `~/.expo/template-cache/*` |
| expo-versions-cache | Expo versions cache | `~/.expo/versions-cache/*` |
| gradle-build-cache | Gradle build cache | `~/.gradle/caches/build-cache-*/*` |
| composer-cache-legacy | PHP Composer cache (legacy) | `~/.composer/cache/*` |
| composer-cache | PHP Composer cache | `~/Library/Caches/composer/*` |
| deno-cache | Deno cache | `~/Library/Caches/deno/*` |
| terraform-cache | Terraform cache | `~/.cache/terraform/*` |

## Excluded

- mole 注释掉的 CocoaPods / Flutter / Dart Pub
- guard / custom / user.sh 广域

## Milestone

**Phase 4c v1 complete** — enabled rules ≈ **150**.
