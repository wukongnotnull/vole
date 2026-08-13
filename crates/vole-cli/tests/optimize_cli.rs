use std::fs;
use std::process::{Command, Stdio};

fn vole() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vole"));
    cmd.env("VOLE_TEST_NO_AUTH", "1");
    cmd
}

#[test]
fn optimize_help_lists_command() {
    let output = vole()
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
fn optimize_help_mentions_whitelist() {
    let output = vole().args(["optimize", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("whitelist"),
        "help missing whitelist: {stdout}"
    );
}

#[test]
fn optimize_whitelist_list_add_remove_non_tty() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(home.join(".config/vole")).unwrap();

    let add = vole()
        .env("HOME", &home)
        .args(["optimize", "--whitelist-add", "dock_refresh"])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&add.stderr)
    );

    let list = vole()
        .env("HOME", &home)
        .args(["optimize", "--whitelist-list"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("dock_refresh"), "list stdout={stdout}");

    let remove = vole()
        .env("HOME", &home)
        .args(["optimize", "--whitelist-remove", "dock_refresh"])
        .output()
        .unwrap();
    assert!(
        remove.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&remove.stderr)
    );

    let list2 = vole()
        .env("HOME", &home)
        .args(["optimize", "--whitelist-list"])
        .output()
        .unwrap();
    assert!(list2.status.success());
    let stdout2 = String::from_utf8_lossy(&list2.stdout);
    assert!(
        stdout2.contains("白名单为空") || !stdout2.contains("dock_refresh"),
        "list2={stdout2}"
    );
}

#[test]
fn optimize_whitelist_flag_non_tty_errors() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let output = vole()
        .env("HOME", &home)
        .args(["optimize", "--whitelist"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_ne!(output.status.code(), Some(0));
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        err.contains("whitelist-add")
            || err.contains("whitelist-remove")
            || err.contains("whitelist-list")
            || err.contains("非交互"),
        "unexpected err={err}"
    );
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
