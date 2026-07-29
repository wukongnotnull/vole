//! Minimal interactive menu when `vole` is run with no subcommand.

use std::io::{self, BufRead, IsTerminal, Write};

use crate::clean;
use crate::history_cmd;

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
            "\nVole\n  1) status (--json snapshot)\n  2) clean --plan\n  3) history\n  4) quit"
        )
        .is_err()
        {
            return 1;
        }
        if write!(stdout, "Select [1-4]: ").is_err() || stdout.flush().is_err() {
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
                if let Err(e) = crate::cmd_status(true, false) {
                    eprintln!("vole status: {e}");
                    return 1;
                }
            }
            "2" => {
                let code = clean::run_clean(clean::CleanOptions {
                    json: false,
                    json_stream: false,
                    plan_out: None,
                    apply_plan: None,
                    permanent: false,
                    whitelist: false,
                    whitelist_add: None,
                    whitelist_remove: None,
                    whitelist_list: false,
                });
                if code != 0 {
                    return code;
                }
            }
            "3" => {
                let code = history_cmd::run(false, 20);
                if code != 0 {
                    return code;
                }
            }
            "4" | "q" | "quit" | "exit" => return 0,
            other => {
                let _ = writeln!(stdout, "Unknown choice: {other}");
            }
        }
    }
}
