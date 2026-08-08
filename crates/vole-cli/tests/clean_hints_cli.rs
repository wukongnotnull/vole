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
