use std::fs;
use std::process::Command;

#[test]
fn help_lists_remove() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .arg("--help")
        .output()
        .expect("run vole --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("remove"), "unexpected help: {stdout}");
}

fn remove_preview_json(flag: &str) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(home.join(".config/vole")).unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("vole");
    fs::write(&exe, b"x").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["remove", flag, "--json"])
        .env("HOME", &home)
        .env("VOLE_CONFIG_DIR", home.join(".config/vole"))
        .env("VOLE_UPDATE_EXE", &exe)
        .env("VOLE_NO_OPLOG", "1")
        .output()
        .unwrap_or_else(|_| panic!("run vole remove {flag} --json"));
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("config") || stdout.contains(".config/vole"),
        "{stdout}"
    );
}

#[test]
fn plan_json_lists_config() {
    remove_preview_json("--plan");
}

#[test]
fn dry_run_json_lists_config() {
    remove_preview_json("--dry-run");
}
