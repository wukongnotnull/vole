//! `vole optimize` plan / apply / whitelist 接线。

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::thread;

use crossbeam_channel::unbounded;
use vole_core::mutex::{try_lock_optimize, MutexError};
use vole_core::ops::{
    apply_optimize_plan, build_optimize_plan, coverage_with_apply_permission_hint,
    report_has_permission_skips, OptimizeApplyError, OptimizeApplyOptions, OptimizePlanOptions,
    APPLY_PERMISSION_WARN,
};
use vole_core::protection::{AppProtection, ProtectionCatalog};
use vole_core::units;
use vole_core::vole_proto::{Plan as ProtoPlan, Report, StreamEvent, SCHEMA_VERSION};
use vole_core::whitelist;

use crate::signals;
use crate::tui::{run_paginated_select, MenuItem, MenuState, SelectOutcome, SortMode};

pub struct OptimizeOptions {
    /// `--plan` / `--dry-run` / `-n`：强制走自动化 plan 路径。
    pub explicit_plan: bool,
    pub json: bool,
    pub json_stream: bool,
    pub plan_out: Option<PathBuf>,
    pub apply_plan: Option<PathBuf>,
    pub permanent: bool,
    pub task: Option<String>,
    pub whitelist: bool,
    pub whitelist_add: Option<String>,
    pub whitelist_remove: Option<String>,
    pub whitelist_list: bool,
}

impl OptimizeOptions {
    fn is_whitelist_command(&self) -> bool {
        self.whitelist
            || self.whitelist_list
            || self.whitelist_add.is_some()
            || self.whitelist_remove.is_some()
    }
}

pub fn run_optimize(opts: OptimizeOptions) -> i32 {
    match run_optimize_inner(opts) {
        Ok(()) => 0,
        Err(e) if e.kind() == io::ErrorKind::Interrupted => 130,
        Err(e) => {
            eprintln!("vole optimize: {e}");
            1
        }
    }
}

fn run_optimize_inner(opts: OptimizeOptions) -> io::Result<()> {
    let _lock = try_lock_optimize().map_err(map_mutex_error)?;

    if opts.is_whitelist_command() {
        return run_whitelist(&opts);
    }
    if let Some(ref plan_path) = opts.apply_plan {
        return run_apply(&opts, plan_path);
    }
    if gate_interactive(io::stdin().is_terminal(), io::stdout().is_terminal(), &opts) {
        return run_interactive(&opts);
    }
    run_plan(opts)
}

/// TTY 裸调用进入确认轨的门控（可单测，不依赖真实 TTY）。
pub(crate) fn gate_interactive(stdin_tty: bool, stdout_tty: bool, opts: &OptimizeOptions) -> bool {
    stdin_tty
        && stdout_tty
        && !opts.explicit_plan
        && !opts.json
        && !opts.json_stream
        && opts.plan_out.is_none()
        && opts.apply_plan.is_none()
        && !opts.is_whitelist_command()
}

fn run_interactive(opts: &OptimizeOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let catalog = ProtectionCatalog::embedded();
    let protection = AppProtection::new();
    let task_whitelist = vole_core::whitelist::load_optimize()?;
    let plan_opts = OptimizePlanOptions {
        home: &home,
        ttl_secs: 900,
        only_task: opts.task.as_deref(),
        task_whitelist: &task_whitelist,
    };

    let cancel = vole_core::cancel::CancelToken::new();
    signals::spawn_signal_cancel(cancel);

    let plan = build_optimize_plan(&catalog, &protection, &plan_opts)
        .map_err(|e| io::Error::other(e.to_string()))?;

    if plan.entries.is_empty() {
        eprintln!("Nothing to optimize.");
        return Ok(());
    }

    print_human_plan(&plan);

    eprint!("Proceed with optimize? [y/N] ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    if !line.trim().eq_ignore_ascii_case("y") {
        eprintln!("Aborted.");
        return Ok(());
    }

    let apply_opts = OptimizeApplyOptions {
        permanent: opts.permanent,
    };
    let report = apply_optimize_plan(&plan, &protection, apply_opts, &task_whitelist, None)
        .map_err(map_apply_error)?;
    print_human_report(&report);
    Ok(())
}

fn run_plan(opts: OptimizeOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let catalog = ProtectionCatalog::embedded();
    let protection = AppProtection::new();
    let task_whitelist = vole_core::whitelist::load_optimize()?;
    let plan_opts = OptimizePlanOptions {
        home: &home,
        ttl_secs: 900,
        only_task: opts.task.as_deref(),
        task_whitelist: &task_whitelist,
    };

    let cancel = vole_core::cancel::CancelToken::new();
    signals::spawn_signal_cancel(cancel);

    let stream_tx = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let _ = event_tx.send(StreamEvent::Progress {
            scanned: 0,
            current: "scanning optimize tasks".into(),
        });
        Some((event_tx, writer))
    } else {
        None
    };

    let plan = build_optimize_plan(&catalog, &protection, &plan_opts)
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

