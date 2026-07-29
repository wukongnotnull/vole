//! `vole clean` plan / apply 接线。

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::thread;

use crossbeam_channel::unbounded;
use vole_core::cancel::CancelToken;
use vole_core::mutex::{try_lock_clean, MutexError};
use vole_core::ops::{
    apply_proto_plan, plan_to_proto, ApplyPlanError, ApplyPlanOptions, OpsError, Orchestrator,
    Plan, ProtoPlanError,
};
use vole_core::protection::AppProtection;
use vole_core::rules::{default_rules_dir, load_rules_from_dir, LoadError};
use vole_core::vole_proto::{Plan as ProtoPlan, Report, StreamEvent, SCHEMA_VERSION};
use vole_core::whitelist;

use crate::signals;

pub struct CleanOptions {
    pub json: bool,
    pub json_stream: bool,
    pub plan_out: Option<PathBuf>,
    pub apply_plan: Option<PathBuf>,
    pub permanent: bool,
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
    let _lock = try_lock_clean().map_err(map_mutex_error)?;

    if let Some(ref plan_path) = opts.apply_plan {
        return run_apply(&opts, plan_path);
    }

    run_plan(opts)
}

fn run_plan(opts: CleanOptions) -> io::Result<()> {
    let rules = load_rules_from_dir(default_rules_dir()).map_err(map_load_error)?;
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

    if opts.json_stream {
        orch.emit(StreamEvent::Done {
            report: zero_report(),
        });
    }
    drop(orch);
    if let Some(handle) = stream_writer {
        handle
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
    }

    let proto = plan_to_proto(&plan).map_err(map_proto_error)?;
    write_plan_output(&opts, &plan, &proto)?;
    Ok(())
}

fn run_apply(opts: &CleanOptions, plan_path: &PathBuf) -> io::Result<()> {
    let json = std::fs::read_to_string(plan_path)?;
    let plan: ProtoPlan = serde_json::from_str(&json).map_err(io::Error::other)?;

    let whitelist_patterns = whitelist::load_clean()?;
    let protection = AppProtection::new();
    let apply_opts = ApplyPlanOptions {
        permanent: opts.permanent,
    };

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
            Some(&on_event),
        )
        .map_err(map_apply_error)?;
        writer
            .join()
            .map_err(|_| io::Error::other("stream writer panicked"))??;
        report
    } else {
        apply_proto_plan(&plan, &protection, &whitelist_patterns, apply_opts, None)
            .map_err(map_apply_error)?
    };

    write_apply_output(opts, &report)?;
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

fn write_plan_output(opts: &CleanOptions, plan: &Plan, proto: &ProtoPlan) -> io::Result<()> {
    if let Some(path) = &opts.plan_out {
        let json = serde_json::to_string_pretty(proto).map_err(io::Error::other)?;
        std::fs::write(path, format!("{json}\n"))?;
    }

    if opts.json_stream {
        return Ok(());
    }

    if should_use_json(opts.json) {
        let json = serde_json::to_string(proto).map_err(io::Error::other)?;
        println!("{json}");
        return Ok(());
    }

    print_human_plan(plan);
    Ok(())
}

fn write_apply_output(opts: &CleanOptions, report: &Report) -> io::Result<()> {
    if opts.json_stream {
        return Ok(());
    }

    if should_use_json(opts.json) {
        let json = serde_json::to_string(report).map_err(io::Error::other)?;
        println!("{json}");
        return Ok(());
    }

    print_human_report(report);
    Ok(())
}

fn should_use_json(force: bool) -> bool {
    if force {
        return true;
    }
    !io::stdout().is_terminal()
}

fn print_human_plan(plan: &Plan) {
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
}

fn print_human_report(report: &Report) {
    println!(
        "Apply: {} succeeded, {} skipped, {} failed",
        report.succeeded, report.skipped, report.failed
    );
    if report.trashed_bytes > 0 || report.deleted_bytes > 0 {
        println!(
            "  trashed {} bytes, permanently deleted {} bytes",
            report.trashed_bytes, report.deleted_bytes
        );
    }
}

fn zero_report() -> Report {
    Report {
        succeeded: 0,
        skipped: 0,
        failed: 0,
        skipped_by_reason: vec![],
        trashed_bytes: 0,
        deleted_bytes: 0,
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
