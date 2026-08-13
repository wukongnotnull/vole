//! `vole clean` plan / apply / whitelist 接线。

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::thread;

use crossbeam_channel::unbounded;
use vole_core::cancel::CancelToken;
use vole_core::mutex::{try_lock_clean, MutexError};
use vole_core::ops::{
    apply_proto_plan, collect_clean_hints, coverage_note, coverage_with_apply_permission_hint,
    coverage_with_orphan_notices, enabled_rule_count, plan_to_proto, report_has_permission_skips,
    ApplyPlanError, ApplyPlanOptions, CleanHints, CleanHintsOptions, HintItem, OpsError,
    Orchestrator, Plan, PlanNotice, ProtoPlanError, APPLY_PERMISSION_WARN,
    GROUP_CONTAINERS_TRUNCATED_WARN, GROUP_CONTAINERS_WARN, HANDOFF_PASTEBOARD_TRUNCATED_WARN,
    HANDOFF_PASTEBOARD_WARN, ORPHAN_LIBRARY_WARN, SYSTEM_SERVICES_WARN,
};
use vole_core::protection::AppProtection;
use vole_core::rules::{default_rules_dir, load_rules_from_dir, LoadError, PgrepProcessProbe};
use vole_core::units;
use vole_core::vole_proto::{HintNotice, Plan as ProtoPlan, Report, StreamEvent, SCHEMA_VERSION};
use vole_core::whitelist;

use crate::signals;
use crate::tui::{run_paginated_select, MenuItem, MenuState, SelectOutcome, SortMode};

pub struct CleanOptions {
    /// `--plan` / `--dry-run` / `-n`：强制走自动化 plan 路径。
    pub explicit_plan: bool,
    pub json: bool,
    pub json_stream: bool,
    pub plan_out: Option<PathBuf>,
    pub apply_plan: Option<PathBuf>,
    pub permanent: bool,
    pub whitelist: bool,
    pub whitelist_add: Option<String>,
    pub whitelist_remove: Option<String>,
    pub whitelist_list: bool,
}

impl CleanOptions {
    fn is_whitelist_command(&self) -> bool {
        self.whitelist
            || self.whitelist_list
            || self.whitelist_add.is_some()
            || self.whitelist_remove.is_some()
    }
}

pub fn run_clean(opts: CleanOptions) -> i32 {
    match run_clean_inner(opts) {
        Ok(()) => 0,
        Err(e) if e.kind() == io::ErrorKind::Interrupted => 130,
        Err(e) => {
            eprintln!("vole clean: {}", e);
            1
        }
    }
}

fn run_clean_inner(opts: CleanOptions) -> io::Result<()> {
    if opts.is_whitelist_command() {
        return run_whitelist(&opts);
    }

    let _lock = try_lock_clean().map_err(map_mutex_error)?;

    if let Some(ref plan_path) = opts.apply_plan {
        return run_apply(&opts, plan_path);
    }
    if gate_interactive(io::stdin().is_terminal(), io::stdout().is_terminal(), &opts) {
        return run_interactive(&opts);
    }
    run_plan(opts)
}

/// TTY 裸调用进入确认轨的门控（可单测，不依赖真实 TTY）。
pub(crate) fn gate_interactive(stdin_tty: bool, stdout_tty: bool, opts: &CleanOptions) -> bool {
    stdin_tty
        && stdout_tty
        && !opts.explicit_plan
        && !opts.json
        && !opts.json_stream
        && opts.plan_out.is_none()
        && opts.apply_plan.is_none()
        && !opts.is_whitelist_command()
}

pub(crate) fn clean_scan_spinner_message() -> &'static str {
    "Scanning caches..."
}

