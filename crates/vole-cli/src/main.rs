//! vole 命令行入口。
#![forbid(unsafe_code)]

mod clean;
mod history_cmd;
mod installer;
mod interactive;
mod optimize;
mod purge;
mod remove;
mod signals;
mod terminal;
mod touchid;
mod tty_spinner;
mod tui;
mod uninstall;
mod update;
mod update_banner;

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use vole_core::analyze::{analyze_directory, analyze_directory_with_progress};
use vole_core::cancel::CancelToken;
use vole_core::status::{CollectionMode, StatusCollector, REFRESH_INTERVAL};
use vole_core::vole_proto::{AnalyzeEntry, AnalyzeOutput};

#[derive(Parser)]
#[command(
    name = "vole",
    version,
    about = "macOS cleanup and monitoring",
    after_help = "Notes:\n  - Run `vole` with no subcommand in a terminal to open the home menu.\n  - Bare TTY `clean` / `optimize`: scan a plan, prompt Proceed? [y/N], then apply.\n\nDetailed Usage and Options for each command follow."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

pub(crate) fn clap_command() -> clap::Command {
    Cli::command()
}

/// Top-level overview plus each subcommand's long help (Usage + Options).
pub(crate) fn write_full_help<W: Write>(w: &mut W) -> io::Result<()> {
    let mut root = clap_command();
    root.write_long_help(&mut *w)?;
    let subs: Vec<clap::Command> = root
        .get_subcommands()
        .filter(|c| c.get_name() != "help")
        .cloned()
        .collect();
    for mut sub in subs {
        let name = sub.get_name().to_string();
        writeln!(&mut *w)?;
        writeln!(&mut *w, "────────────────────────────────────────────────────────")?;
        writeln!(&mut *w, "  vole {name}")?;
        writeln!(&mut *w, "────────────────────────────────────────────────────────")?;
        sub = sub.bin_name(format!("vole {name}"));
        sub.write_long_help(&mut *w)?;
    }
    Ok(())
}

fn wants_full_help(args: &[String]) -> bool {
    matches!(
        args,
        [h] if h == "-h" || h == "--help" || h == "help"
    )
}

