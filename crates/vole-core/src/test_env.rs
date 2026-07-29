//! 测试间共享环境锁，避免并行修改 `HOME` 等全局状态。

use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());

pub fn lock() -> MutexGuard<'static, ()> {
    // 某测试 panic 后 Mutex 会 poisoned；恢复内层锁，避免后续测试全部连锁失败。
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
