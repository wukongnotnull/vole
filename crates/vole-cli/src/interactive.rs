//! Minimal interactive menu when `vole` is run with no subcommand.

use std::io::{self, BufRead, IsTerminal, Write};
use std::process::Command;

pub fn run() -> i32 {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("vole: run with a subcommand (see --help), or in a terminal for the menu");
        return 2;
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        if writeln!(
            stdout,
            "\nVole\n  1) status (--json snapshot)\n  2) clean --plan\n  3) uninstall --plan\n  4) history\n  5) quit"
        )
        .is_err()
        {
            return 1;
        }
        if write!(stdout, "Select [1-5]: ").is_err() || stdout.flush().is_err() {
            return 1;
        }

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => return 0,
            Ok(_) => {}
            Err(e) => {
                eprintln!("vole: {e}");
                return 1;
            }
        }
        match line.trim() {
            "1" => {
                if let Err(msg) = run_child(&["status", "--json"]) {
                    let _ = writeln!(stdout, "{msg}");
                }
            }
            "2" => {
                if let Err(msg) = run_child(&["clean", "--plan"]) {
                    let _ = writeln!(stdout, "{msg}");
                }
            }
            "3" => {
                if let Err(msg) = run_child(&["uninstall", "--plan"]) {
                    let _ = writeln!(stdout, "{msg}");
                }
            }
            "4" => {
                if let Err(msg) = run_child(&["history"]) {
                    let _ = writeln!(stdout, "{msg}");
                }
            }
            "5" | "q" | "quit" | "exit" => return 0,
            other => {
                let _ = writeln!(stdout, "Unknown choice: {other}");
            }
        }
    }
}

/// Spawn a fresh `vole` process so submenu failures / signals don't tear down the menu,
/// and so clean doesn't accumulate in-process signal threads.
fn run_child(args: &[&str]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("vole: resolve executable: {e}"))?;
    let status = Command::new(exe)
        .args(args)
        .status()
        .map_err(|e| format!("vole: spawn {}: {e}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "vole {}: exited {}",
            args.first().unwrap_or(&"?"),
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into())
        ))
    }
}
