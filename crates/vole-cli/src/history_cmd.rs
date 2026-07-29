//! `vole history` — mole-compatible session + deletion audit viewer.

use std::io::{self, Write};

use vole_core::history::{self, normalize_limit, DEFAULT_LIMIT};

pub fn run(json: bool, limit: u32) -> i32 {
    let limit = normalize_limit(if limit == 0 { DEFAULT_LIMIT } else { limit });
    let report = history::load_default();
    if json {
        match serde_json::to_string_pretty(&report.to_json(limit)) {
            Ok(s) => {
                println!("{s}");
                0
            }
            Err(e) => {
                eprintln!("vole history: {e}");
                1
            }
        }
    } else {
        let text = history::render_text(&report, limit);
        let mut out = io::stdout().lock();
        if let Err(e) = out.write_all(text.as_bytes()) {
            eprintln!("vole history: {e}");
            return 1;
        }
        0
    }
}
