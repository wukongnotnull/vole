use std::fs;
use std::process::Command;

#[test]
fn worktree_help_lists_command_and_avoids_safe_verdict() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["worktree", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    assert!(stdout.contains("--plan"));
    assert!(stdout.contains("--apply"));
    assert!(stdout.contains("trash") || stdout.contains("prune"));
    assert!(!stdout.contains("safe to delete"));
    assert!(!stdout.contains("deletable"));
}

#[test]
fn plan_json_lists_extra_worktree_and_apply_trashes_it() {
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
        .args(["worktree", "--plan", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("worktree:linked") || stdout.contains("worktree:orphan"));
    assert!(!stdout.contains("\"safe\""));
    let plan: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let entries = plan["entries"].as_array().unwrap();
    assert!(entries
        .iter()
        .all(|e| e["path"] != repo.to_string_lossy().as_ref()));

    let plan_path = dir.path().join("plan.json");
    fs::write(&plan_path, stdout.as_bytes()).unwrap();
    let trash = dir.path().join("trash");
    fs::create_dir_all(&trash).unwrap();
    let apply = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("MOLE_TEST_TRASH_DIR", &trash)
        .current_dir(&repo)
        .args(["worktree", "--apply", plan_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert!(repo.join("README").exists(), "primary checkout must remain");
    assert!(!wt.exists(), "worktree checkout should be gone");
    let list = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(list.status.success());
    let listed = String::from_utf8_lossy(&list.stdout);
    let wt_s = wt.canonicalize().unwrap_or(wt.clone());
    assert!(
        !listed.contains(wt_s.to_string_lossy().as_ref())
            && !listed.contains(wt.to_string_lossy().as_ref()),
        "worktree still registered: {listed}"
    );
}

#[test]
fn agent_container_worktree_is_trashed_and_pruned_not_whitelisted() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let repo = home.join("Projects/repo");
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
    let wt = home.join(".claude/worktrees/leftover-wt");
    fs::create_dir_all(home.join(".claude/worktrees")).unwrap();
    git(&["worktree", "add", "--detach", wt.to_str().unwrap()]);

    let out = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .current_dir(home)
        .args(["worktree", "--plan", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let plan: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let wt_canon = wt.canonicalize().unwrap();
    let entry = plan["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["path"] == wt_canon.to_string_lossy().as_ref())
        .unwrap_or_else(|| panic!("leftover worktree missing from plan: {stdout}"));
    let label = entry["label"].as_str().unwrap();
    let repo_canon = repo.canonicalize().unwrap();
    assert!(
        label.starts_with(&format!("repo:{} ", repo.display()))
            || label.starts_with(&format!("repo:{} ", repo_canon.display())),
        "label should name the primary checkout: {label}"
    );

    let plan_path = dir.path().join("plan.json");
    fs::write(&plan_path, stdout.as_bytes()).unwrap();
    let trash = dir.path().join("trash");
    fs::create_dir_all(&trash).unwrap();
    let apply = Command::new(env!("CARGO_BIN_EXE_vole"))
        .env("HOME", home)
        .env("MOLE_TEST_TRASH_DIR", &trash)
        .current_dir(home)
        .args(["worktree", "--apply", plan_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert!(repo.join("README").exists(), "primary checkout must remain");
    assert!(
        !wt.exists(),
        "leftover worktree should be gone; apply stdout={} stderr={}",
        String::from_utf8_lossy(&apply.stdout),
        String::from_utf8_lossy(&apply.stderr)
    );
    let list = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo)
        .output()
        .unwrap();
    let listed = String::from_utf8_lossy(&list.stdout);
    assert!(
        !listed.contains(wt_canon.to_string_lossy().as_ref())
            && !listed.contains(wt.to_string_lossy().as_ref()),
        "worktree still registered: {listed}"
    );
}
