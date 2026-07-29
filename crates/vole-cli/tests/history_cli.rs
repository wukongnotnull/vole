use std::fs;
use std::process::Command;

#[test]
fn history_json_limit_reads_temp_home_logs() {
    let home = tempfile::tempdir().expect("temp home");
    let log_dir = home.path().join("Library/Logs/mole");
    fs::create_dir_all(&log_dir).expect("mkdir logs");
    fs::write(
        log_dir.join("operations.log"),
        "\
# ========== clean session started at 2026-05-24 10:00:00 ==========
[2026-05-24 10:00:01] [clean] TRASHED /tmp/a (1KB)
# ========== clean session ended at 2026-05-24 10:00:02, 1 items, 1KB ==========
# ========== purge session started at 2026-05-24 11:00:00 ==========
[2026-05-24 11:00:01] [purge] REMOVED /tmp/b (2KB)
# ========== purge session ended at 2026-05-24 11:00:02, 1 items, 2KB ==========
",
    )
    .expect("write ops");
    fs::write(
        log_dir.join("deletions.log"),
        "2026-05-24T10:00:02+0000\ttrash\t1\tok\t/tmp/a\n\
2026-05-24T11:00:01+0000\tpermanent\t2\tok\t/tmp/b\n",
    )
    .expect("write dels");

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["history", "--json", "--limit", "5"])
        .env("HOME", home.path())
        .env_remove("MOLE_OPERATIONS_LOG")
        .env_remove("OPERATIONS_LOG_FILE")
        .env_remove("MOLE_DELETE_LOG")
        .output()
        .expect("run vole history");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let data: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(data["limit"], 5);
    assert_eq!(data["sessions"][0]["command"], "purge");
    assert_eq!(data["sessions"][1]["command"], "clean");
    assert_eq!(data["sessions"][1]["actions"]["trashed"], 1);
    assert_eq!(data["deletions"][0]["path"], "/tmp/b");
}
