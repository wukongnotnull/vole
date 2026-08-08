use std::fs;
use std::process::Command;

#[test]
fn purge_help_lists_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["purge", "--help"])
        .output()
        .expect("run vole purge --help");
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
fn purge_plan_json_with_temp_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let project = home.join("Projects/demo");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("package.json"), b"{}").unwrap();
    let nm = project.join("node_modules");
    fs::create_dir_all(nm.join("leftpad")).unwrap();
    fs::write(nm.join("leftpad/index.js"), b"x").unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(14 * 86_400);
    let times = std::fs::FileTimes::new().set_modified(old);
    fs::File::open(&nm).unwrap().set_times(times).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .args(["purge", "--plan", "--json"])
        .output()
        .expect("run vole purge --plan --json");
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("schema_version") && stdout.contains("purge:node_modules"),
        "unexpected plan json: {stdout}"
    );
}

#[test]
fn aliases_are_recognized() {
    for cmd in ["optimise", "analyse", "completion"] {
        let output = Command::new(env!("CARGO_BIN_EXE_vole"))
            .args([cmd, "--help"])
            .output()
            .unwrap_or_else(|_| panic!("run vole {cmd} --help"));
        assert!(
            output.status.success(),
            "{cmd} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
