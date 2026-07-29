# Phase 4c Batch 6 selection

Target: **40** rules (Phase 4c+ first post-v1 batch; 150 → **190**)  
**Actual: 40** (app-caches +20, user-devtools +20).

## Block A — `data/rules/app-caches.toml` (+20)

| proposed_id | label | path |
|---|---|---|
| iqiyi-cache | iQIYI cache | `~/Library/Caches/com.iqiyi.player` |
| tencent-video-cache | Tencent Video cache | `~/Library/Caches/com.tencent.tenvideo` |
| kugou-music-cache | Kugou Music cache | `~/Library/Caches/com.kugou.mac/*` |
| kuwo-music-cache | Kuwo Music cache | `~/Library/Caches/com.kuwo.mac/*` |
| apple-tv-cache | Apple TV cache | `~/Library/Caches/com.apple.TV/*` |
| mpv-cache | MPV cache | `~/Library/Caches/io.mpv` |
| transmission-cache | Transmission cache | `~/Library/Caches/org.m0k.transmission` |
| aria2-cache | Aria2 cache | `~/Library/Caches/net.xmac.aria2gui` |
| google-clearcut-logs | Google Clearcut logs | `~/Library/Caches/CCTClearcutLogger` |
| alilang-security-component | AliLang security component | `~/Library/Caches/com.alibaba.AliLang.osx/*` |
| ora-browser-cache | Ora browser cache | `~/Library/Caches/com.orabrowser.app/*` |
| filo-cache | Filo cache | `~/Library/Caches/com.filo.client/*` |
| kaku-cache | Kaku cache | `~/.cache/kaku/*` |
| spacedrive-thumbnail-cache | Spacedrive thumbnail cache | `~/Library/Application Support/spacedrive/thumbnails/*` |
| stremio-server-cache | Stremio server cache | `~/Library/Application Support/stremio/stremio-server/stremio-cache/*` |
| senplayer-video-cache | SenPlayer video cache | `~/Library/Containers/com.wuziqi.SenPlayer/Data/tmp/videoCache/*` |
| douyu-cache | Douyu cache | `~/Library/Caches/com.douyu.*/*` |
| huya-cache | Huya cache | `~/Library/Caches/com.huya.*/*` |
| qq-music-container-cache | QQ Music container cache | `~/Library/Containers/com.tencent.QQMusicMac/Data/Library/Caches/*` |
| final-cut-pro-cache | Final Cut Pro cache | `~/Library/Caches/com.apple.FinalCut/*` |

## Block B — `data/rules/user-devtools.toml` (+20)

| proposed_id | label | path |
|---|---|---|
| sequel-ace-cache | Sequel Ace cache | `~/Library/Caches/com.sequel-ace.sequel-ace/*` |
| sequel-pro-cache | Sequel Pro cache | `~/Library/Caches/com.eggerapps.Sequel-Pro/*` |
| redis-desktop-manager-cache | Redis Desktop Manager cache | `~/Library/Caches/redis-desktop-manager/*` |
| sbt-boot-cache | SBT boot cache | `~/.sbt/boot/*` |
| sbt-launcher-cache | SBT launcher cache | `~/.sbt/launchers/*` |
| ivy-cache | Ivy cache | `~/.ivy2/cache/*` |
| bazel-cache | Bazel cache | `~/.cache/bazel/*` |
| zig-cache | Zig cache | `~/.cache/zig/*` |
| grafana-cache | Grafana cache | `~/.grafana/cache/*` |
| nuget-packages-cache | NuGet packages cache | `~/.nuget/packages/*` |
| jetbrains-ide-logs | JetBrains IDE logs | `~/Library/Logs/JetBrains/*` |
| container-storage-temp | Container storage temp | `~/.local/share/containers/storage/tmp/*` |
| gitlab-runner-cache | GitLab Runner cache | `~/.cache/gitlab-runner/*` |
| github-actions-cache | GitHub Actions cache | `~/.github/cache/*` |
| circleci-cache | CircleCI cache | `~/.circleci/cache/*` |
| sonarqube-cache | SonarQube cache | `~/.sonar/*` |
| prometheus-wal-cache | Prometheus WAL cache | `~/.prometheus/data/wal/*` |
| jenkins-workspace-cache | Jenkins workspace cache | `~/.jenkins/workspace/*/target/*` |
| postman-cache | Postman cache | `~/Library/Caches/com.postmanlabs.mac/*` |
| tableplus-cache | TablePlus cache | `~/Library/Caches/com.tinyapp.TablePlus/*` |

## Excluded

- mole 注释：CocoaPods / Flutter / Dart Pub
- `user.sh` 广域 sweep
- guard / custom / sudo
- Final Cut **generated** caches（guard；非本批 `final-cut-pro-cache` 静态路径）

## Milestone

Phase 4c+ Batch 6 — enabled rules **190**；库存 `ported` **184/513**（≈36%）。
