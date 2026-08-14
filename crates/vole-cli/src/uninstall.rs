//! `vole uninstall` plan / apply / TTY interactive 接线。

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::thread;

use crossbeam_channel::unbounded;
use vole_core::delete::{measure_path_size_kb, PathSizeKb};
use vole_core::mutex::{try_lock_uninstall, MutexError};
use vole_core::ops::{
    apply_uninstall_plan, build_uninstall_plan, build_uninstall_plan_for_apps,
    coverage_with_apply_permission_hint, default_applications_dirs, report_has_permission_skips,
    scan_applications, UninstallApplyError, UninstallApplyOptions, UninstallPlanOptions,
    APPLY_PERMISSION_WARN,
};
use vole_core::protection::{
    official_uninstaller_vendor, should_protect_from_uninstall, AppIdentity, AppProtection,
    ProtectionCatalog,
};
use vole_core::units;
use vole_core::vole_proto::{Plan as ProtoPlan, Report, StreamEvent, SCHEMA_VERSION};

use crate::signals;
use crate::tui::{run_paginated_select, MenuItem, MenuState, SelectOutcome};

pub struct UninstallOptions {
    /// `--plan`（隐藏别名 `--dry-run` / `-n`）：强制走自动化 plan 路径。
    pub explicit_plan: bool,
    pub json: bool,
    pub json_stream: bool,
    pub plan_out: Option<PathBuf>,
    pub apply_plan: Option<PathBuf>,
    pub permanent: bool,
    pub target: Option<String>,
}

pub fn run_uninstall(opts: UninstallOptions) -> i32 {
    match run_uninstall_inner(opts) {
        Ok(()) => 0,
        Err(e) if e.kind() == io::ErrorKind::Interrupted => 130,
        Err(e) => {
            eprintln!("vole uninstall: {e}");
            1
        }
    }
}

fn run_uninstall_inner(opts: UninstallOptions) -> io::Result<()> {
    let _lock = try_lock_uninstall().map_err(map_mutex_error)?;

    if let Some(ref plan_path) = opts.apply_plan {
        return run_apply(&opts, plan_path);
    }
    if gate_interactive(io::stdin().is_terminal(), io::stdout().is_terminal(), &opts) {
        return run_interactive(&opts);
    }
    run_plan(opts)
}

/// TTY 裸调用进入交互多选的门控（可单测，不依赖真实 TTY）。
pub(crate) fn gate_interactive(stdin_tty: bool, stdout_tty: bool, opts: &UninstallOptions) -> bool {
    stdin_tty
        && stdout_tty
        && !opts.explicit_plan
        && !opts.json
        && !opts.json_stream
        && opts.plan_out.is_none()
        && opts.apply_plan.is_none()
        && opts.target.is_none()
}

