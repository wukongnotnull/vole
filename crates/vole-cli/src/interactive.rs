//! Bare `vole` home menu — mole-aligned ratatui navigation shell.

use std::io::{self, IsTerminal, Write};
use std::process::Command;

use vole_core::ops::{is_touchid_configured, resolve_touchid_paths};

use crate::tui::{run_home_menu, HomeAction, HomeMenuConfig, HomeMenuRunOpts};
use crate::update_banner::read_update_message_cache;

pub fn run() -> i32 {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("vole: run with a subcommand (see --help), or in a terminal for the menu");
        return 2;
    }

    let touchid_configured = is_touchid_configured(&resolve_touchid_paths());
    let update_message = read_update_message_cache();
    let show_update = update_message
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());

    let action = match run_home_menu(HomeMenuRunOpts {
        cfg: HomeMenuConfig {
            touchid_configured,
            show_update,
        },
        update_message,
    }) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("vole: {e}");
            return 1;
        }
    };

    match action {
        HomeAction::Launch(cmd) => match exec_self(cmd.argv()) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("vole: {e}");
                1
            }
        },
        HomeAction::ShowHelp => print_help_and_exit(),
        HomeAction::ShowVersion => print_version_and_exit(),
        HomeAction::Quit => 0,
    }
}

fn print_help_and_exit() -> i32 {
    let _ = io::stdout().write_all(b"\x1b[2J\x1b[H");
    if let Err(e) = crate::write_full_help(&mut io::stdout()) {
        eprintln!("vole: {e}");
        return 1;
    }
    0
}

fn print_version_and_exit() -> i32 {
    let _ = io::stdout().write_all(b"\x1b[2J\x1b[H");
    println!("vole {}", env!("CARGO_PKG_VERSION"));
    0
}

/// Replace this process with another `vole` invocation (`args` are argv after the binary).
/// On Unix this does not return on success. Empty `args` opens the home menu.
#[cfg(unix)]
pub(crate) fn exec_self(args: &[&str]) -> io::Result<i32> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe()?;
    let err = Command::new(&exe).args(args).exec();
    Err(io::Error::other(format!("exec {}: {err}", args.join(" "))))
}

#[cfg(not(unix))]
pub(crate) fn exec_self(args: &[&str]) -> io::Result<i32> {
    let exe = std::env::current_exe()?;
    let status = Command::new(&exe).args(args).status()?;
    Ok(status.code().unwrap_or(1))
}

/// Reopen the bare `vole` home menu (status / paginated-select `B Back`).
pub(crate) fn exit_to_home() -> ! {
    match exec_self(&[]) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("vole: {e}");
            std::process::exit(1);
        }
    }
}
