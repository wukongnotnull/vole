//! Regression: `clean --apply --json-stream` must exit after emitting `done`.
//!
//! Bug: missing `drop(event_tx)` before joining the stream writer left the
//! NDJSON writer blocked on `recv` forever while the UI spinner spun.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn clean_apply_json_stream_exits_after_done() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(home.join(".cache/vole")).unwrap();

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let plan_path = dir.path().join("empty-plan.json");
    let mut plan = fs::File::create(&plan_path).unwrap();
    writeln!(
        plan,
        r#"{{"schema_version":1,"created_at":{created_at},"ttl_secs":900,"entries":[]}}"#
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_vole");
    let rules = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/rules");
    let plan_arg = plan_path.to_str().unwrap().to_string();
    let home_arg = home.clone();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let output = Command::new(bin)
            .env("HOME", &home_arg)
            .env("VOLE_RULES_DIR", rules)
            .args(["clean", "--apply", &plan_arg, "--json-stream"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(output);
    });

    let output = match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => panic!("failed to run vole: {e}"),
        Err(_) => panic!(
            "vole clean --apply --json-stream hung >10s (missing drop(event_tx)?)"
        ),
    };

    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""type":"done""#),
        "expected done event in stream, got: {stdout}"
    );
}
