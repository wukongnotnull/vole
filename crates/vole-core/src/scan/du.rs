//! `du -skPx` 返回的字节数（对齐 mole `getDirectorySizeFromDu`）。

use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

const DU_TIMEOUT: Duration = Duration::from_secs(30);

/// 折叠目录体积；`du` 不可用时返回 0。
pub fn du_directory_size(path: &Path) -> std::io::Result<u64> {
    du_directory_size_inner(path).or(Ok(0))
}

fn du_directory_size_inner(path: &Path) -> std::io::Result<u64> {
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path not found: {}", path.display()),
        ));
    }

    let path = path.to_path_buf();
    let handle = thread::spawn(move || {
        Command::new("du")
            .args(["-skPx"])
            .arg(&path)
            .output()
    });

    let start = Instant::now();
    while !handle.is_finished() {
        if start.elapsed() >= DU_TIMEOUT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "du timeout",
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }

    let output = handle
        .join()
        .map_err(|_| std::io::Error::other("du thread panicked"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let fields: Vec<&str> = stdout.split_whitespace().collect();
    if fields.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "du output empty",
        ));
    }
    let kb: u64 = fields[0]
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(kb.saturating_mul(1024))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn du_reports_directory_size() {
        let dir = std::env::temp_dir().join(format!("vole-du-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = File::create(dir.join("data.bin")).unwrap();
        f.write_all(&vec![0u8; 8192]).unwrap();

        let size = du_directory_size(&dir).unwrap();
        assert!(size >= 8192, "du size {size}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