#[derive(Subcommand)]
enum Command {
    /// Clean caches and leftover files.
    ///
    /// On a TTY with no flags: scan a plan, confirm, then apply.
    /// With `--plan` / `--json`, or when not a TTY: emit a plan only.
    Clean {
        /// Emit candidates only; do not delete (default when not a TTY; on a TTY skips confirm).
        #[arg(long, conflicts_with = "apply")]
        plan: bool,
        /// Same as `--plan`.
        #[arg(long = "dry-run", short = 'n', conflicts_with = "apply")]
        dry_run: bool,
        /// Apply entries from a plan file (TTL + TOCTOU revalidation required).
        #[arg(long, value_name = "PLAN", conflicts_with_all = ["dry_run", "plan_out"])]
        apply: Option<PathBuf>,
        /// Permanently delete instead of moving to Trash (`--apply` or after interactive confirm).
        #[arg(long)]
        permanent: bool,
        /// Print JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
        /// Stream NDJSON events to stdout.
        #[arg(long = "json-stream")]
        json_stream: bool,
        /// Write plan JSON to a file.
        #[arg(long, conflicts_with = "apply")]
        plan_out: Option<PathBuf>,
        /// TTY paginated multi-select for the protected-cache whitelist; scripts use --whitelist-*.
        #[arg(
            long,
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent"]
        )]
        whitelist: bool,
        /// Add a path pattern to the whitelist (non-interactive).
        #[arg(
            long = "whitelist-add",
            value_name = "PATH",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist"]
        )]
        whitelist_add: Option<String>,
        /// Remove a path pattern from the whitelist (non-interactive).
        #[arg(
            long = "whitelist-remove",
            value_name = "PATH",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist"]
        )]
        whitelist_remove: Option<String>,
        /// List the current whitelist (non-interactive).
        #[arg(
            long = "whitelist-list",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist"]
        )]
        whitelist_list: bool,
    },
    /// Uninstall apps and their user-domain leftovers.
    ///
    /// On a TTY with no flags: paginated select, confirm, then uninstall.
    /// With `--plan` / `--json`, or when not a TTY: emit a plan only.
    Uninstall {
        /// Emit candidates only; do not delete (default when not a TTY; on a TTY skips interactive UI).
        #[arg(long, conflicts_with = "apply")]
        plan: bool,
        /// Same as `--plan`.
        #[arg(long = "dry-run", short = 'n', conflicts_with = "apply")]
        dry_run: bool,
        /// Apply entries from a plan file.
        #[arg(long, value_name = "PLAN", conflicts_with_all = ["dry_run", "plan_out"])]
        apply: Option<PathBuf>,
        /// Permanently delete instead of moving to Trash (`--apply` or interactive uninstall).
        #[arg(long)]
        permanent: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Stream NDJSON events.
        #[arg(long = "json-stream")]
        json_stream: bool,
        /// Write plan JSON to a file.
        #[arg(long, conflicts_with = "apply")]
        plan_out: Option<PathBuf>,
        /// Optional filter by bundle id or app name.
        target: Option<String>,
    },
    /// Run system optimization tasks.
    ///
    /// Privileged DNS uses `sudo -n`; other gaps are noted in `coverage_note`.
    /// On a TTY with no flags: scan a plan, confirm, then apply.
    /// With `--plan` / `--json`, or when not a TTY: emit a plan only.
    #[command(visible_alias = "optimise")]
    Optimize {
        /// Emit candidates only; do not change the system (default when not a TTY; on a TTY skips confirm).
        #[arg(long, conflicts_with = "apply")]
        plan: bool,
        /// Same as `--plan`.
        #[arg(long = "dry-run", short = 'n', conflicts_with = "apply")]
        dry_run: bool,
        /// Apply entries from a plan file.
        #[arg(long, value_name = "PLAN", conflicts_with_all = ["dry_run", "plan_out"])]
        apply: Option<PathBuf>,
        /// Permanently delete instead of Trash (`--apply` or after confirm; delete-class entries only).
        #[arg(long)]
        permanent: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Stream NDJSON events.
        #[arg(long = "json-stream")]
        json_stream: bool,
        /// Write plan JSON to a file.
        #[arg(long, conflicts_with = "apply")]
        plan_out: Option<PathBuf>,
        /// Optional: run a single optimize task id (experimental).
        #[arg(long, value_name = "TASK_ID")]
        task: Option<String>,
        /// TTY paginated multi-select for the optimize-task whitelist; scripts use --whitelist-*.
        #[arg(
            long,
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "plan", "dry_run", "task"]
        )]
        whitelist: bool,
        /// Add a task id to the optimize whitelist (non-interactive).
        #[arg(
            long = "whitelist-add",
            value_name = "TASK_ID",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist", "plan", "dry_run", "task"]
        )]
        whitelist_add: Option<String>,
        /// Remove a task id from the optimize whitelist (non-interactive).
        #[arg(
            long = "whitelist-remove",
            value_name = "TASK_ID",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist", "plan", "dry_run", "task"]
        )]
        whitelist_remove: Option<String>,
        /// List the current optimize-task whitelist (non-interactive).
        #[arg(
            long = "whitelist-list",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist", "plan", "dry_run", "task"]
        )]
        whitelist_list: bool,
    },
    /// Live system health monitor.
    Status {
        /// Print JSON instead of the TUI.
        #[arg(long)]
        json: bool,
        /// Continuous NDJSON stream (watch-style).
        #[arg(long = "json-stream")]
        json_stream: bool,
    },
    /// Analyze directory disk usage.
    #[command(visible_alias = "analyse")]
    Analyze {
        /// Target directory (default: `$HOME`).
        path: Option<PathBuf>,
        /// Print JSON instead of the TUI.
        #[arg(long)]
        json: bool,
    },
    /// Review operation history and deletion audit.
    History {
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Max sessions / deletions to show (1..=200, default 20).
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Remove stale project build artifacts.
    ///
    /// On a TTY with no flags: paginated select, confirm, then purge.
    /// With `--plan` / `--json`, or when not a TTY: emit a plan only.
    Purge {
        /// Emit candidates only; do not delete (default when not a TTY; on a TTY skips interactive UI).
        #[arg(long, conflicts_with = "apply")]
        plan: bool,
        /// Same as `--plan`.
        #[arg(long = "dry-run", short = 'n', conflicts_with = "apply")]
        dry_run: bool,
        /// Apply entries from a plan file.
        #[arg(long, value_name = "PLAN", conflicts_with_all = ["dry_run", "plan_out"])]
        apply: Option<PathBuf>,
        /// Permanently delete instead of moving to Trash (`--apply` or interactive purge).
        #[arg(long)]
        permanent: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Stream NDJSON events.
        #[arg(long = "json-stream")]
        json_stream: bool,
        /// Write plan JSON to a file.
        #[arg(long, conflicts_with = "apply")]
        plan_out: Option<PathBuf>,
        /// Include zero-size artifact directories.
        #[arg(long = "include-empty")]
        include_empty: bool,
    },
    /// Find and remove installer packages.
    ///
    /// On a TTY with no flags: paginated select, confirm, then clean up.
    /// With `--plan` / `--json`, or when not a TTY: emit a plan only.
    Installer {
        /// Emit candidates only; do not delete (default when not a TTY; on a TTY skips interactive UI).
        #[arg(long, conflicts_with = "apply")]
        plan: bool,
        /// Same as `--plan`.
        #[arg(long = "dry-run", short = 'n', conflicts_with = "apply")]
        dry_run: bool,
        /// Apply entries from a plan file.
        #[arg(long, value_name = "PLAN", conflicts_with_all = ["dry_run", "plan_out"])]
        apply: Option<PathBuf>,
        /// Permanently delete instead of moving to Trash (`--apply` or interactive cleanup).
        #[arg(long)]
        permanent: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Stream NDJSON events.
        #[arg(long = "json-stream")]
        json_stream: bool,
        /// Write plan JSON to a file.
        #[arg(long, conflicts_with = "apply")]
        plan_out: Option<PathBuf>,
    },
    /// Configure Touch ID for sudo.
    Touchid {
        /// `status` | `enable` | `disable`; omit for interactive toggle.
        action: Option<String>,
        /// Preview PAM changes without writing files.
        #[arg(long)]
        plan: bool,
        /// Same as `--plan`.
        #[arg(long = "dry-run", short = 'n')]
        dry_run: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Self-update (check → download → verify → install).
    Update {
        /// Force reinstall; for Homebrew installs also bypasses the “prefer brew” gate.
        #[arg(long, short = 'f')]
        force: bool,
        /// Install latest nightly from `main` (rejected for Homebrew installs).
        #[arg(long)]
        nightly: bool,
        /// Check for updates only; do not download or install.
        #[arg(long)]
        check: bool,
        /// Non-interactive confirm (including Homebrew self-update confirm).
        #[arg(long, short = 'y')]
        yes: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Uninstall vole (binaries and local config).
    Remove {
        /// Preview items to delete; do not delete.
        #[arg(long = "dry-run", short = 'n')]
        dry_run: bool,
        /// Skip interactive confirmation.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Also delete the operation audit log (kept by default).
        #[arg(long = "purge-oplog")]
        purge_oplog: bool,
    },
    /// Generate a shell completion script to stdout.
    #[command(visible_alias = "completion")]
    Completions {
        /// Target shell: bash / zsh / fish / elvish / powershell
        shell: CompletionShell,
    },
}

#[derive(Clone, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
}

impl From<CompletionShell> for Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Zsh => Shell::Zsh,
            CompletionShell::Fish => Shell::Fish,
            CompletionShell::Elvish => Shell::Elvish,
            CompletionShell::Powershell => Shell::PowerShell,
        }
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if wants_full_help(&argv) {
        if let Err(e) = write_full_help(&mut io::stdout()) {
            eprintln!("vole: {e}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    let cli = Cli::parse();
    match cli.command {
        None => std::process::exit(interactive::run()),
        Some(Command::Clean {
            plan,
            dry_run,
            apply,
            permanent,
            json,
            json_stream,
            plan_out,
            whitelist,
            whitelist_add,
            whitelist_remove,
            whitelist_list,
        }) => {
            let code = clean::run_clean(clean::CleanOptions {
                explicit_plan: plan || dry_run,
                json,
                json_stream,
                plan_out,
                apply_plan: apply,
                permanent,
                whitelist,
                whitelist_add,
                whitelist_remove,
                whitelist_list,
            });
            std::process::exit(code);
        }
        Some(Command::Uninstall {
            plan,
            dry_run,
            apply,
            permanent,
            json,
            json_stream,
            plan_out,
            target,
        }) => {
            let code = uninstall::run_uninstall(uninstall::UninstallOptions {
                explicit_plan: plan || dry_run,
                json,
                json_stream,
                plan_out,
                apply_plan: apply,
                permanent,
                target,
            });
            std::process::exit(code);
        }
        Some(Command::Optimize {
            plan,
            dry_run,
            apply,
            permanent,
            json,
            json_stream,
            plan_out,
            task,
            whitelist,
            whitelist_add,
            whitelist_remove,
            whitelist_list,
        }) => {
            let code = optimize::run_optimize(optimize::OptimizeOptions {
                explicit_plan: plan || dry_run,
                json,
                json_stream,
                plan_out,
                apply_plan: apply,
                permanent,
                task,
                whitelist,
                whitelist_add,
                whitelist_remove,
                whitelist_list,
            });
            std::process::exit(code);
        }
        Some(Command::Status { json, json_stream }) => {
            if let Err(e) = cmd_status(json, json_stream) {
                eprintln!("vole status: {}", e);
                std::process::exit(1);
            }
        }
        Some(Command::Analyze { path, json }) => {
            if let Err(e) = cmd_analyze(path, json) {
                eprintln!("vole analyze: {}", e);
                std::process::exit(1);
            }
        }
        Some(Command::History { json, limit }) => {
            std::process::exit(history_cmd::run(json, limit));
        }
        Some(Command::Purge {
            plan,
            dry_run,
            apply,
            permanent,
            json,
            json_stream,
            plan_out,
            include_empty,
        }) => {
            let code = purge::run_purge(purge::PurgeOptions {
                explicit_plan: plan || dry_run,
                json,
                json_stream,
                plan_out,
                apply_plan: apply,
                permanent,
                include_empty,
            });
            std::process::exit(code);
        }
        Some(Command::Installer {
            plan,
            dry_run,
            apply,
            permanent,
            json,
            json_stream,
            plan_out,
        }) => {
            let code = installer::run_installer(installer::InstallerOptions {
                explicit_plan: plan || dry_run,
                json,
                json_stream,
                plan_out,
                apply_plan: apply,
                permanent,
            });
            std::process::exit(code);
        }
        Some(Command::Touchid {
            action,
            plan,
            dry_run,
            json,
        }) => {
            let code = touchid::run_touchid(touchid::TouchidOptions {
                action,
                dry_run: dry_run || plan,
                plan,
                json,
            });
            std::process::exit(code);
        }
        Some(Command::Update {
            force,
            nightly,
            check,
            yes,
            json,
        }) => {
            let code = update::run_update_cli(update::UpdateCliOptions {
                force,
                nightly,
                check,
                yes,
                json,
            });
            std::process::exit(code);
        }
        Some(Command::Remove {
            dry_run,
            yes,
            json,
            purge_oplog,
        }) => {
            let code = remove::run_remove_cli(remove::RemoveCliOptions {
                dry_run,
                yes,
                json,
                purge_oplog,
            });
            std::process::exit(code);
        }
        Some(Command::Completions { shell }) => {
            let mut cmd = clap_command();
            generate(Shell::from(shell), &mut cmd, "vole", &mut io::stdout());
        }
    }
}

fn should_use_json(force: bool) -> bool {
    if force {
        return true;
    }
    !io::stdout().is_terminal()
}

pub(crate) fn cmd_status(force_json: bool, json_stream: bool) -> io::Result<()> {
    if json_stream {
        return cmd_status_stream();
    }
    if should_use_json(force_json) {
        let mut collector = StatusCollector::new();
        let snap = collector.collect_full().map_err(io::Error::other)?;
        let out = serde_json::to_string(&snap).map_err(io::Error::other)?;
        println!("{}", out);
        return Ok(());
    }
    cmd_status_tui()
}

fn cmd_status_stream() -> io::Result<()> {
    let cancel = CancelToken::new();
    signals::spawn_signal_cancel(cancel.clone());
    let mut collector = StatusCollector::new();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    while !cancel.is_cancelled() {
        let snap = collector
            .collect(CollectionMode::Fast)
            .map_err(io::Error::other)?;
        let line = serde_json::to_string(&snap).map_err(io::Error::other)?;
        out.write_all(line.as_bytes())?;
        out.write_all(b"\n")?;
        out.flush()?;
        if cancel.is_cancelled() {
            break;
        }
        std::thread::sleep(REFRESH_INTERVAL);
    }
    Ok(())
}

fn resolve_analyze_path(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

fn cmd_analyze(path: Option<PathBuf>, force_json: bool) -> io::Result<()> {
    let root = resolve_analyze_path(path);
    let cancel = CancelToken::new();
    signals::spawn_signal_cancel(cancel.clone());
    if should_use_json(force_json) {
        let out = analyze_directory(&root, &cancel).map_err(map_scan_cancel)?;
        let json = serde_json::to_string(&out).map_err(io::Error::other)?;
        println!("{}", json);
        return Ok(());
    }
    cmd_analyze_tui(&root, cancel)
}

fn map_scan_cancel(err: io::Error) -> io::Error {
    if err.kind() == io::ErrorKind::Interrupted {
        std::process::exit(130);
    }
    err
}

enum AnalyzeScanMsg {
    Child(AnalyzeEntry),
    Done(AnalyzeOutput),
}

fn cmd_analyze_tui(initial: &Path, cancel: CancelToken) -> io::Result<()> {
    terminal::install_panic_hook();
    let mut guard = terminal::TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut term = Terminal::new(backend)?;
    let theme = tui::DesignSystem::resolve().theme;

    let local_snapshots_tip =
        vole_core::localsnapshots::to_info(vole_core::localsnapshots::probe_local_snapshots(
            &vole_core::localsnapshots::LiveLocalSnapshotDeps,
        ))
        .map(|info| info.message);

    let mut stack: Vec<PathBuf> = vec![initial.to_path_buf()];
    let mut state = tui::AnalyzeState::default();
    let mut out = AnalyzeOutput::default();
    let mut scanning = true;
    let mut scan_rx: Option<std::sync::mpsc::Receiver<io::Result<AnalyzeScanMsg>>> = None;
    let mut pending_delete: Vec<String> = Vec::new();

    let poll = Duration::from_millis(33);

    loop {
        if scanning && scan_rx.is_none() {
            let path = stack.last().cloned().unwrap();
            let mode = state.live_sort_mode;
            out = AnalyzeOutput {
                path: path.to_string_lossy().into_owned(),
                ..AnalyzeOutput::default()
            };
            state = tui::AnalyzeState {
                live_sort_mode: mode,
                status: state.status.clone(),
                ..tui::AnalyzeState::default()
            };
            state.begin_live_scan();
            let cancel_scan = cancel.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let tx_child = tx.clone();
                let result = analyze_directory_with_progress(&path, &cancel_scan, |child| {
                    let _ = tx_child.send(Ok(AnalyzeScanMsg::Child(child)));
                });
                match result {
                    Ok(snapshot) => {
                        let _ = tx.send(Ok(AnalyzeScanMsg::Done(snapshot)));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                    }
                }
            });
            scan_rx = Some(rx);
        }

        let can_go_back = stack.len() > 1;
        term.draw(|f| {
            tui::render_analyze(
                f,
                &out,
                &theme,
                &tui::AnalyzeRenderOpts {
                    selected: state.selected,
                    scanning,
                    local_snapshots_tip: local_snapshots_tip.as_deref(),
                    can_go_back,
                    show_large_files: state.show_large_files,
                    multi_selected: &state.multi_selected,
                    large_multi_selected: &state.large_multi_selected,
                    footer_mode: state.footer_mode(&out, can_go_back),
                    status: &state.status,
                    entry_filter: &state.entry_filter,
                    large_filter: &state.large_filter,
                },
            )
        })?;

        if let Some(rx) = &scan_rx {
            while let Ok(result) = rx.try_recv() {
                match result {
                    Ok(AnalyzeScanMsg::Child(child)) => {
                        tui::upsert_live_child(&mut out, child);
                        state.apply_live_sort_after_progress(&mut out);
                        state.clamp_selection(&out);
                    }
                    Ok(AnalyzeScanMsg::Done(snapshot)) => {
                        scan_rx = None;
                        let pin = state.take_live_scan_pin_first();
                        let keep_path = if !pin {
                            state
                                .visible_entries(&out)
                                .get(state.selected)
                                .map(|e| e.path.clone())
                        } else {
                            None
                        };
                        let mode = state.live_sort_mode;
                        out = snapshot;
                        state = tui::AnalyzeState {
                            live_sort_mode: mode,
                            ..tui::AnalyzeState::default()
                        };
                        if pin {
                            state.selected = 0;
                        } else if let Some(path) = keep_path {
                            if let Some(i) = out.entries.iter().position(|e| e.path == path) {
                                state.selected = i;
                            }
                        }
                        scanning = false;
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                        scan_rx = None;
                        cancel.cancel();
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        if event::poll(poll)? {
            if let Event::Key(key) = event::read()? {
                let filtering = state.entry_filtering || state.large_filtering;
                let Some(ak) = tui::map_analyze_key(key, filtering) else {
                    continue;
                };
                let effect = state.handle_key(ak, &out, scanning, can_go_back);
                if scanning && state.auto_sort_live {
                    state.apply_live_sort_after_progress(&mut out);
                }
                match effect {
                    tui::AnalyzeEffect::Quit => cancel.cancel(),
                    tui::AnalyzeEffect::GoBack => {
                        stack.pop();
                        scanning = true;
                        scan_rx = None;
                        let mode = state.live_sort_mode;
                        state = tui::AnalyzeState {
                            live_sort_mode: mode,
                            ..tui::AnalyzeState::default()
                        };
                    }
                    tui::AnalyzeEffect::EnterDir(path) => {
                        stack.push(PathBuf::from(path));
                        scanning = true;
                        scan_rx = None;
                        let mode = state.live_sort_mode;
                        state = tui::AnalyzeState {
                            live_sort_mode: mode,
                            ..tui::AnalyzeState::default()
                        };
                    }
                    tui::AnalyzeEffect::RequestDelete(paths) => {
                        pending_delete = paths;
                        let n = pending_delete.len();
                        let label = if n == 1 {
                            PathBuf::from(&pending_delete[0])
                                .file_name()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| pending_delete[0].clone())
                        } else {
                            format!("{n} items")
                        };
                        state.status =
                            format!("Delete: {label}  Press Enter to confirm  |  ESC cancel");
                    }
                    tui::AnalyzeEffect::ConfirmDelete => {
                        let report = tui::trash_analyze_paths(&pending_delete);
                        tui::apply_removals(&mut out, &report.removed);
                        state.multi_selected.clear();
                        state.large_multi_selected.clear();
                        state.clamp_selection(&out);
                        pending_delete.clear();
                        if report.errors.is_empty() {
                            state.status = format!("Deleted {}", report.removed.len());
                        } else {
                            state.status = format!(
                                "Deleted {}; errors: {}",
                                report.removed.len(),
                                report.errors.join("; ")
                            );
                        }
                    }
                    tui::AnalyzeEffect::CancelDelete => {
                        pending_delete.clear();
                    }
                    tui::AnalyzeEffect::Open(paths) => {
                        let n = paths.len();
                        for p in paths {
                            let argv = tui::open_argv(&p);
                            if let Err(e) = tui::spawn_detached(&argv) {
                                state.status = format!("Open failed: {e}");
                            }
                        }
                        if state.status.is_empty() {
                            state.status = if n == 1 {
                                "Opening…".into()
                            } else {
                                format!("Opening {n} items…")
                            };
                        }
                    }
                    tui::AnalyzeEffect::Preview(path) => {
                        let is_dir = out
                            .entries
                            .iter()
                            .find(|e| e.path == path)
                            .map(|e| e.is_dir)
                            .unwrap_or(false);
                        if let Some(argv) = tui::preview_target(&path, is_dir) {
                            if let Err(e) = tui::spawn_detached(&argv) {
                                state.status = format!("Preview failed: {e}");
                            } else {
                                state.status = "Previewing…".into();
                            }
                        }
                    }
                    tui::AnalyzeEffect::Reveal(paths) => {
                        let n = paths.len();
                        for p in paths {
                            let argv = tui::reveal_argv(&p);
                            if let Err(e) = tui::spawn_detached(&argv) {
                                state.status = format!("Reveal failed: {e}");
                            }
                        }
                        if state.status.is_empty() {
                            state.status = if n == 1 {
                                "Showing in Finder…".into()
                            } else {
                                format!("Showing {n} items in Finder…")
                            };
                        }
                    }
                    tui::AnalyzeEffect::Refresh => {
                        scanning = true;
                        scan_rx = None;
                        let mode = state.live_sort_mode;
                        state = tui::AnalyzeState {
                            live_sort_mode: mode,
                            status: "Refreshing...".into(),
                            ..tui::AnalyzeState::default()
                        };
                    }
                    tui::AnalyzeEffect::None => {}
                }
            }
        }

        if cancel.is_cancelled() {
            break;
        }
    }

    term.show_cursor()?;
    guard.restore();
    if cancel.is_cancelled() {
        std::process::exit(130);
    }
    Ok(())
}

fn cmd_status_tui() -> io::Result<()> {
    if std::env::var_os("VOLE_STATUS_PANIC") == Some("1".into()) {
        panic!("vole status panic test");
    }

    terminal::install_panic_hook();
    let mut guard = terminal::TerminalGuard::enter()?;
    let cancel = CancelToken::new();
    signals::spawn_signal_cancel(cancel.clone());

    let backend = CrosstermBackend::new(io::stdout());
    let mut term = Terminal::new(backend)?;
    let theme = tui::DesignSystem::resolve().theme;

    let mut collector = StatusCollector::new();
    let mut snap = collector.collect_full().map_err(io::Error::other)?;
    let prefs = tui::load_status_prefs();
    let mut cat_hidden = prefs.cat_hidden;
    let mut cpu_cores = prefs.cpu_cores;
    let mut anim_frame: u64 = 0;
    let mut back_home = false;

    let poll = Duration::from_millis(33);
    let mut last_collect = std::time::Instant::now();
    while !cancel.is_cancelled() && !back_home {
        if event::poll(poll)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        cancel.cancel();
                    }
                    KeyCode::Char('q') | KeyCode::Esc => cancel.cancel(),
                    KeyCode::Char('b') | KeyCode::Char('B') => back_home = true,
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        cat_hidden = !cat_hidden;
                        tui::save_cat_hidden(cat_hidden);
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        cpu_cores = tui::next_cpu_cores(cpu_cores);
                        tui::save_cpu_cores(cpu_cores);
                    }
                    _ => {}
                }
            }
        }

        if last_collect.elapsed() >= REFRESH_INTERVAL {
            snap = collector
                .collect(CollectionMode::Fast)
                .map_err(io::Error::other)?;
            last_collect = std::time::Instant::now();
        }

        // Cap step so the front-facing vole slides slowly even under high CPU.
        let step = (1u64 + (snap.cpu.usage / 50.0).floor().max(0.0) as u64).min(2);
        anim_frame = anim_frame.wrapping_add(step);

        let opts = tui::StatusRenderOpts {
            cat_hidden,
            anim_frame,
            cpu_cores,
        };
        term.draw(|f| tui::render_status(f, &snap, &theme, opts))?;

        if cancel.is_cancelled() || back_home {
            break;
        }
    }

    term.show_cursor()?;
    guard.restore();
    if back_home {
        interactive::exit_to_home();
    }
    if cancel.is_cancelled() {
        std::process::exit(130);
    }
    Ok(())
}
