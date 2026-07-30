use std::fs;
use std::process::Command;

#[test]
fn optimize_help_lists_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["optimize", "--help"])
        .output()
        .expect("run vole optimize --help");
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
fn optimize_plan_json_with_temp_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let state = home.join("Library/Saved Application State/com.example.old.savedState");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("w.plist"), b"x").unwrap();
    // Age the directory so saved_state discoverer includes it.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86_400);
    let times = std::fs::FileTimes::new().set_modified(old);
    fs::File::open(&state).unwrap().set_times(times).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .args([
            "optimize",
            "--plan",
            "--json",
            "--task",
            "saved_state_cleanup",
        ])
        .output()
        .expect("run vole optimize --plan --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("schema_version") && stdout.contains("optimize:delete:saved_state_cleanup"),
        "unexpected plan json: {stdout}"
    );
}
