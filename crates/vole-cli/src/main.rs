//! vole 命令行入口。
#![forbid(unsafe_code)]

mod signals;
mod terminal;
mod tui;

use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use vole_core::cancel::CancelToken;
use vole_core::status::{CollectionMode, StatusCollector, REFRESH_INTERVAL};

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
        /// 只产出候选集，不改动任何文件。
        #[arg(long)]
        plan: bool,
        /// 以 NDJSON 事件流输出到 stdout。
        #[arg(long = "json-stream")]
        json_stream: bool,
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
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Clean { plan, json_stream } => cmd_clean(plan, json_stream),
        Command::Status { json, json_stream } => {
            if let Err(e) = cmd_status(json, json_stream) {
                eprintln!("vole status: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn cmd_clean(plan: bool, json_stream: bool) {
    if plan && json_stream {
        println!(
            r#"{{"schema_version":{},"type":"done","candidates":0}}"#,
            vole_core::vole_proto::SCHEMA_VERSION
        );
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
