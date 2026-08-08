use std::fs;
use std::process::Command;

#[test]
fn help_lists_update() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .arg("--help")
        .output()
        .expect("run vole --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("update"), "unexpected help: {stdout}");
}

#[test]
fn update_help_lists_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["update", "--help"])
        .output()
        .expect("run vole update --help");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--force"), "{stdout}");
    assert!(stdout.contains("--nightly"), "{stdout}");
    assert!(stdout.contains("--check"), "{stdout}");
}

#[test]
fn check_json_reports_origin_without_network_install() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("vole");
    fs::write(&exe, b"#!/bin/sh\necho vole 0.0.1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&exe).unwrap().permissions();
        p.set_mode(0o755);
        fs::set_permissions(&exe, p).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("VOLE_UPDATE_EXE", &exe)
        .env("VOLE_CONFIG_DIR", dir.path().join("config"))
        .env("VOLE_UPDATE_FAKE", "9.9.9")
        .args(["update", "--check", "--json"])
        .output()
        .expect("run vole update --check --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"origin\""), "{stdout}");
    assert!(stdout.contains("manual"), "{stdout}");
    assert!(stdout.contains("9.9.9"), "{stdout}");
}