fn run_interactive(opts: &CleanOptions) -> io::Result<()> {
    let spinner = crate::tty_spinner::TtySpinner::start(clean_scan_spinner_message());
    let rules = load_rules_from_dir(default_rules_dir()).map_err(map_load_error)?;
    let enabled = enabled_rule_count(&rules);
    let whitelist_patterns = whitelist::load_clean()?;
    let protection = AppProtection::new();

    let cancel = CancelToken::new();
    signals::spawn_signal_cancel(cancel.clone());
    let orch = Orchestrator::new(cancel, None);

    let plan = match orch.build_plan(&rules, &protection, &whitelist_patterns) {
        Ok(plan) => plan,
        Err(OpsError::Cancelled) => {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        Err(OpsError::Strategy(e)) => {
            return Err(io::Error::other(format!("strategy: {e}")));
        }
    };
    drop(orch);

    if plan.entries.is_empty() {
        spinner.stop();
        eprintln!("Nothing to clean.");
        return Ok(());
    }

    let base_note = coverage_note(enabled);
    let note = coverage_with_orphan_notices(&base_note, &plan.notices);
    let mut proto = plan_to_proto(&plan).map_err(map_proto_error)?;
    proto.coverage_note = Some(note);
    let hints = collect_plan_hints();
    spinner.stop();
    print_human_plan(&plan, &base_note);
    print_human_hints(&hints);

    eprint!("Proceed with clean? [y/N] ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    if !line.trim().eq_ignore_ascii_case("y") {
        eprintln!("Aborted.");
        return Ok(());
    }

    let apply_opts = ApplyPlanOptions {
        permanent: opts.permanent,
    };
    let process_probe = PgrepProcessProbe;
    let report = apply_proto_plan(
        &proto,
        &protection,
        &whitelist_patterns,
        apply_opts,
        &rules,
        &process_probe,
        None,
    )
    .map_err(map_apply_error)?;
    print_human_report(&report);
    Ok(())
}

fn run_plan(opts: CleanOptions) -> io::Result<()> {
    let rules = load_rules_from_dir(default_rules_dir()).map_err(map_load_error)?;
    let enabled = enabled_rule_count(&rules);
    let whitelist_patterns = whitelist::load_clean()?;
    let protection = AppProtection::new();

    let cancel = CancelToken::new();
    signals::spawn_signal_cancel(cancel.clone());

    let (orch, stream_writer) = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        (
            Orchestrator::new(cancel.clone(), Some(event_tx)),
            Some(writer),
        )
    } else {
        (Orchestrator::new(cancel.clone(), None), None)
    };

    let plan = match orch.build_plan(&rules, &protection, &whitelist_patterns) {
        Ok(plan) => plan,
        Err(OpsError::Cancelled) => {
            if opts.json_stream {
                orch.emit(StreamEvent::Aborted {
                    reason: "cancelled".into(),
                });
            }
            drop(orch);
            if let Some(handle) = stream_writer {
                handle
                    .join()
                    .map_err(|_| io::Error::other("stream writer panicked"))??;
            }
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        Err(OpsError::Strategy(e)) => {
            drop(orch);
            if let Some(handle) = stream_writer {
                let _ = handle.join();
            }
            return Err(io::Error::other(format!("strategy: {e}")));
        }
    };

    let base_note = coverage_note(enabled);
    let note = coverage_with_orphan_notices(&base_note, &plan.notices);

    if opts.json_stream {
        orch.emit(StreamEvent::Done {
            report: plan_done_report(&note),
        });
    }
    drop(orch);
    if let Some(handle) = stream_writer {
        handle
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
    }

    let mut proto = plan_to_proto(&plan).map_err(map_proto_error)?;
    proto.coverage_note = Some(note.clone());
    let hints = collect_plan_hints();
    write_plan_output(&opts, &plan, &proto, &base_note, &hints)?;
    Ok(())
}

fn collect_plan_hints() -> CleanHints {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    let Some(home) = home else {
        return CleanHints::default();
    };
    collect_clean_hints(&CleanHintsOptions::production(&home))
}

fn hint_items_to_notices(items: &[HintItem]) -> Vec<HintNotice> {
    items
        .iter()
        .map(|h| HintNotice {
            kind: h.kind.as_str().into(),
            summary: h.summary.clone(),
            detail: h.detail.clone(),
        })
        .collect()
}

fn plan_json_with_hints(
    proto: &ProtoPlan,
    notices: &[HintNotice],
) -> io::Result<serde_json::Value> {
    let mut value = serde_json::to_value(proto).map_err(io::Error::other)?;
    if !notices.is_empty() {
        let hints = serde_json::to_value(notices).map_err(io::Error::other)?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("hints".into(), hints);
        }
    }
    Ok(value)
}

fn run_apply(opts: &CleanOptions, plan_path: &PathBuf) -> io::Result<()> {
    let json = std::fs::read_to_string(plan_path)?;
    let plan: ProtoPlan = serde_json::from_str(&json).map_err(io::Error::other)?;

    let rules = load_rules_from_dir(default_rules_dir()).map_err(map_load_error)?;
    let whitelist_patterns = whitelist::load_clean()?;
    let protection = AppProtection::new();
    let apply_opts = ApplyPlanOptions {
        permanent: opts.permanent,
    };
    let process_probe = PgrepProcessProbe;

    let report = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let on_event = |event: StreamEvent| {
            let _ = event_tx.send(event);
        };
        let report = apply_proto_plan(
            &plan,
            &protection,
            &whitelist_patterns,
            apply_opts,
            &rules,
            &process_probe,
            Some(&on_event),
        )
        .map_err(map_apply_error)?;
        // Close the channel so the stream writer exits; without this, join hangs
        // forever (uninstall/optimize already drop; clean apply was missing it).
        drop(event_tx);
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
        report
    } else {
        apply_proto_plan(
            &plan,
            &protection,
            &whitelist_patterns,
            apply_opts,
            &rules,
            &process_probe,
            None,
        )
        .map_err(map_apply_error)?
    };

    write_apply_output(opts, report)?;
    Ok(())
}

