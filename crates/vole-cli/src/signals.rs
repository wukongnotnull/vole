//! SIGINT / SIGTERM → `CancelToken`。

use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use signal_hook::iterator::Signals;

use vole_core::cancel::CancelToken;

pub fn spawn_signal_cancel(cancel: CancelToken) {
    std::thread::spawn(move || {
        let mut signals = Signals::new([SIGINT, SIGTERM]).expect("register signals");
        // 收到首个信号即取消；`break` 使循环只处理一次。
        #[allow(clippy::never_loop)]
        for _ in &mut signals {
            cancel.cancel();
            break;
        }
    });
}
