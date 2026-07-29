//! `vole clean` plan / apply / whitelist 接线。

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::thread;

use crossbeam_channel::unbounded;
use vole_core::cancel::CancelToken;
use vole_core::mutex::{try_lock_clean, MutexError};
use vole_core::ops::{
    apply_proto_plan, coverage_note, enabled_rule_count, plan_to_proto, ApplyPlanError,
    ApplyPlanOptions, OpsError, Orchestrator, Plan, ProtoPlanError,
};
use vole_core::protection::AppProtection;
use vole_core::rules::{default_rules_dir, load_rules_from_dir, LoadError};
use vole_core::units;
use vole_core::vole_proto::{Plan as ProtoPlan, Report, StreamEvent, SCHEMA_VERSION};
use vole_core::whitelist;

use crate::signals;

pub struct CleanOptions {
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

    run_plan(opts)
}

fn run_plan(opts: CleanOptions) -> io::Result<()> {
    let rules = load_rules_from_dir(default_rules_dir()).map_err(map_load_error)?;
    let enabled = enabled_rule_count(&rules);
    let note = coverage_note(enabled);
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
    write_plan_output(&opts, &plan, &proto, &note)?;
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

fn write_plan_output(
    opts: &CleanOptions,
    plan: &Plan,
    proto: &ProtoPlan,
    coverage: &str,
) -> io::Result<()> {
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

    print_human_plan(plan, coverage);
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
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        let patterns = whitelist::load_clean()?;
        print_whitelist_list(&patterns);
        writeln!(stdout)?;
        write!(stdout, "[a] 添加  [r] 移除  [q] 退出 > ")?;
        stdout.flush()?;

        let mut action = String::new();
        stdin.lock().read_line(&mut action)?;
        let action = action.trim().to_lowercase();
        match action.as_str() {
            "q" | "quit" | "" => break,
            "a" | "add" => {
                write!(stdout, "路径 pattern: ")?;
                stdout.flush()?;
                let mut path = String::new();
                stdin.lock().read_line(&mut path)?;
                let path = path.trim();
                if path.is_empty() {
                    continue;
                }
                whitelist::add_clean(path)?;
                println!("已添加: {path}");
            }
            "r" | "remove" => {
                if patterns.is_empty() {
                    println!("白名单为空，无可移除项");
                    continue;
                }
                write!(stdout, "编号或路径: ")?;
                stdout.flush()?;
                let mut input = String::new();
                stdin.lock().read_line(&mut input)?;
                let input = input.trim();
                if input.is_empty() {
                    continue;
                }
                let target = if let Ok(num) = input.parse::<usize>() {
                    patterns.get(num.saturating_sub(1)).map(String::as_str)
                } else {
                    Some(input)
                };
                let Some(pattern) = target else {
                    println!("无效编号: {input}");
                    continue;
                };
                if whitelist::remove_clean(pattern)? {
                    println!("已移除: {pattern}");
                } else {
                    println!("未找到: {pattern}");
                }
            }
            _ => println!("未知操作，请输入 a / r / q"),
        }
        writeln!(stdout)?;
    }
    Ok(())
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
