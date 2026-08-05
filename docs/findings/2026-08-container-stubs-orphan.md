# Findings：container stubs orphan（v1.6.0）

日期：2026-08-05。实现 `orphaned-container-stubs` 期间沉淀的关键结论。

## 1. 保护层冲突是本刀的核心工程问题

`data/protection.toml` 把 `com.macpaw.*` 列入 `data_protected_bundles`，cleanup 模式下
`should_protect_path` 对 `~/Library/Containers/com.macpaw.*` 恒保护。若沿用共享闸口
（plan 的 `validate_path_for_deletion` / apply 的 `verify_plan_entry_for_apply` /
`mole_delete_verified`），该规则**永远空转**——Mole 在 `_remove_verified_container_stub`
里同样刻意绕开 `safe_remove` 并留了注释警告勿「统一」回去。Vole 采用同构解法：

- plan：`rule_id == orphaned-container-stubs` 时不调 `validate_path_for_deletion`，
  改为窄形状校验（路径必须恰为 `~/Library/Containers/<单层名>`）；失败按
  `NeedsPrivilege`（对齐 `ProtectedPath` 映射）skip。
- apply：早分支专用 carve-out——`verify_plan_entry`（无 protect 的身份 TOCTOU）+
  `remove_verified_container_stub`（重验 stub 形状后 unlink metadata + rmdir）。
  绝不落入 trash / `mole_delete_verified`；`--permanent` 不改变行为。

## 2. TeamID 前缀其实仍是 reverse-DNS

spec 曾假设 `S8EX82NJP6.com.macpaw.*` 这类 TeamID 前缀非 reverse-DNS、须跳过 mdfind。
实测 Mole 的 `mole_is_reverse_dns_bundle_id` 正则（与 Vole `is_reverse_dns_bundle_id`
完全同形）**接受**纯字母数字段的 TeamID 前缀 → 它们照走 mdfind fail-closed。
mdfind 跳过只发生在真正非 reverse-DNS 的名字（如含空格）。实现按 Mole 行为落地，
测试覆盖两种形状。

## 3. rmdir 非空失败就是 TOCTOU 防线

carve-out 不做「先扫再删」的复杂锁：metadata unlink 后若目录长出新内容，
`fs::remove_dir` 非空必失败，目录原样保留（skip 计入 `PathVanished`）。
加上 apply 前的身份重验（塞入 `Data/` 会改父目录 mtime → identity mismatch），
双保险都命中同一 skip 语义。

## 4. 豁免面最小化的验收方式

- plan 集成测试用**真 embedded protection** 构造 `com.macpaw.*` stub，断言能入选
  （证明未死于 validate），同时既有 property 测试保证其它路径保护不变。
- apply 测试用 FakeTrash 断言 carve-out 全程不触碰废纸篓。

## 5. 版本与规则

- 规则数 513 → **514**；`zzz-orphaned.toml` 内顺序：orphaned-app-data →
  orphaned-container-stubs → orphaned-system-services（末位断言同步更新）。
- 零 `schema_version` 变更；无新 `SkipReason` 变体（形状失败复用 NeedsPrivilege，
  降级复用 TccDenied）。
