//! 测试间共享环境锁，避免并行修改 `HOME` 等全局状态。

use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());

pub fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap()
}
