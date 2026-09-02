use std::fs;
use std::process::Command;

#[test]
fn agent_help_lists_command_and_avoids_safe_verdict() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["agent", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    assert!(stdout.contains("--plan"));
    assert!(stdout.contains("--apply"));
    assert!(stdout.contains("trash"));
    assert!(!stdout.contains("safe to delete"));
    assert!(!stdout.contains("deletable"));
}

#[test]
fn plan_json_lists_cache_and_apply_trashes_it() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let cache = home.join(".cursor/Cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("x"), b"hello").unwrap();
    let cwd = home.join("Projects/demo");
    fs::create_dir_all(&cwd).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("VOLE_TIMEOUT_AGENT_SCAN_SEC", "15")
        .current_dir(&cwd)
        .args(["agent", "--plan", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("agent:cache"));
    assert!(!stdout.contains("\"safe\""));
    let plan: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(plan["schema_version"], 1);

    let plan_path = dir.path().join("plan.json");
    fs::write(&plan_path, stdout.as_bytes()).unwrap();
    let trash = dir.path().join("trash");
    fs::create_dir_all(&trash).unwrap();
    let apply = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("MOLE_TEST_TRASH_DIR", &trash)
        .current_dir(&cwd)
        .args(["agent", "--apply", plan_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert!(!cache.exists(), "cache should be gone");
}

#[test]
fn plan_excludes_worktree_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let repo = home.join("Projects/demo");
    fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        let st = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_AUTHOR_NAME", "vole")
            .env("GIT_AUTHOR_EMAIL", "vole@test")
            .env("GIT_COMMITTER_NAME", "vole")
            .env("GIT_COMMITTER_EMAIL", "vole@test")
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?}");
    };
    git(&["init"]);
    fs::write(repo.join("README"), b"x").unwrap();
    git(&["add", "README"]);
    git(&["commit", "-m", "init"]);
    let wt = repo.join(".worktrees/old");
    fs::create_dir_all(repo.join(".worktrees")).unwrap();
    let st = Command::new("git")
        .args(["worktree", "add", "--detach", wt.to_str().unwrap()])
        .current_dir(&repo)
        .status()
        .unwrap();
    assert!(st.success());

    let out = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .current_dir(&repo)
        .args(["agent", "--plan", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains(wt.to_string_lossy().as_ref()));
    assert!(!stdout.contains("worktree:"));
}

#[test]
fn top_level_hints_still_rejected() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["hints"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
