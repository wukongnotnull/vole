//! vole 命令行入口。本阶段只有能让一致性 harness 打通链路的最小骨架。
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

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
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Clean { plan, json_stream } => cmd_clean(plan, json_stream),
    }
}

fn cmd_clean(plan: bool, json_stream: bool) {
    // 规则引擎属于 Phase 4。本阶段候选集恒为空，目的是让 harness 有可驱动的对象。
    if plan && json_stream {
        println!(
            r#"{{"schema_version":{},"type":"done","candidates":0}}"#,
            vole_core::vole_proto::SCHEMA_VERSION
        );
    }
}
