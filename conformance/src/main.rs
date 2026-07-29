//! 一致性 harness：在同一 fixture 上分别驱动 mole 与 vole，比对候选集。
//!
//! 本阶段 vole 侧候选集恒为空（规则引擎属于 Phase 4），所以 diff 必然非空。
//! 这是预期结果——目的是证明链路通了，而不是证明两者一致。
#![forbid(unsafe_code)]

mod clean_fixture;
mod fixture;
mod guard;

use std::collections::BTreeSet;
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
///
/// 与 `vole_proto::Candidate` 同名容易混淆，故另起名字：那个用 `PathBuf` 且是
/// 协议类型，这个是 harness 内部的可比形式。
///
/// 设计文档 7A 要求比对路径、标签、归属规则、体积、跳过原因五个维度；
/// 本阶段只有前两个可得（mole 补丁只吐这两项），其余随 Phase 4 的规则引擎接入。
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedCandidate {
    path: String,
    label: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fixture_path = arg(&args, "--fixture").expect("需要 --fixture <path>");
    let mole = arg(&args, "--mole").expect("需要 --mole <path到 bin/clean.sh>");
    let vole = arg(&args, "--vole").expect("需要 --vole <path到 vole 二进制>");

    let root = std::env::var("VOLE_TEST_ROOT")
        .expect("必须设置 VOLE_TEST_ROOT。见设计文档 7.0：不要在开发机上跑。");
    let root = PathBuf::from(root);

    let fx: fixture::Fixture =
        serde_json::from_reader(std::fs::File::open(&fixture_path).expect("打不开 fixture"))
            .expect("fixture 不是合法 JSON");

    let real_home = PathBuf::from(std::env::var("HOME").expect("HOME 未设置"));
    let sentinels = vec![
        real_home.join("Library/Caches"),
        real_home.join("Library/Logs"),
        real_home.join("Desktop"),
        real_home.join("Documents"),
    ];
    let guard = guard::Guard::new(&root, &sentinels).expect("护栏初始化失败");

    let mole_set = run_mole(&mole, &root, &fx);
    let vole_set = run_vole(&vole, &root, &fx);

    if let Err(v) = guard.assert_no_outside_changes() {
        eprintln!("护栏触发：{:?} 发生 {:?}，中止。", v.path, v.kind);
        std::process::exit(2);
    }

    report_diff(&mole_set, &vole_set);
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn fresh_home(root: &Path, tag: &str, fx: &fixture::Fixture) -> PathBuf {
    let home = root.join(format!("{}-{}", fx.id, tag));
    std::fs::remove_dir_all(&home).ok();
    std::fs::create_dir_all(&home).expect("建 HOME 失败");
    fx.materialize(&home).expect("物化 fixture 失败");
    home
}

fn run_mole(mole: &str, root: &Path, fx: &fixture::Fixture) -> BTreeSet<NormalizedCandidate> {
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

fn run_vole(vole: &str, root: &Path, fx: &fixture::Fixture) -> BTreeSet<NormalizedCandidate> {
    let home = fresh_home(root, "vole", fx);
    let out = home.join("candidates.ndjson");

    let output = Command::new(vole)
        .args(["clean", "--plan", "--json-stream"])
        .env("HOME", &home)
        .output()
        .expect("启动 vole 失败");
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
