//! 一致性 harness：在同一 fixture 上分别驱动 mole 与 vole，比对候选集。
#![forbid(unsafe_code)]

mod clean_fixture;
mod fixture;
mod guard;

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CandidateLine {
    #[serde(rename = "type")]
    kind: String,
    path: Option<String>,
    label: Option<String>,
}

/// 比对单元，路径已归一化成 `$HOME/…` 形式。
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedCandidate {
    path: String,
    label: String,
}

/// Legacy `entries[]` smoke fixtures or extracted `fixture[]` clean rule fixtures.
enum LoadedFixture {
    Legacy(fixture::Fixture),
    Clean(clean_fixture::CleanFixture),
}

impl LoadedFixture {
    fn id(&self) -> &str {
        match self {
            LoadedFixture::Legacy(f) => &f.id,
            LoadedFixture::Clean(f) => &f.id,
        }
    }

    fn materialize(&self, home: &Path) -> io::Result<()> {
        match self {
            LoadedFixture::Legacy(f) => f.materialize(home),
            LoadedFixture::Clean(f) => f.materialize(home),
        }
    }
}

fn load_fixture(path: &Path) -> LoadedFixture {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("打不开 fixture {}: {e}", path.display());
    });
    let value: serde_json::Value = serde_json::from_str(&text).expect("fixture 不是合法 JSON");
    if value.get("fixture").is_some() {
        LoadedFixture::Clean(
            clean_fixture::CleanFixture::load(path).expect("CleanFixture 解析失败"),
        )
    } else if value.get("entries").is_some() {
        LoadedFixture::Legacy(serde_json::from_value(value).expect("Fixture 解析失败"))
    } else {
        panic!("fixture {} 缺少 fixture[] 或 entries[]", path.display());
    }
}

