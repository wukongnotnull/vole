//! vole 命令行入口。
#![forbid(unsafe_code)]

mod clean;
mod signals;
mod terminal;
mod tui;

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use vole_core::analyze::analyze_directory;
use vole_core::cancel::CancelToken;
use vole_core::status::{CollectionMode, StatusCollector, REFRESH_INTERVAL};
use vole_core::vole_proto::AnalyzeOutput;

#[derive(Parser)]
#[command(name = "vole", version, about = "macOS cleanup and monitoring")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 清理缓存与残留文件。
    Clean {
        /// 只产出候选集，不改动任何文件（默认行为；对齐 mole --dry-run）。
        #[arg(long, conflicts_with = "apply")]
        plan: bool,
        /// 同 `--plan`。
        #[arg(long = "dry-run", short = 'n', conflicts_with = "apply")]
        dry_run: bool,
        /// 执行 plan 文件中的条目（须通过 TTL 与 TOCTOU 重验）。
        #[arg(long, value_name = "PLAN", conflicts_with_all = ["dry_run", "plan_out"])]
        apply: Option<PathBuf>,
        /// 永久删除而非移入废纸篓（仅与 `--apply` 联用）。
        #[arg(long, requires = "apply")]
        permanent: bool,
        /// 输出 JSON 而非人类可读文本。
        #[arg(long)]
        json: bool,
        /// 以 NDJSON 事件流输出到 stdout。
        #[arg(long = "json-stream")]
        json_stream: bool,
        /// 将 plan JSON 写入文件。
        #[arg(long, conflicts_with = "apply")]
        plan_out: Option<PathBuf>,
        /// 交互式管理受保护路径白名单（对齐 mole `clean --whitelist`）。
        #[arg(
            long,
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent"]
        )]
        whitelist: bool,
        /// 向白名单添加路径 pattern（非交互）。
        #[arg(
            long = "whitelist-add",
            value_name = "PATH",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist"]
        )]
        whitelist_add: Option<String>,
        /// 从白名单移除路径 pattern（非交互）。
        #[arg(
            long = "whitelist-remove",
            value_name = "PATH",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist"]
        )]
        whitelist_remove: Option<String>,
        /// 列出当前白名单（非交互）。
        #[arg(
            long = "whitelist-list",
            conflicts_with_all = ["apply", "plan_out", "json_stream", "permanent", "whitelist"]
        )]
        whitelist_list: bool,
    },
    /// 实时系统监控。
    Status {
        /// 输出 JSON 而非 TUI。
        #[arg(long)]
        json: bool,
        /// 连续 NDJSON 流（对齐 mole --watch）。
        #[arg(long = "json-stream")]
        json_stream: bool,
    },
    /// 目录体积分析（对齐 mole analyze）。
    Analyze {
        /// 目标目录（默认 `$HOME`）。
        path: Option<PathBuf>,
        /// 输出 JSON 而非 TUI。
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Clean {
            plan: _,
            dry_run: _,
            apply,
            permanent,
            json,
            json_stream,
            plan_out,
            whitelist,
            whitelist_add,
            whitelist_remove,
            whitelist_list,
        } => {
            let code = clean::run_clean(clean::CleanOptions {
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
        Command::Status { json, json_stream } => {
            if let Err(e) = cmd_status(json, json_stream) {
                eprintln!("vole status: {}", e);
                std::process::exit(1);
            }
        }
        Command::Analyze { path, json } => {
            if let Err(e) = cmd_analyze(path, json) {
                eprintln!("vole analyze: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn should_use_json(force: bool) -> bool {
    if force {
        return true;
    }
    !io::stdout().is_terminal()
}

fn cmd_status(force_json: bool, json_stream: bool) -> io::Result<()> {
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

fn cmd_analyze_tui(initial: &Path, cancel: CancelToken) -> io::Result<()> {
    terminal::install_panic_hook();
    let mut guard = terminal::TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut term = Terminal::new(backend)?;
    let theme = tui::Theme::default();

    let mut stack: Vec<PathBuf> = vec![initial.to_path_buf()];
    let mut selected = 0usize;
    let mut out = AnalyzeOutput::default();
    let mut scanning = true;
    let mut scan_rx: Option<std::sync::mpsc::Receiver<io::Result<AnalyzeOutput>>> = None;

    let poll = Duration::from_millis(33);

    loop {
        if scanning && scan_rx.is_none() {
            let path = stack.last().cloned().unwrap();
            out.path = path.to_string_lossy().into_owned();
            let cancel_scan = cancel.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(analyze_directory(&path, &cancel_scan));
            });
            scan_rx = Some(rx);
        }

        term.draw(|f| tui::render_analyze(f, &out, selected, scanning, &theme))?;

        if let Some(rx) = &scan_rx {
            if let Ok(result) = rx.try_recv() {
                scan_rx = None;
                match result {
                    Ok(snapshot) => {
                        out = snapshot;
                        selected = 0;
                        scanning = false;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        if event::poll(poll)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        cancel.cancel();
                    }
                    KeyCode::Char('q') | KeyCode::Esc if stack.len() <= 1 => cancel.cancel(),
                    KeyCode::Esc if stack.len() > 1 => {
                        stack.pop();
                        scanning = true;
                        scan_rx = None;
                    }
                    KeyCode::Up => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        if selected + 1 < out.entries.len() {
                            selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(entry) = out.entries.get(selected) {
                            if entry.is_dir {
                                stack.push(PathBuf::from(&entry.path));
                                scanning = true;
                                scan_rx = None;
                            }
                        }
                    }
                    _ => {}
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
    let theme = tui::Theme::default();

    let mut collector = StatusCollector::new();
    let mut snap = collector.collect_full().map_err(io::Error::other)?;

    let poll = Duration::from_millis(33);
    let mut last_collect = std::time::Instant::now();
    while !cancel.is_cancelled() {
        if event::poll(poll)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        cancel.cancel();
                    }
                    KeyCode::Char('q') | KeyCode::Esc => cancel.cancel(),
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

        term.draw(|f| tui::render_status(f, &snap, &theme))?;

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
