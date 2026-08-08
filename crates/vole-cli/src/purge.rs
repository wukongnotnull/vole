//! `vole purge` plan / apply 接线。

use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::SystemTime;

use crossbeam_channel::unbounded;
use vole_core::mutex::{try_lock_purge, MutexError};
use vole_core::ops::{
    apply_purge_plan, build_purge_plan, coverage_with_apply_permission_hint,
    report_has_permission_skips, PurgeApplyError, PurgeApplyOptions, PurgePlanOptions,
    APPLY_PERMISSION_WARN, DEFAULT_PURGE_MIN_AGE_DAYS,
};
use vole_core::protection::AppProtection;
use vole_core::units;
use vole_core::vole_proto::{Plan as ProtoPlan, Report, StreamEvent, SCHEMA_VERSION};

use crate::signals;

pub struct PurgeOptions {
    pub json: bool,
    pub json_stream: bool,
    pub plan_out: Option<PathBuf>,
    pub apply_plan: Option<PathBuf>,
    pub permanent: bool,
    pub include_empty: bool,
}

pub fn run_purge(opts: PurgeOptions) -> i32 {
    match run_purge_inner(opts) {
        Ok(()) => 0,
        Err(e) if e.kind() == io::ErrorKind::Interrupted => 130,
        Err(e) => {
            eprintln!("vole purge: {e}");
            1
        }
    }
}

fn run_purge_inner(opts: PurgeOptions) -> io::Result<()> {
    let _lock = try_lock_purge().map_err(map_mutex_error)?;

    if let Some(ref plan_path) = opts.apply_plan {
        return run_apply(&opts, plan_path);
    }
    run_plan(opts)
}

fn run_plan(opts: PurgeOptions) -> io::Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME not set"))?;
    let protection = AppProtection::new();
    let plan_opts = PurgePlanOptions {
        home: &home,
        ttl_secs: 900,
        search_roots: None,
        include_empty: opts.include_empty,
        min_age_days: DEFAULT_PURGE_MIN_AGE_DAYS,
        now: SystemTime::now(),
    };

    let cancel = vole_core::cancel::CancelToken::new();
    signals::spawn_signal_cancel(cancel);

    let stream_tx = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let _ = event_tx.send(StreamEvent::Progress {
            scanned: 0,
            current: "scanning project artifacts".into(),
        });
        Some((event_tx, writer))
    } else {
        None
    };

    let plan =
        build_purge_plan(&protection, &plan_opts).map_err(|e| io::Error::other(e.to_string()))?;

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

fn run_apply(opts: &PurgeOptions, plan_path: &Path) -> io::Result<()> {
    let json = std::fs::read_to_string(plan_path)?;
    let plan: ProtoPlan = serde_json::from_str(&json).map_err(io::Error::other)?;
    if plan.schema_version != SCHEMA_VERSION {
        return Err(io::Error::other(format!(
            "unsupported plan schema version {}",
            plan.schema_version
        )));
    }

    let protection = AppProtection::new();
    let apply_opts = PurgeApplyOptions {
        permanent: opts.permanent,
    };

    let mut report = if opts.json_stream {
        let (event_tx, event_rx) = unbounded();
        let writer = spawn_stream_writer(event_rx)?;
        let on_event = |event: StreamEvent| {
            let _ = event_tx.send(event);
        };
        let report = apply_purge_plan(&plan, &protection, apply_opts, Some(&on_event))
            .map_err(map_apply_error)?;
        drop(event_tx);
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
        report
    } else {
        apply_purge_plan(&plan, &protection, apply_opts, None).map_err(map_apply_error)?
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

fn write_plan_output(opts: &PurgeOptions, plan: &ProtoPlan) -> io::Result<()> {
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
        "purge plan: {} entries (ttl {}s)",
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
        "purge apply: succeeded={} skipped={} failed={} trashed={} deleted={}",
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
        .name("vole-purge-stream".into())
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
        MutexError::AlreadyRunning => io::Error::other("another vole purge is running"),
        other => io::Error::other(other.to_string()),
    }
}

fn map_apply_error(e: PurgeApplyError) -> io::Error {
    io::Error::other(e.to_string())
}
