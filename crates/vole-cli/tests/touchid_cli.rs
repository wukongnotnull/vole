use std::fs;
use std::process::Command;

#[test]
fn touchid_help_lists_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["touchid", "--help"])
        .output()
        .expect("run vole touchid --help");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("status") || stdout.contains("enable") || stdout.contains("Touch"),
        "unexpected help: {stdout}"
    );
}

#[test]
fn touchid_status_json_with_injected_pam() {
    let dir = tempfile::tempdir().unwrap();
    let sudo = dir.path().join("sudo");
    let local = dir.path().join("sudo_local");
    fs::write(&sudo, "sudo_local\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("VOLE_TEST_NO_AUTH", "1")
        .env("VOLE_PAM_SUDO_FILE", &sudo)
        .env("VOLE_PAM_SUDO_LOCAL_FILE", &local)
        .args(["touchid", "status", "--json"])
        .output()
        .expect("run vole touchid status --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("configured"),
        "unexpected status json: {stdout}"
    );
}

#[test]
fn touchid_enable_dry_run_no_write() {
    let dir = tempfile::tempdir().unwrap();
    let sudo = dir.path().join("sudo");
    let local = dir.path().join("sudo_local");
    fs::write(&sudo, "sudo_local\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("VOLE_TEST_NO_AUTH", "1")
        .env("VOLE_PAM_SUDO_FILE", &sudo)
        .env("VOLE_PAM_SUDO_LOCAL_FILE", &local)
        .args(["touchid", "enable", "--dry-run", "--json"])
        .output()
        .expect("run vole touchid enable --dry-run");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!local.exists(), "dry-run must not create sudo_local");
}

#[test]
fn touchid_enable_with_injected_paths_writes_local() {
    let dir = tempfile::tempdir().unwrap();
    let sudo = dir.path().join("sudo");
    let local = dir.path().join("sudo_local");
    fs::write(&sudo, "sudo_local\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("VOLE_PAM_SUDO_FILE", &sudo)
        .env("VOLE_PAM_SUDO_LOCAL_FILE", &local)
        .args(["touchid", "enable", "--json"])
        .output()
        .expect("run vole touchid enable");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        local.exists() && fs::read_to_string(&local).unwrap().contains("pam_tid.so"),
        "expected sudo_local with pam_tid.so"
    );
}

#[test]
fn touchid_listed_in_top_level_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["--help"])
        .output()
        .expect("run vole --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("touchid"),
        "top-level help missing touchid: {stdout}"
    );
}
