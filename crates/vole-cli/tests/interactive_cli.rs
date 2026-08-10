use std::process::{Command, Stdio};

#[test]
fn bare_vole_non_tty_exits_without_hanging() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run vole");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subcommand") || stderr.contains("terminal"),
        "stderr={stderr}"
    );
}

#[test]
fn top_level_help_mentions_home_menu_not_numbered_plan_list() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["--help"])
        .output()
        .expect("help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // after_help 或 about：指向终端首页，而非暗示 11 项数字菜单
    assert!(
        stdout.to_lowercase().contains("home menu") || stdout.contains("mole-style"),
        "help={stdout}"
    );
    assert!(
        stdout.to_lowercase().contains("confirm")
            || stdout.contains("Proceed")
            || stdout.to_lowercase().contains("tty"),
        "expected T6 confirm-track mention in help: {stdout}"
    );
    assert!(
        !stdout.contains("plan-only until"),
        "stale T5 caveat still in help: {stdout}"
    );
    assert!(
        !stdout.contains("Select [1-11]"),
        "stale numbered menu leaked into help: {stdout}"
    );
}

#[test]
fn clean_help_mentions_tty_confirm() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["clean", "--help"])
        .output()
        .expect("help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("确认")
            || stdout.to_lowercase().contains("confirm")
            || stdout.contains("Proceed"),
        "clean help={stdout}"
    );
}

#[test]
fn optimize_help_mentions_tty_confirm() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["optimize", "--help"])
        .output()
        .expect("help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("确认")
            || stdout.to_lowercase().contains("confirm")
            || stdout.contains("Proceed"),
        "optimize help={stdout}"
    );
}