/// Marker files under real `$HOME` — stable sentinels that are not churned by other apps.
fn conformance_sentinels(real_home: &Path) -> Vec<PathBuf> {
    const MARKER: &[u8] = b"vole-conformance-guard";
    let dirs = [
        real_home.join("Library/Caches"),
        real_home.join("Library/Logs"),
        real_home.join("Desktop"),
        real_home.join("Documents"),
    ];
    dirs.into_iter()
        .map(|dir| {
            std::fs::create_dir_all(&dir).expect("create sentinel parent dir");
            let path = dir.join(".vole-conformance-sentinel");
            std::fs::write(&path, MARKER).expect("write conformance sentinel");
            path
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fixture_path = arg(&args, "--fixture").expect("需要 --fixture <path>");
    let mole = arg(&args, "--mole").expect("需要 --mole <path到 bin/clean.sh>");
    let vole = arg(&args, "--vole").expect("需要 --vole <path到 vole 二进制>");

    let root = std::env::var("VOLE_TEST_ROOT")
        .expect("必须设置 VOLE_TEST_ROOT。见设计文档 7.0：不要在开发机上跑。");
    let root = PathBuf::from(root);

    let fx = load_fixture(Path::new(&fixture_path));

    let real_home = PathBuf::from(std::env::var("HOME").expect("HOME 未设置"));
    let sentinels = conformance_sentinels(&real_home);
    let guard = guard::Guard::new(&root, &sentinels).expect("护栏初始化失败");

    let mole_set = run_mole(&mole, &root, &fx);
    let vole_set = run_vole(&vole, &root, &fx);

    if let Err(v) = guard.assert_no_outside_changes() {
        eprintln!("护栏触发：{:?} 发生 {:?}，中止。", v.path, v.kind);
        std::process::exit(2);
    }

    match &fx {
        LoadedFixture::Clean(cfx) => report_clean_fixture(&mole_set, &vole_set, cfx),
        LoadedFixture::Legacy(_) => report_diff(&mole_set, &vole_set),
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn fresh_home(root: &Path, tag: &str, fx: &LoadedFixture) -> PathBuf {
    let home = root.join(format!("{}-{}", fx.id(), tag));
    std::fs::remove_dir_all(&home).ok();
    std::fs::create_dir_all(&home).expect("建 HOME 失败");
    fx.materialize(&home).expect("物化 fixture 失败");
    home
}

fn run_mole(mole: &str, root: &Path, fx: &LoadedFixture) -> BTreeSet<NormalizedCandidate> {
    let home = fresh_home(root, "mole", fx);
    let out = home.join("candidates.ndjson");

    let status = Command::new(mole)
        .arg("--dry-run")
        .env("HOME", &home)
        .env("VOLE_CONFORMANCE_OUT", &out)
        .env("MOLE_TEST_NO_AUTH", "1")
        .env("MO_NO_OPLOG", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("启动 mole 失败");
    // mole 的退出码在部分环境下非 0（缺 gtimeout 等），不作为判据。
    eprintln!("mole 退出码 {:?}", status.code());

    parse_ndjson(&out, &home)
}

fn run_vole(vole: &str, root: &Path, fx: &LoadedFixture) -> BTreeSet<NormalizedCandidate> {
    let home = fresh_home(root, "vole", fx);
    let out = home.join("candidates.ndjson");

    let mut cmd = Command::new(vole);
    cmd.args(["clean", "--plan", "--json-stream"])
        .env("HOME", &home)
        .env("VOLE_TEST_HOME", &home);
    if let Ok(rules_dir) = std::env::var("VOLE_RULES_DIR") {
        cmd.env("VOLE_RULES_DIR", rules_dir);
    }

    let output = cmd.output().expect("启动 vole 失败");
    std::fs::write(&out, &output.stdout).expect("写 vole 输出失败");

    parse_ndjson(&out, &home)
}

fn parse_ndjson(path: &Path, home: &Path) -> BTreeSet<NormalizedCandidate> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let home_str = home.to_string_lossy().to_string();
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<CandidateLine>(l).ok())
        .filter(|c| c.kind == "candidate")
        .map(|c| NormalizedCandidate {
            // 两侧 HOME 不同目录，必须归一化后才可比。
            path: c.path.unwrap_or_default().replace(&home_str, "$HOME"),
            label: c.label.unwrap_or_default(),
        })
        .collect()
}

fn normalize_fixture_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        format!("$HOME/{rest}")
    } else if path == "~" {
        "$HOME".to_string()
    } else {
        path.to_string()
    }
}

fn report_clean_fixture(
    mole: &BTreeSet<NormalizedCandidate>,
    vole: &BTreeSet<NormalizedCandidate>,
    fx: &clean_fixture::CleanFixture,
) {
    let mut failed = false;

    println!(
        "mole 候选 {} 项，vole 候选 {} 项（按 fixture 期望校验）",
        mole.len(),
        vole.len()
    );

    for expected in &fx.expect_selected {
        let (path, label) = expected.split_once('|').unwrap_or_else(|| {
            panic!(
                "{}: expect_selected entry must be path|label: {expected}",
                fx.id
            )
        });
        let path = normalize_fixture_path(path);

        let vole_ok = vole.iter().any(|c| c.path == path && c.label == label);
        let mole_ok = mole.iter().any(|c| c.path == path);

        if vole_ok {
            println!("  OK vole: {path} | {label}");
        } else {
            eprintln!("  FAIL vole missing: {path} | {label}");
            failed = true;
        }
        if mole_ok {
            println!("  OK mole path: {path}");
        } else {
            eprintln!("  FAIL mole missing path: {path}");
            failed = true;
        }
    }

    for path in &fx.expect_not_selected {
        let path = normalize_fixture_path(path);
        if mole.iter().any(|c| c.path == path) {
            eprintln!("  FAIL mole selected forbidden path: {path}");
            failed = true;
        }
        if vole.iter().any(|c| c.path == path) {
            eprintln!("  FAIL vole selected forbidden path: {path}");
            failed = true;
        }
    }

    if failed {
        eprintln!("\nDIFF: fixture oracle check failed");
        std::process::exit(1);
    }
    println!("\nOK: mole 与 vole 均满足 fixture 期望");
}

fn report_diff(mole: &BTreeSet<NormalizedCandidate>, vole: &BTreeSet<NormalizedCandidate>) {
    let only_mole: Vec<_> = mole.difference(vole).collect();
    let only_vole: Vec<_> = vole.difference(mole).collect();

    println!("mole 候选 {} 项，vole 候选 {} 项", mole.len(), vole.len());
    if !only_mole.is_empty() {
        println!("\n仅 mole 有（{} 项）：", only_mole.len());
        for c in &only_mole {
            println!("  - {} | {}", c.path, c.label);
        }
    }
    if !only_vole.is_empty() {
        println!("\n仅 vole 有（{} 项）：", only_vole.len());
        for c in &only_vole {
            println!("  + {} | {}", c.path, c.label);
        }
    }

    if only_mole.is_empty() && only_vole.is_empty() {
        println!("\nOK: 候选集一致");
    } else {
        println!("\nDIFF: 候选集不一致");
        std::process::exit(1);
    }
}
