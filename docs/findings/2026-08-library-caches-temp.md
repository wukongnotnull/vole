# library-caches-temp（1.22.0）

Mole 对 `/Library/Caches` 单次 find：maxdepth 5、`*.cache`/`*.tmp` ≥ TEMP age、`*.log` ≥ LOG age（当前均为 7 天），经 `should_protect_path` 后 `safe_sudo_remove`。

Vole 一规则 `library-caches-temp`：形状谓词绑 apply + `path_allowed_for_privilege`，`sudo -n` permanent；含 `com.apple.*` 子树（用户选择对齐 Mole，不收紧）。
