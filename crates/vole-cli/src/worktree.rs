//! `vole worktree` plan / apply / TTY interactive 接线。

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::unbounded;
use vole_core::mutex::{try_lock_worktree, MutexError};
use vole_core::ops::{
    apply_worktree_plan, build_worktree_plan, coverage_with_apply_permission_hint,
    report_has_permission_skips, LiveGitProbe, WorktreeApplyError, WorktreeApplyOptions,
    WorktreePlanOptions, APPLY_PERMISSION_WARN, DEFAULT_WORKTREE_TTL_SECS,
};
use vole_core::protection::AppProtection;
use vole_core::units;
use vole_core::vole_proto::{Plan as ProtoPlan, PlanEntry, Report, StreamEvent, SCHEMA_VERSION};

use crate::signals;
use crate::tui::{run_paginated_select, MenuItem, MenuState, SelectOutcome};

pub struct WorktreeOptions {
    /// `--plan` / `--dry-run` / `-n`：强制走自动化 plan 路径。
    pub explicit_plan: bool,
    pub json: bool,
    pub json_stream: bool,
    pub plan_out: Option<PathBuf>,
    pub apply_plan: Option<PathBuf>,
    pub permanent: bool,
}

pub fn run_worktree(opts: WorktreeOptions) -> i32 {
    match run_worktree_inner(opts) {
        Ok(()) => 0,
        Err(e) if e.kind() == io::ErrorKind::Interrupted => 130,
        Err(e) => {
            eprintln!("vole worktree: {e}");
            1
        }
    }
}

fn run_worktree_inner(opts: WorktreeOptions) -> io::Result<()> {
    let _lock = try_lock_worktree().map_err(map_mutex_error)?;

    if let Some(ref plan_path) = opts.apply_plan {
        return run_apply(&opts, plan_path);
    }
    if gate_interactive(io::stdin().is_terminal(), io::stdout().is_terminal(), &opts) {
        return run_interactive(&opts);
    }
    run_plan(opts)
}

/// TTY 裸调用进入交互多选的门控（可单测，不依赖真实 TTY）。
pub(crate) fn gate_interactive(stdin_tty: bool, stdout_tty: bool, opts: &WorktreeOptions) -> bool {
    stdin_tty
        && stdout_tty
        && !opts.explicit_plan
        && !opts.json
        && !opts.json_stream
        && opts.plan_out.is_none()
        && opts.apply_plan.is_none()
}

fn run_interactive(opts: &WorktreeOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let cwd = env::current_dir()?;
    let protection = AppProtection::new();
    let git = LiveGitProbe;
    let plan_opts = WorktreePlanOptions {
        home: &home,
        cwd: &cwd,
        ttl_secs: DEFAULT_WORKTREE_TTL_SECS,
        search_roots: None,
        now: SystemTime::now(),
        git: &git,
    };
    let plan = build_worktree_plan(&protection, &plan_opts)
        .map_err(|e| io::Error::other(e.to_string()))?;

    if plan.entries.is_empty() {
        eprintln!("No leftover git worktrees found.");
        return Ok(());
    }

    let selected_idxs = loop {
        let items: Vec<MenuItem> = plan.entries.iter().map(menu_item_from_entry).collect();
        let mut cfg = MenuState::config_from_env();
        cfg.ignore_initial_enter = true;
        cfg.preselected = Vec::new();
        if let Ok((_, rows)) = crossterm::terminal::size() {
            cfg.term_height = rows;
        }

        match run_paginated_select("Select Git Worktrees to Remove", items, cfg)? {
            SelectOutcome::Cancelled => return Ok(()),
            SelectOutcome::Back => crate::interactive::exit_to_home(),
            SelectOutcome::Confirmed(idxs) if idxs.is_empty() => {
                eprintln!("No items selected");
                continue;
            }
            SelectOutcome::Confirmed(idxs) => break idxs,
        }
    };

    eprintln!("Selected {} worktree(s) for removal:", selected_idxs.len());
    for &i in &selected_idxs {
        let entry = &plan.entries[i];
        eprintln!("  - {} ({})", entry.label, entry.path.display());
    }
    eprint!("Proceed with worktree removal? [y/N] ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    if !line.trim().eq_ignore_ascii_case("y") {
        eprintln!("Aborted.");
        return Ok(());
    }

    let apply_plan = filter_plan_entries(plan, &selected_idxs);
    if apply_plan.entries.is_empty() {
        eprintln!("Nothing to remove for the selection.");
        return Ok(());
    }

    let apply_opts = WorktreeApplyOptions {
        permanent: opts.permanent,
    };
    let report = apply_worktree_plan(&apply_plan, &protection, apply_opts, &git, None)
        .map_err(map_apply_error)?;
    print_human_report(&report);
    Ok(())
}

