use std::process::Command;

fn vole() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vole"))
}

/// Commands that treat `--dry-run` / `-n` as hidden aliases of `--plan`.
const PLAN_COMMANDS: &[&str] = &[
    "clean",
    "uninstall",
    "optimize",
    "worktree",
    "purge",
    "installer",
    "touchid",
    "remove",
];

#[test]
fn plan_commands_help_lists_plan_not_dry_run() {
    for cmd in PLAN_COMMANDS {
        let output = vole()
            .args([cmd, "--help"])
            .output()
            .unwrap_or_else(|_| panic!("run vole {cmd} --help"));
        assert!(
            output.status.success(),
            "{cmd} --help failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("--plan"),
            "{cmd} --help must list --plan:\n{stdout}"
        );
        assert!(
            !stdout.contains("--dry-run"),
            "{cmd} --help must hide --dry-run alias:\n{stdout}"
        );
    }
}

#[test]
fn hidden_dry_run_aliases_still_parse() {
    for cmd in PLAN_COMMANDS {
        for flag in ["--dry-run", "-n"] {
            let output = vole()
                .args([cmd, flag, "--help"])
                .output()
                .unwrap_or_else(|_| panic!("run vole {cmd} {flag} --help"));
            // `--help` still wins; unknown flags would fail before help.
            assert!(
                output.status.success(),
                "{cmd} {flag} must remain a recognized alias: stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
