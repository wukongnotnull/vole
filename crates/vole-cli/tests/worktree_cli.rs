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