fn run_apply(opts: &OptimizeOptions, plan_path: &Path) -> io::Result<()> {
    let json = std::fs::read_to_string(plan_path)?;
    let plan: ProtoPlan = serde_json::from_str(&json).map_err(io::Error::other)?;
    if plan.schema_version != SCHEMA_VERSION {
        return Err(io::Error::other(format!(
            "unsupported plan schema version {}",
            plan.schema_version
        )));
    }

    let protection = AppProtection::new();
    let apply_opts = OptimizeApplyOptions {
        permanent: opts.permanent,
    };
    let task_whitelist = vole_core::whitelist::load_optimize()?;

    let mut report = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let on_event = |event: StreamEvent| {
            let _ = event_tx.send(event);
        };
        let report = apply_optimize_plan(
            &plan,
            &protection,
            apply_opts,
            &task_whitelist,
            Some(&on_event),
        )
        .map_err(map_apply_error)?;
        drop(event_tx);
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
        report
    } else {
        apply_optimize_plan(&plan, &protection, apply_opts, &task_whitelist, None)
            .map_err(map_apply_error)?
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

fn write_plan_output(opts: &OptimizeOptions, plan: &ProtoPlan) -> io::Result<()> {
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
        "optimize plan: {} entries (ttl {}s)",
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
        "optimize apply: succeeded={} skipped={} failed={} trashed={} deleted={}",
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
        .name("vole-optimize-stream".into())
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
        MutexError::AlreadyRunning => io::Error::other("another vole optimize is running"),
        other => io::Error::other(other.to_string()),
    }
}

fn map_apply_error(e: OptimizeApplyError) -> io::Error {
    io::Error::other(e.to_string())
}

fn run_whitelist(opts: &OptimizeOptions) -> io::Result<()> {
    if let Some(id) = &opts.whitelist_add {
        whitelist::add_optimize(id)?;
        println!("已添加优化白名单: {id}");
        return Ok(());
    }
    if let Some(id) = &opts.whitelist_remove {
        if whitelist::remove_optimize(id)? {
            println!("已移除优化白名单: {id}");
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("优化白名单中未找到: {id}"),
            ));
        }
        return Ok(());
    }
    if opts.whitelist_list {
        let ids = whitelist::load_optimize()?;
        if should_use_json(opts.json) {
            let json = serde_json::to_string(&ids).map_err(io::Error::other)?;
            println!("{json}");
        } else {
            print_whitelist_list(&ids);
        }
        return Ok(());
    }
    if opts.whitelist {
        if io::stdin().is_terminal() {
            return run_whitelist_interactive();
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "非交互环境请使用 --whitelist-add、--whitelist-remove 或 --whitelist-list",
        ));
    }
    Ok(())
}

fn print_whitelist_list(ids: &[String]) {
    if ids.is_empty() {
        println!("白名单为空");
        return;
    }
    println!("优化任务白名单（跳过执行）:");
    for (idx, id) in ids.iter().enumerate() {
        println!("  {}. {id}", idx + 1);
    }
}

fn run_whitelist_interactive() -> io::Result<()> {
    let current = whitelist::load_optimize()?;
    let build = whitelist::build_optimize_whitelist_menu(&current);
    if build.entries.is_empty() {
        return Err(io::Error::other("No items provided"));
    }

    let items: Vec<MenuItem> = build
        .entries
        .iter()
        .map(|e| MenuItem {
            label: format!("{} ({})", e.label, e.pattern),
            filter_name: Some(e.label.clone()),
            epoch: None,
            size_kb: None,
        })
        .collect();

    let mut cfg = MenuState::config_from_env();
    cfg.sort_mode = SortMode::Name;
    cfg.ignore_initial_enter = true;
    cfg.preselected = build.preselected.clone();
    if let Ok((_, rows)) = crossterm::terminal::size() {
        cfg.term_height = rows;
    }

    let title = format!(
        "Optimize Whitelist, Select tasks to skip\nEdit: {}",
        whitelist::optimize_config_display_path()
    );

    match run_paginated_select(&title, items, cfg)? {
        SelectOutcome::Cancelled => {
            println!("Cancelled, no changes saved");
            Ok(())
        }
        SelectOutcome::Confirmed(idxs) => {
            let merged =
                whitelist::merge_whitelist_selection(&build.entries, &idxs, &build.custom_patterns);
            whitelist::save_optimize(&merged)?;
            println!("Whitelist Updated");
            println!("Skipped {} tasks", idxs.len());
            println!("Config: {}", whitelist::optimize_config_display_path());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare_opts() -> OptimizeOptions {
        OptimizeOptions {
            explicit_plan: false,
            json: false,
            json_stream: false,
            plan_out: None,
            apply_plan: None,
            permanent: false,
            task: None,
            whitelist: false,
            whitelist_add: None,
            whitelist_remove: None,
            whitelist_list: false,
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
            &OptimizeOptions {
                explicit_plan: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &OptimizeOptions {
                json_stream: true,
                ..bare_opts()
            }
        ));
        assert!(gate_interactive(
            true,
            true,
            &OptimizeOptions {
                task: Some("dns_flush".into()),
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &OptimizeOptions {
                whitelist: true,
                ..bare_opts()
            }
        ));
    }
}
