use std::io;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use crate::traits::{SysCommand, SysCommandError};

pub struct MacSysCommand;

impl SysCommand for MacSysCommand {
    fn run(&self, argv: &[&str], timeout: Duration) -> Result<Output, SysCommandError> {
        if argv.is_empty() {
            return Err(SysCommandError::Failed(1));
        }
        let mut child = Command::new(argv[0])
            .args(&argv[1..])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let start = std::time::Instant::now();
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    io::copy(&mut out, &mut stdout).ok();
                }
                if let Some(mut err) = child.stderr.take() {
                    io::copy(&mut err, &mut stderr).ok();
                }
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            if start.elapsed() > timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SysCommandError::Timeout);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
