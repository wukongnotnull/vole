use std::fs;
use std::process::Command;

#[test]
fn clean_plan_human_shows_build_artifact_hint() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let root = home.join("hints-root");
    fs::create_dir_all(root.join("proj/node_modules")).unwrap();
    fs::write(root.join("proj/package.json"), b"{}").unwrap();
    let cfg = home.join(".config/vole");
    fs::create_dir_all(&cfg).unwrap();
    fs::write(cfg.join("purge_paths"), format!("{}\n", root.display())).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("VOLE_TIMEOUT_HINT_SCAN_SEC", "15")
        .args(["clean", "--plan"])
        .output()
        .expect("run vole clean --plan");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Build artifacts") && combined.contains("vole purge"),
        "expected hints in output: {combined}"
    );
}

#[test]
fn clean_plan_json_includes_project_artifacts_hint() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let root = home.join("hints-root");
    fs::create_dir_all(root.join("proj/node_modules")).unwrap();
    fs::write(root.join("proj/package.json"), b"{}").unwrap();
    let cfg = home.join(".config/vole");
    fs::create_dir_all(&cfg).unwrap();
    fs::write(cfg.join("purge_paths"), format!("{}\n", root.display())).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .args(["clean", "--plan", "--json"])
        .output()
        .expect("run vole clean --plan --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"hints\"") && stdout.contains("project_artifacts"),
        "unexpected json: {stdout}"
    );
}

#[test]
fn top_level_hints_command_is_rejected() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["hints"])
        .output()
        .expect("run vole hints");
    assert!(
        !output.status.success(),
        "vole hints must not be a top-level command"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized")
            || stderr.contains("unexpected")
            || stderr.contains("error")
            || stderr.contains("hints"),
        "stderr={stderr}"
    );
}

#[test]
fn clean_plan_json_includes_launch_agents_hint() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let agents = home.join("Library/LaunchAgents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(
        agents.join("com.example.stale.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.example.stale</string>
    <key>Program</key><string>/Applications/Missing.app/Contents/MacOS/Missing</string>
</dict>
</plist>
"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("VOLE_TIMEOUT_HINT_SCAN_SEC", "15")
        .args(["clean", "--plan", "--json"])
        .output()
        .expect("run vole clean --plan --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"hints\"") && stdout.contains("launch_agents"),
        "unexpected json: {stdout}"
    );
    assert!(!stdout.contains("\"schema_version\":2"));
}

#[test]
fn clean_plan_human_launch_agent_and_orphan_skip_or_hit() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    fs::create_dir_all(home.join("Library/LaunchAgents")).unwrap();
    fs::write(
        home.join("Library/LaunchAgents/com.example.stale.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Program</key><string>/Applications/Missing.app/Contents/MacOS/Missing</string>
</dict></plist>
"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("VOLE_TIMEOUT_HINT_SCAN_SEC", "15")
        .args(["clean", "--plan"])
        .output()
        .expect("plan");
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Stale login item") && combined.contains("com.example.stale.plist"),
        "{combined}"
    );
}

#[test]
fn clean_plan_no_hint_kinds_when_home_empty() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("VOLE_TIMEOUT_HINT_SCAN_SEC", "15")
        .args(["clean", "--plan"])
        .output()
        .expect("plan");
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains("Stale login") && !combined.contains("Orphan dotfiles"),
        "no-hit must print nothing: {combined}"
    );
}

#[test]
fn clean_plan_zero_hint_budget_prints_skip_lines() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    fs::create_dir_all(home.join("Library/LaunchAgents")).unwrap();
    fs::create_dir_all(home.join(".fooapp")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("VOLE_TIMEOUT_HINT_SCAN_SEC", "0")
        .args(["clean", "--plan"])
        .output()
        .expect("plan");
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("scan skipped"),
        "expected skip line: {combined}"
    );
}

#[test]
fn clean_apply_does_not_emit_hints() {
    let dir = tempfile::tempdir().unwrap();
    let plan = dir.path().join("plan.json");
    fs::write(
        &plan,
        r#"{"schema_version":1,"created_at":1700000000,"ttl_secs":900,"entries":[]}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", dir.path())
        .args(["clean", "--apply", plan.to_str().unwrap(), "--json"])
        .output()
        .expect("apply");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("launch_agents") && !stderr.contains("Stale login"),
        "apply must not run hints: stdout={stdout} stderr={stderr}"
    );
}
