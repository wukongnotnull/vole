use std::process::{Command, Stdio};

#[test]
fn clean_non_tty_stays_plan_only() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["clean"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("VOLE_TEST_NO_AUTH", "1")
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Non-TTY stdout forces JSON plan (should_use_json), not human "Plan:" text.
    assert!(
        combined.contains("schema_version") || combined.contains("\"entries\""),
        "expected JSON plan output on non-TTY"
    );
    assert!(
        !combined.contains("Proceed with clean?"),
        "must not prompt on non-TTY"
    );
}

#[test]
fn optimize_non_tty_stays_plan_only() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["optimize"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("VOLE_TEST_NO_AUTH", "1")
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("optimize plan:") || combined.contains("entries"),
        "expected plan output, got {combined}"
    );
    assert!(
        !combined.contains("Proceed with optimize?"),
        "must not prompt on non-TTY"
    );
}
