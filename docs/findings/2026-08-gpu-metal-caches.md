# gpu-metal-caches（1.25.0）

Mole 仅清理 Darwin `C/` 下重建型 GPU 缓存（metal/metalfe/gpuarchiver），且用「目录内无近期文件」判定 stale（默认 1 天），避免误删活动 shader 缓存；跳过 EDR。

Vole 对齐该形状与 stale 语义；apply 绑谓词 + EDR + stale + `sudo -n`。
