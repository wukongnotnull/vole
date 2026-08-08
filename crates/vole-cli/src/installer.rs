//! `vole installer` plan / apply 接线。

use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::SystemTime;

use crossbeam_channel::unbounded;
use vole_core::mutex::{try_lock_installer, MutexError};
use vole_core::ops::{
    apply_installer_plan, build_installer_plan, coverage_with_apply_permission_hint,
    report_has_permission_skips, InstallerApplyError, InstallerApplyOptions, InstallerPlanOptions,
    APPLY_PERMISSION_WARN, DEFAULT_INSTALLER_SCAN_MAX_DEPTH,
};
use vole_core::protection::AppProtection;
use vole_core::units;
use vole_core::vole_proto::{Plan as ProtoPlan, Report, StreamEvent, SCHEMA_VERSION};

use crate::signals;

pub struct InstallerOptions {
    pub json: bool,
    pub json_stream: bool,
    pub plan_out: Option<PathBuf>,
    pub apply_plan: Option<PathBuf>,
    pub permanent: bool,
}

pub fn run_installer(opts: InstallerOptions) -> i32 {
    match run_installer_inner(opts) {
        Ok(code) => code,
        Err(e) if e.kind() == io::ErrorKind::Interrupted => 130,
        Err(e) => {
            eprintln!("vole installer: {e}");
            1
        }
    }
}

fn run_installer_inner(opts: InstallerOptions) -> io::Result<i32> {
    let _lock = try_lock_installer().map_err(map_mutex_error)?;

    if let Some(ref plan_path) = opts.apply_plan {
        return run_apply(&opts, plan_path);
    }
    run_plan(opts)?;
    Ok(0)
}

fn run_plan(opts: InstallerOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let protection = AppProtection::new();
    let injected = parse_scan_roots_env();
    let max_depth = parse_max_depth_env();
    let roots_owned = injected;
    let plan_opts = InstallerPlanOptions {
        home: &home,
        ttl_secs: 900,
        scan_roots: roots_owned.as_deref(),
        max_depth,
        now: SystemTime::now(),
    };

    let cancel = vole_core::cancel::CancelToken::new();
    signals::spawn_signal_cancel(cancel);

    let stream_tx = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let _ = event_tx.send(StreamEvent::Progress {
            scanned: 0,
            current: "scanning installer packages".into(),
        });
        Some((event_tx, writer))
    } else {
        None
    };

    let plan = build_installer_plan(&protection, &plan_opts)
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

fn run_apply(opts: &InstallerOptions, plan_path: &Path) -> io::Result<i32> {
    let json = std::fs::read_to_string(plan_path)?;
    let plan: ProtoPlan = serde_json::from_str(&json).map_err(io::Error::other)?;
    if plan.schema_version != SCHEMA_VERSION {
        return Err(io::Error::other(format!(
            "unsupported plan schema version {}",
            plan.schema_version
        )));
    }

    let protection = AppProtection::new();
    let apply_opts = InstallerApplyOptions {
        permanent: opts.permanent,
    };

    let mut report = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let on_event = |event: StreamEvent| {
            let _ = event_tx.send(event);
        };
        let report = apply_installer_plan(&plan, &protection, apply_opts, Some(&on_event))
            .map_err(map_apply_error)?;
        drop(event_tx);
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
        report
    } else {
        apply_installer_plan(&plan, &protection, apply_opts, None).map_err(map_apply_error)?
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

    // Mole INSTALLER_EXIT_INCOMPLETE 精神：有失败则非零。
    if report.failed > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn parse_scan_roots_env() -> Option<Vec<PathBuf>> {
    let raw = env::var_os("VOLE_INSTALLER_SCAN_ROOTS")?;
    let s = raw.to_string_lossy();
    if s.is_empty() {
        return None;
    }
    let roots: Vec<PathBuf> = s
        .split(':')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    if roots.is_empty() {
        None
    } else {
        Some(roots)
    }
}

fn parse_max_depth_env() -> usize {
    env::var("VOLE_INSTALLER_SCAN_MAX_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&d| d > 0)
        .unwrap_or(DEFAULT_INSTALLER_SCAN_MAX_DEPTH)
}

fn write_plan_output(opts: &InstallerOptions, plan: &ProtoPlan) -> io::Result<()> {
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
        "installer plan: {} entries (ttl {}s)",
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
        "installer apply: succeeded={} skipped={} failed={} trashed={} deleted={}",
        report.succeeded,
        report.skipped,
        report.failed,
        units::bytes_bin(report.trashed_bytes),
        units::bytes_bin(report.deleted_bytes)
    );
    if report.failed > 0 {
        eprintln!("installer cleanup incomplete");
    }
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
        .name("vole-installer-stream".into())
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
        MutexError::AlreadyRunning => io::Error::other("another vole installer is running"),
        other => io::Error::other(other.to_string()),
    }
}

fn map_apply_error(e: InstallerApplyError) -> io::Error {
    io::Error::other(e.to_string())
}
