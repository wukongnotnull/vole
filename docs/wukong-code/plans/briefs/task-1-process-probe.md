### Task 1: `ProcessProbe` + 单元测试（TDD）

**Files:**
- Create: `crates/vole-core/src/rules/process_guard.rs`
- Modify: `crates/vole-core/src/rules/mod.rs`
- Test: same module `#[cfg(test)]`

**Interfaces:**
- Produces:
  - `pub enum ProcessState { Running, Idle, Unknown }`
  - `pub trait ProcessProbe: Send + Sync { fn exact_name_running(&self, name: &str) -> ProcessState; }`
  - `pub fn should_skip_for_not_running(probe: &dyn ProcessProbe, names: &[String]) -> bool`
    - 空列表 → `false`
    - 忽略空字符串
    - 任一 `Running` 或 `Unknown` → `true`
  - `pub struct FakeProcessProbe { pub running: HashSet<String>, pub unknown: HashSet<String> }` （pub，供测试与注入）

Follow TDD: write failing tests first, then implement, then commit.

Commit message:
```
feat(rules): add ProcessProbe and not_running skip helper
```

Global: GPL-3.0-only; macOS only; vole-core forbid unsafe.