fn run_interactive(opts: &UninstallOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let apps_dirs = applications_dirs_from_env(&home);
    let catalog = ProtectionCatalog::embedded();
    let protection = AppProtection::new();

    // Mole-style stderr spinner while scanning apps and measuring sizes (before the menu).
    let spinner = crate::tty_spinner::TtySpinner::start("Scanning applications...");
    let scanned = scan_applications(&apps_dirs).map_err(|e| io::Error::other(e.to_string()))?;
    let candidates: Vec<AppIdentity> = scanned
        .into_iter()
        .filter(|app| {
            if should_protect_from_uninstall(&app.bundle_id, &catalog) {
                return false;
            }
            if official_uninstaller_vendor(
                &app.bundle_id,
                &app.display_name,
                &app.app_path.display().to_string(),
                &catalog,
            )
            .is_some()
            {
                return false;
            }
            true
        })
        .collect();

    if candidates.is_empty() {
        spinner.stop();
        eprintln!("No removable apps found.");
        return Ok(());
    }

    // Measure sizes once while the spinner is still running; reuse across menu retries.
    let size_kb_by_idx: Vec<Option<u64>> = candidates
        .iter()
        .map(|app| {
            let path_str = app.app_path.display().to_string();
            match measure_path_size_kb(&path_str) {
                PathSizeKb::Known(kb) => Some(kb),
                PathSizeKb::Unknown => None,
            }
        })
        .collect();
    spinner.stop();

    let selected_apps = loop {
        let items: Vec<MenuItem> = candidates
            .iter()
            .zip(size_kb_by_idx.iter())
            .map(|(app, &size_kb)| MenuItem {
                label: app.display_name.clone(),
                filter_name: Some(app.display_name.clone()),
                epoch: None,
                size_kb,
            })
            .collect();

        let mut cfg = MenuState::config_from_env();
        cfg.ignore_initial_enter = true;
        if let Ok((_, rows)) = crossterm::terminal::size() {
            cfg.term_height = rows;
        }

        match run_paginated_select("Select Apps to Remove", items, cfg)? {
            SelectOutcome::Cancelled => return Ok(()),
            SelectOutcome::Back => crate::interactive::exit_to_home(),
            SelectOutcome::Confirmed(idxs) if idxs.is_empty() => {
                eprintln!("No apps selected");
                continue;
            }
            SelectOutcome::Confirmed(idxs) => {
                break idxs
                    .into_iter()
                    .map(|i| candidates[i].clone())
                    .collect::<Vec<_>>();
            }
        }
    };

    eprintln!("Selected {} app(s) for removal:", selected_apps.len());
    for app in &selected_apps {
        eprintln!("  - {} ({})", app.display_name, app.app_path.display());
    }
    eprint!("Proceed with uninstallation? [y/N] ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let answer = line.trim();
    if !answer.eq_ignore_ascii_case("y") {
        eprintln!("Aborted.");
        return Ok(());
    }

    let plan_opts = UninstallPlanOptions {
        applications_dirs: &apps_dirs,
        home: &home,
        target_bundle_or_name: None,
        ttl_secs: 900,
    };
    let plan = build_uninstall_plan_for_apps(&catalog, &protection, &plan_opts, &selected_apps)
        .map_err(|e| io::Error::other(e.to_string()))?;

    if plan.entries.is_empty() {
        eprintln!("Nothing to uninstall for the selected apps.");
        return Ok(());
    }

    let apply_opts = UninstallApplyOptions {
        permanent: opts.permanent,
    };
    let report =
        apply_uninstall_plan(&plan, &protection, apply_opts, None).map_err(map_apply_error)?;
    print_human_report(&report);
    Ok(())
}

fn run_plan(opts: UninstallOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let apps_dirs = applications_dirs_from_env(&home);
    let catalog = ProtectionCatalog::embedded();
    let protection = AppProtection::new();
    let plan_opts = UninstallPlanOptions {
        applications_dirs: &apps_dirs,
        home: &home,
        target_bundle_or_name: opts.target.as_deref(),
        ttl_secs: 900,
    };

    let cancel = vole_core::cancel::CancelToken::new();
    signals::spawn_signal_cancel(cancel);

    let stream_tx = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let _ = event_tx.send(StreamEvent::Progress {
            scanned: 0,
            current: "scanning applications".into(),
        });
        Some((event_tx, writer))
    } else {
        None
    };

    let plan = build_uninstall_plan(&catalog, &protection, &plan_opts)
        .map_err(|e| io::Error::other(e.to_string()))?;

    if let Some((event_tx, writer)) = stream_tx {
        let _ = event_tx.send(StreamEvent::Done {
            report: Report {
                coverage_note: plan.coverage_note.clone(),
                ..Report::default()
            },
        });
        drop(event_tx);
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
    }

    write_plan_output(&opts, &plan)?;
    Ok(())
}

