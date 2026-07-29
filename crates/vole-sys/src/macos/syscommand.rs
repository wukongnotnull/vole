use std::process::{Command, Output};
use std::time::Duration;

use crate::traits::{SysCommand, SysCommandError};

pub struct MacSysCommand;

impl SysCommand for MacSysCommand {
    fn run(&self, argv: &[&str], timeout: Duration) -> Result<Output, SysCommandError> {
        if argv.is_empty() {
            return Err(SysCommandError::Failed(1));
        }
        let mut child = Command::new(argv[0]).args(&argv[1..]).spawn()?;
        let start = std::time::Instant::now();
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                return Ok(Output {
                    status,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
            }
            if start.elapsed() > timeout {
                return Err(SysCommandError::Timeout);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
