# idleassetsd-cfnetwork-tmp（1.23.0）

Mole 用 sudo 在 `/private/var/folders` 定位 `*/T/com.apple.idleassetsd`，再删其中 ≥7d 的 `CFNetworkDownload_*.tmp`（中止的 Aerial/动态壁纸下载，可达数百 GB）。

Vole 一规则：形状谓词要求 folders 根 + `/T/com.apple.idleassetsd/` 标记 + 确切文件名模式；apply 绑谓词 + `sudo -n`。
