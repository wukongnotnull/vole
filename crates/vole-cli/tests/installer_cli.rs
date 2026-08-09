use std::fs;
use std::process::Command;

#[test]
fn installer_help_lists_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["installer", "--help"])
        .output()
        .expect("run vole installer --help");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--plan") || stdout.contains("plan"));
    assert!(stdout.contains("--apply") || stdout.contains("apply"));
}

#[test]
fn installer_help_mentions_interactive_tty() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["installer", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.to_lowercase().contains("interactive")
            || stdout.contains("TTY")
            || stdout.contains("多选"),
        "{stdout}"
    );
}

#[test]
fn installer_plan_json_with_temp_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let downloads = home.join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    fs::write(downloads.join("App.dmg"), b"x").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env(
            "VOLE_INSTALLER_SCAN_ROOTS",
            downloads.to_string_lossy().as_ref(),
        )
        .args(["installer", "--plan", "--json"])
        .output()
        .expect("run vole installer --plan --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("schema_version") && stdout.contains("installer:dmg"),
        "unexpected plan json: {stdout}"
    );
}

#[test]
fn installer_listed_in_top_level_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["--help"])
        .output()
        .expect("run vole --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("installer"),
        "top-level help missing installer: {stdout}"
    );
}