fn run_plan(opts: WorktreeOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let cwd = env::current_dir()?;
    let protection = AppProtection::new();
    let git = LiveGitProbe;
    let plan_opts = WorktreePlanOptions {
        home: &home,
        cwd: &cwd,
        ttl_secs: DEFAULT_WORKTREE_TTL_SECS,
        search_roots: None,
        now: SystemTime::now(),
        git: &git,
    };

    let cancel = vole_core::cancel::CancelToken::new();
    signals::spawn_signal_cancel(cancel);

    let stream_tx = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let _ = event_tx.send(StreamEvent::Progress {
            scanned: 0,
            current: "scanning git worktrees".into(),
        });
        Some((event_tx, writer))
    } else {
        None
    };

    let plan = build_worktree_plan(&protection, &plan_opts)
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

fn run_apply(opts: &WorktreeOptions, plan_path: &Path) -> io::Result<()> {
    let json = std::fs::read_to_string(plan_path)?;
    let plan: ProtoPlan = serde_json::from_str(&json).map_err(io::Error::other)?;
    if plan.schema_version != SCHEMA_VERSION {
        return Err(io::Error::other(format!(
            "unsupported plan schema version {}",
            plan.schema_version
        )));
    }

    let protection = AppProtection::new();
    let git = LiveGitProbe;
    let apply_opts = WorktreeApplyOptions {
        permanent: opts.permanent,
    };

    let mut report = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let on_event = |event: StreamEvent| {
            let _ = event_tx.send(event);
        };
        let report = apply_worktree_plan(&plan, &protection, apply_opts, &git, Some(&on_event))
            .map_err(map_apply_error)?;
        drop(event_tx);
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
        report
    } else {
        apply_worktree_plan(&plan, &protection, apply_opts, &git, None).map_err(map_apply_error)?
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

fn menu_item_from_entry(entry: &PlanEntry) -> MenuItem {
    let path_str = entry.path.display().to_string();
    let label = if entry.blockers.is_empty() {
        format!("{}  {}", entry.label, path_str)
    } else {
        format!(
            "{}  [{}]  {}",
            entry.label,
            entry.blockers.join(","),
            path_str
        )
    };
    MenuItem {
        label,
        filter_name: Some(path_str),
        epoch: mtime_epoch(entry.mtime),
        size_kb: Some(entry.size / 1024),
    }
}

fn filter_plan_entries(mut plan: ProtoPlan, idxs: &[usize]) -> ProtoPlan {
    let keep: std::collections::HashSet<usize> = idxs.iter().copied().collect();
    plan.entries = plan
        .entries
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, e)| e)
        .collect();
    plan
}

fn mtime_epoch(mtime: SystemTime) -> Option<i64> {
    mtime
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

fn write_plan_output(opts: &WorktreeOptions, plan: &ProtoPlan) -> io::Result<()> {
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
        "worktree plan: {} entries (ttl {}s)",
        plan.entries.len(),
        plan.ttl_secs
    );
    for entry in &plan.entries {
        if entry.blockers.is_empty() {
            eprintln!(
                "  {}  {}  {}",
                units::bytes_bin(entry.size),
                entry.rule_id,
                entry.path.display()
            );
        } else {
            eprintln!(
                "  {}  {}  {}  blockers={}",
                units::bytes_bin(entry.size),
                entry.rule_id,
                entry.path.display(),
                entry.blockers.join(",")
            );
        }
    }
    if let Some(note) = &plan.coverage_note {
        eprintln!("\n{note}");
    }
}

fn print_human_report(report: &Report) {
    eprintln!(
        "worktree apply: succeeded={} skipped={} failed={} trashed={} deleted={}",
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
        .name("vole-worktree-stream".into())
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
        MutexError::AlreadyRunning => io::Error::other("another vole worktree is running"),
        other => io::Error::other(other.to_string()),
    }
}

fn map_apply_error(e: WorktreeApplyError) -> io::Error {
    io::Error::other(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare_opts() -> WorktreeOptions {
        WorktreeOptions {
            explicit_plan: false,
            json: false,
            json_stream: false,
            plan_out: None,
            apply_plan: None,
            permanent: false,
        }
    }

    #[test]
    fn interactive_gate_requires_bare_tty() {
        let bare = bare_opts();
        assert!(!gate_interactive(false, false, &bare));
        assert!(gate_interactive(true, true, &bare));
        assert!(!gate_interactive(
            true,
            true,
            &WorktreeOptions {
                explicit_plan: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &WorktreeOptions {
                json: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &WorktreeOptions {
                apply_plan: Some(PathBuf::from("p.json")),
                ..bare_opts()
            }
        ));
    }
}
