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