fn run_apply(opts: &UninstallOptions, plan_path: &PathBuf) -> io::Result<()> {
    let json = std::fs::read_to_string(plan_path)?;
    let plan: ProtoPlan = serde_json::from_str(&json).map_err(io::Error::other)?;
    if plan.schema_version != SCHEMA_VERSION {
        return Err(io::Error::other(format!(
            "unsupported plan schema version {}",
            plan.schema_version
        )));
    }

    let protection = AppProtection::new();
    let apply_opts = UninstallApplyOptions {
        permanent: opts.permanent,
    };

    let mut report = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let on_event = |event: StreamEvent| {
            let _ = event_tx.send(event);
        };
        let report = apply_uninstall_plan(&plan, &protection, apply_opts, Some(&on_event))
            .map_err(map_apply_error)?;
        drop(event_tx);
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
        report
    } else {
        apply_uninstall_plan(&plan, &protection, apply_opts, None).map_err(map_apply_error)?
    };

    if should_use_json(opts.json) {
        report.coverage_note =
            coverage_with_apply_permission_hint(report.coverage_note.as_deref(), &report);
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(io::Error::other)?
        );
    } else {
        print_human_report(&report);
    }
    Ok(())
}

fn applications_dirs_from_env(home: &Path) -> Vec<PathBuf> {
    if let Ok(raw) = env::var("VOLE_APPLICATIONS_DIR") {
        return raw
            .split(':')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    default_applications_dirs(home)
}

fn write_plan_output(opts: &UninstallOptions, plan: &ProtoPlan) -> io::Result<()> {
    let json = serde_json::to_string_pretty(plan).map_err(io::Error::other)?;
    if let Some(ref path) = opts.plan_out {
        std::fs::write(path, &json)?;
    }
    if should_use_json(opts.json) {
        println!("{json}");
    } else if opts.plan_out.is_none() {
        print_human_plan(plan);
    }
    Ok(())
}

fn print_human_plan(plan: &ProtoPlan) {
    eprintln!(
        "uninstall plan: {} entries (ttl {}s)",
        plan.entries.len(),
        plan.ttl_secs
    );
    for entry in &plan.entries {
        eprintln!(
            "  {}  {}  {}",
            units::bytes_bin(entry.size),
            entry.rule_id,
            entry.path.display()
        );
    }
    if let Some(note) = &plan.coverage_note {
        eprintln!("\n{note}");
    }
}

fn print_human_report(report: &Report) {
    eprintln!(
        "uninstall apply: succeeded={} skipped={} failed={} trashed={} deleted={}",
        report.succeeded,
        report.skipped,
        report.failed,
        units::bytes_bin(report.trashed_bytes),
        units::bytes_bin(report.deleted_bytes)
    );
    if let Some(note) = &report.coverage_note {
        eprintln!("{note}");
    }
    if report_has_permission_skips(report) {
        eprintln!("{APPLY_PERMISSION_WARN}");
    }
}

fn should_use_json(force: bool) -> bool {
    force || !io::stdout().is_terminal()
}

fn spawn_stream_writer(
    event_rx: crossbeam_channel::Receiver<StreamEvent>,
) -> io::Result<thread::JoinHandle<io::Result<()>>> {
    thread::Builder::new()
        .name("vole-uninstall-stream".into())
        .spawn(move || {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            while let Ok(event) = event_rx.recv() {
                let value = event.with_schema(SCHEMA_VERSION);
                let line = serde_json::to_string(&value).map_err(io::Error::other)?;
                out.write_all(line.as_bytes())?;
                out.write_all(b"\n")?;
                out.flush()?;
            }
            Ok(())
        })
        .map_err(io::Error::other)
}

fn map_mutex_error(e: MutexError) -> io::Error {
    match e {
        MutexError::AlreadyRunning => io::Error::other("another vole uninstall is running"),
        other => io::Error::other(other.to_string()),
    }
}

fn map_apply_error(e: UninstallApplyError) -> io::Error {
    io::Error::other(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare_opts() -> UninstallOptions {
        UninstallOptions {
            explicit_plan: false,
            json: false,
            json_stream: false,
            plan_out: None,
            apply_plan: None,
            permanent: false,
            target: None,
        }
    }

    #[test]
    fn interactive_gate_requires_bare_tty_flags() {
        let bare = bare_opts();
        assert!(!gate_interactive(false, false, &bare));
        assert!(gate_interactive(true, true, &bare));
        assert!(!gate_interactive(
            true,
            true,
            &UninstallOptions {
                explicit_plan: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &UninstallOptions {
                json: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &UninstallOptions {
                target: Some("Foo".into()),
                ..bare_opts()
            }
        ));
    }
}
