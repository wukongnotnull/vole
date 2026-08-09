use std::fs;
use std::process::{Command, Stdio};

fn vole() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vole"));
    cmd.env("VOLE_TEST_NO_AUTH", "1");
    cmd
}

#[test]
fn whitelist_help_mentions_paginated_or_flags() {
    let output = vole().args(["clean", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("whitelist-add")
            && (stdout.contains("分页")
                || stdout.to_lowercase().contains("paginated")
                || stdout.contains("TTY")
                || stdout.contains("多选")),
        "{stdout}"
    );
}

#[test]
fn whitelist_list_add_remove_non_tty() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(home.join(".config/mole")).unwrap();

    let add = vole()
        .env("HOME", &home)
        .args([
            "clean",
            "--whitelist-add",
            "~/Library/Caches/example-keep/*",
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&add.stderr)
    );

    let list = vole()
        .env("HOME", &home)
        .args(["clean", "--whitelist-list"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("example-keep"), "list stdout={stdout}");

    let remove = vole()
        .env("HOME", &home)
        .args([
            "clean",
            "--whitelist-remove",
            "~/Library/Caches/example-keep/*",
        ])
        .output()
        .unwrap();
    assert!(
        remove.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&remove.stderr)
    );

    let list2 = vole()
        .env("HOME", &home)
        .args(["clean", "--whitelist-list"])
        .output()
        .unwrap();
    assert!(list2.status.success());
    let stdout2 = String::from_utf8_lossy(&list2.stdout);
    assert!(
        stdout2.contains("白名单为空") || !stdout2.contains("example-keep"),
        "list2={stdout2}"
    );
}

#[test]
fn whitelist_flag_non_tty_errors() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let output = vole()
        .env("HOME", &home)
        .args(["clean", "--whitelist"])
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
            || err.contains("非交互")
            || err.contains("InvalidInput")
            || err.contains("whitelist-list"),
        "err={err}"
    );
}
