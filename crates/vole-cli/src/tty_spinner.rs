//! Simple stderr spinner for long-running TTY prep work (mole-style `|/-\\`).

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const FRAMES: &[char] = &['|', '/', '-', '\\'];

/// Background spinner on stderr until [`TtySpinner::stop`] or drop.
pub struct TtySpinner {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TtySpinner {
    /// Start spinning `message` on stderr (e.g. `"Scanning applications..."`).
    pub fn start(message: impl Into<String>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let msg = message.into();
        let stop_flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut i = 0usize;
            while !stop_flag.load(Ordering::Relaxed) {
                eprint!("\r{} {} ", FRAMES[i % FRAMES.len()], msg);
                let _ = io::stderr().flush();
                i = i.wrapping_add(1);
                thread::sleep(Duration::from_millis(80));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }

    /// Stop the spinner thread and clear the current stderr line.
    pub fn stop(mut self) {
        self.finish();
    }

    fn finish(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        eprint!("\r\x1b[2K");
        let _ = io::stderr().flush();
    }
}

impl Drop for TtySpinner {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_cycle_four_glyphs() {
        assert_eq!(FRAMES, &['|', '/', '-', '\\']);
    }

    #[test]
    fn start_and_stop_do_not_panic() {
        let spinner = TtySpinner::start("Scanning applications...");
        thread::sleep(Duration::from_millis(30));
        spinner.stop();
    }

    #[test]
    fn drop_stops_spinner() {
        let spinner = TtySpinner::start("Scanning...");
        thread::sleep(Duration::from_millis(30));
        drop(spinner);
    }
}
