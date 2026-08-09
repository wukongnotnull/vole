use std::fs;
use std::process::Command;

#[test]
fn uninstall_help_lists_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["uninstall", "--help"])
        .output()
        .expect("run vole uninstall --help");
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
fn uninstall_help_mentions_interactive_tty() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["uninstall", "--help"])
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
fn uninstall_plan_json_with_fixture_apps_dir() {
    let dir = tempfile::tempdir().unwrap();
    let apps = dir.path().join("Applications");
    fs::create_dir_all(&apps).unwrap();
    let app = apps.join("FixtureApp.app");
    let contents = app.join("Contents");
    fs::create_dir_all(&contents).unwrap();
    fs::write(
        contents.join("Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.fixtureapp</string>
<key>CFBundleName</key><string>FixtureApp</string>
</dict></plist>"#,
    )
    .unwrap();

    let home = dir.path().join("home");
    fs::create_dir_all(home.join("Library")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", &home)
        .env("VOLE_APPLICATIONS_DIR", &apps)
        .args(["uninstall", "--plan", "--json"])
        .output()
        .expect("run vole uninstall --plan --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("schema_version") && stdout.contains("uninstall:"),
        "unexpected plan json: {stdout}"
    );
    assert!(stdout.contains("com.example.fixtureapp") || stdout.contains("FixtureApp.app"));
}
