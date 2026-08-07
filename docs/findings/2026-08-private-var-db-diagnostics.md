# `/private/var/db/diagnostics`（1.16.0）

Mole 先 `*`/7d 再 `*.tracev3`/30d；第一刀已含 `.tracev3`，第二刀几乎冗余。Vole 对 `.tracev3` 一律 **30d**。

## 落点

- `is_private_var_db_diagnostics_clean_target`：深度 ≤5（+ test remap）
- Privilege allow + plan walk（分龄过滤）；`older_than_days = 7` 初筛
- apply：形状 + `is_file` + 分龄重验 + `sudo -n` permanent

## 安全

- 不接受根目录 / 超深；三树 + 本 rule_id skip
- 7–29 天 `.tracev3` apply 必 skip
