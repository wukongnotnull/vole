//! 将扫描结果转为 `AnalyzeOutput`。

use std::path::Path;

use crate::vole_proto::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};

use crate::cancel::CancelToken;
use crate::scan::{scan_directory, scan_directory_with_progress, DirEntry, ScanResult};

pub fn analyze_directory(path: &Path, cancel: &CancelToken) -> std::io::Result<AnalyzeOutput> {
    let scan = scan_directory(path, cancel)?;
    Ok(to_analyze_output(path, false, scan))
}

/// 同 [`analyze_directory`]，每完成一个根子项回调（TUI live sort）。
pub fn analyze_directory_with_progress<F>(
    path: &Path,
    cancel: &CancelToken,
    mut on_child: F,
) -> std::io::Result<AnalyzeOutput>
where
    F: FnMut(AnalyzeEntry),
{
    let scan = scan_directory_with_progress(path, cancel, |entry| {
        on_child(dir_entry_to_analyze(entry));
    })?;
    Ok(to_analyze_output(path, false, scan))
}

pub fn to_analyze_output(path: &Path, overview: bool, scan: ScanResult) -> AnalyzeOutput {
    let path_str = path.to_string_lossy().into_owned();
    AnalyzeOutput {
        path: path_str,
        overview,
        entries: scan
            .entries
            .into_iter()
            .map(|e| dir_entry_to_analyze(&e))
            .collect(),
        large_files: scan
            .large_files
            .into_iter()
            .map(|f| AnalyzeFileEntry {
                name: f.name,
                path: f.path.to_string_lossy().into_owned(),
                size: f.size as i64,
            })
            .collect(),
        total_size: scan.total_size as i64,
        total_files: Some(scan.total_files as i64),
    }
}

fn dir_entry_to_analyze(e: &DirEntry) -> AnalyzeEntry {
    AnalyzeEntry {
        name: e.name.clone(),
        path: e.path.to_string_lossy().into_owned(),
        size: e.size as i64,
        is_dir: e.is_dir,
        insight: false,
        cleanable: false,
        last_access: e.last_access.map(format_access_time),
    }
}

fn format_access_time(t: std::time::SystemTime) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let dur = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();

    // 自 1970-01-01 起的 UTC RFC3339（无外部 crate）
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let year_days = if leap { 366 } else { 365 };
        if remaining_days < year_days {
            break;
        }
        remaining_days -= year_days;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0;
    while m < 12 && remaining_days >= month_days[m] as i64 {
        remaining_days -= month_days[m] as i64;
        m += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        y,
        m + 1,
        day,
        hour,
        minute,
        second,
        nanos
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn progress_matches_blocking_analyze() {
        let dir = std::env::temp_dir().join(format!("vole-analyze-prog-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut f = File::create(dir.join("x.bin")).unwrap();
        f.write_all(&[1u8; 128]).unwrap();
        File::create(dir.join("y.txt")).unwrap();

        let cancel = CancelToken::new();
        let mut kids = Vec::new();
        let live = analyze_directory_with_progress(&dir, &cancel, |e| {
            assert!(!e.path.is_empty());
            kids.push(e.path.clone());
        })
        .unwrap();
        let blocking = analyze_directory(&dir, &cancel).unwrap();
        assert_eq!(kids.len(), 2);
        assert_eq!(live.path, blocking.path);
        assert_eq!(live.total_size, blocking.total_size);
        assert_eq!(live.total_files, blocking.total_files);
        assert_eq!(live.entries.len(), blocking.entries.len());
        let _ = fs::remove_dir_all(&dir);
    }
}
