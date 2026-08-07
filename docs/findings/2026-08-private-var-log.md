# `/private/var/log`（1.15.0）

Mole probe 用 maxdepth 3；真正删除走 `safe_sudo_find_delete` **maxdepth 5**。Vole 对齐删除侧。

## 落点

- `is_private_var_log_clean_target`：深度 ≤5 + `.log|.gz|.asl`（+ test remap）
- Privilege allow + plan walk；`older_than_days = 7`
- apply：形状 + `is_file` + 年龄重验 + `sudo -n` permanent

## 安全

- 不接受根目录 / 超深 / 其它扩展名
- 新鲜文件 apply 必 skip；三树路径即使带本 rule_id 也 skip
