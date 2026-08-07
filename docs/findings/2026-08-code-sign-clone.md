# code-sign-clone（1.24.0）

Mole 扫 `/private/var/folders` 下 `*/X/*` 中名为 `*.code_sign_clone` 的目录（浏览器代码签名缓存），跳过 EDR 代理路径，整目录 `safe_sudo_remove`，无年龄过滤。

Vole 一规则对齐该形状；apply 与 `path_allowed_for_privilege` 双重跳过 EDR。