fn spawn_stream_writer(
    event_rx: crossbeam_channel::Receiver<StreamEvent>,
) -> io::Result<thread::JoinHandle<io::Result<()>>> {
    thread::Builder::new()
        .name("vole-clean-stream".into())
        .spawn(move || {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            while let Ok(event) = event_rx.recv() {
                write_stream_event(&mut out, event)?;
            }
            Ok(())
        })
        .map_err(io::Error::other)
}

fn write_stream_event(out: &mut impl Write, event: StreamEvent) -> io::Result<()> {
    let value = event.with_schema(SCHEMA_VERSION);
    let line = serde_json::to_string(&value).map_err(io::Error::other)?;
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

fn write_plan_output(
    opts: &CleanOptions,
    plan: &Plan,
    proto: &ProtoPlan,
    base_coverage: &str,
    hints: &CleanHints,
) -> io::Result<()> {
    let notices = hint_items_to_notices(&hints.items);
    let payload = plan_json_with_hints(proto, &notices)?;

    if let Some(path) = &opts.plan_out {
        let json = serde_json::to_string_pretty(&payload).map_err(io::Error::other)?;
        std::fs::write(path, format!("{json}\n"))?;
    }

    if opts.json_stream {
        return Ok(());
    }

    if should_use_json(opts.json) {
        let json = serde_json::to_string(&payload).map_err(io::Error::other)?;
        println!("{json}");
        return Ok(());
    }

    print_human_plan(plan, base_coverage);
    print_human_hints(hints);
    Ok(())
}

fn print_human_hints(hints: &CleanHints) {
    if hints.items.is_empty() {
        return;
    }
    eprintln!();
    for item in &hints.items {
        eprintln!("  ! {}", item.summary);
        if let Some(detail) = &item.detail {
            eprintln!("    {detail}");
        }
    }
}

fn write_apply_output(opts: &CleanOptions, mut report: Report) -> io::Result<()> {
    if opts.json_stream {
        return Ok(());
    }

    if should_use_json(opts.json) {
        report.coverage_note =
            coverage_with_apply_permission_hint(report.coverage_note.as_deref(), &report);
        let json = serde_json::to_string(&report).map_err(io::Error::other)?;
        println!("{json}");
        return Ok(());
    }

    print_human_report(&report);
    Ok(())
}

fn should_use_json(force: bool) -> bool {
    if force {
        return true;
    }
    !io::stdout().is_terminal()
}

fn print_human_plan(plan: &Plan, coverage: &str) {
    println!(
        "Plan: {} candidate(s), TTL {}s",
        plan.entries.len(),
        plan.ttl.as_secs()
    );
    for entry in &plan.entries {
        println!(
            "  {}  {}  ({})",
            entry.path.display(),
            entry.label,
            entry.rule_id
        );
    }
    eprintln!();
    eprintln!("{coverage}");
    if plan
        .notices
        .contains(&PlanNotice::OrphanLibraryInaccessible)
    {
        eprintln!("{ORPHAN_LIBRARY_WARN}");
    }
    if plan
        .notices
        .contains(&PlanNotice::SystemServicesInaccessible)
    {
        eprintln!("{SYSTEM_SERVICES_WARN}");
    }
    if plan
        .notices
        .contains(&PlanNotice::GroupContainersInaccessible)
    {
        eprintln!("{GROUP_CONTAINERS_WARN}");
    }
    if plan.notices.contains(&PlanNotice::GroupContainersTruncated) {
        eprintln!("{GROUP_CONTAINERS_TRUNCATED_WARN}");
    }
    if plan
        .notices
        .contains(&PlanNotice::HandoffPasteboardInaccessible)
    {
        eprintln!("{HANDOFF_PASTEBOARD_WARN}");
    }
    if plan
        .notices
        .contains(&PlanNotice::HandoffPasteboardTruncated)
    {
        eprintln!("{HANDOFF_PASTEBOARD_TRUNCATED_WARN}");
    }
}

fn print_human_report(report: &Report) {
    println!(
        "Apply: {} succeeded, {} skipped, {} failed",
        report.succeeded, report.skipped, report.failed
    );
    if report.trashed_bytes > 0 {
        println!("移入废纸篓   {}", units::bytes_bin(report.trashed_bytes));
    }
    if report.deleted_bytes > 0 {
        println!("永久删除     {}", units::bytes_bin(report.deleted_bytes));
    }
    if report_has_permission_skips(report) {
        eprintln!("{APPLY_PERMISSION_WARN}");
    }
}

fn run_whitelist(opts: &CleanOptions) -> io::Result<()> {
    if let Some(path) = &opts.whitelist_add {
        whitelist::add_clean(path)?;
        println!("已添加白名单: {path}");
        return Ok(());
    }
    if let Some(path) = &opts.whitelist_remove {
        if whitelist::remove_clean(path)? {
            println!("已移除白名单: {path}");
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("白名单中未找到: {path}"),
            ));
        }
        return Ok(());
    }
    if opts.whitelist_list {
        let patterns = whitelist::load_clean()?;
        if should_use_json(opts.json) {
            let json = serde_json::to_string(&patterns).map_err(io::Error::other)?;
            println!("{json}");
        } else {
            print_whitelist_list(&patterns);
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

fn print_whitelist_list(patterns: &[String]) {
    if patterns.is_empty() {
        println!("白名单为空");
        return;
    }
    println!("白名单（受保护路径）:");
    for (idx, pattern) in patterns.iter().enumerate() {
        println!("  {}. {pattern}", idx + 1);
    }
}

fn run_whitelist_interactive() -> io::Result<()> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let current = whitelist::load_clean_for_manage()?;
    let build = whitelist::build_clean_whitelist_menu(&current, &home);
    if build.entries.is_empty() {
        return Err(io::Error::other("No items provided"));
    }

    let items: Vec<MenuItem> = build
        .entries
        .iter()
        .map(|e| MenuItem {
            label: e.label.clone(),
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
        "Whitelist Manager, Select caches to protect\nEdit: {}",
        whitelist::clean_config_display_path()
    );

    match run_paginated_select(&title, items, cfg)? {
        SelectOutcome::Cancelled => {
            println!("Cancelled, no changes saved");
            Ok(())
        }
        SelectOutcome::Back => crate::interactive::exit_to_home(),
        SelectOutcome::Confirmed(idxs) => {
            let merged =
                whitelist::merge_whitelist_selection(&build.entries, &idxs, &build.custom_patterns);
            whitelist::save_clean(&merged)?;
            let predefined = idxs.len();
            let custom = build.custom_patterns.len();
            println!("Whitelist Updated");
            if custom > 0 {
                println!("Protected {predefined} predefined + {custom} custom patterns");
            } else {
                println!("Protected {} caches", predefined);
            }
            println!("Config: {}", whitelist::clean_config_display_path());
            Ok(())
        }
    }
}

fn plan_done_report(coverage: &str) -> Report {
    Report {
        succeeded: 0,
        skipped: 0,
        failed: 0,
        skipped_by_reason: vec![],
        trashed_bytes: 0,
        deleted_bytes: 0,
        coverage_note: Some(coverage.to_string()),
    }
}

fn map_mutex_error(err: MutexError) -> io::Error {
    match err {
        MutexError::AlreadyRunning | MutexError::Rustix(_) => {
            io::Error::new(io::ErrorKind::WouldBlock, "另一个 vole clean 正在运行")
        }
        MutexError::Io(e) => e,
    }
}

fn map_load_error(err: LoadError) -> io::Error {
    match err {
        LoadError::Io(e) => e,
        LoadError::Toml(e) => io::Error::other(format!("rules: {e}")),
    }
}

fn map_proto_error(err: ProtoPlanError) -> io::Error {
    io::Error::other(err.to_string())
}

fn map_apply_error(err: ApplyPlanError) -> io::Error {
    io::Error::other(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare_opts() -> CleanOptions {
        CleanOptions {
            explicit_plan: false,
            json: false,
            json_stream: false,
            plan_out: None,
            apply_plan: None,
            permanent: false,
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
            &CleanOptions {
                explicit_plan: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                json: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                apply_plan: Some(PathBuf::from("p.json")),
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                whitelist: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                whitelist_list: true,
                ..bare_opts()
            }
        ));
    }

    #[test]
    fn clean_scan_spinner_message_matches_mole() {
        assert_eq!(clean_scan_spinner_message(), "Scanning caches...");
    }
}
