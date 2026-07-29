//! 终端 RAII 与 panic 恢复。

use std::io::{self, stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;

static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(EnableMouseCapture)?;
        TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
        Ok(Self { active: true })
    }

    pub fn restore(&mut self) {
        if self.active {
            restore_terminal_global();
            self.active = false;
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

pub fn restore_terminal_global() {
    if TERMINAL_ACTIVE.load(Ordering::SeqCst) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(DisableMouseCapture);
        let _ = stdout().execute(LeaveAlternateScreen);
        TERMINAL_ACTIVE.store(false, Ordering::SeqCst);
    }
}

pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal_global();
        original(info);
    }));
}
