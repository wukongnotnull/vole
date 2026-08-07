# `/private/var/db/DiagnosticPipeline`（1.17.0）

Mole `safe_sudo_find_delete …/DiagnosticPipeline "*" 7`（maxdepth 5）。

## 落点

- 形状谓词深度 ≤5；Privilege allow；plan walk；apply 绑谓词 + 7d + `sudo -n`
- 三树伪造 skip
