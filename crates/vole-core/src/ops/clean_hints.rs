//! `clean` 内只读 hints（Mole `lib/clean/hints.sh` 主路径子集）。
//!
//! 禁止删除；超时/错误跳过提示，不阻塞 clean。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::purge_plan::{
    is_project_root_for_hints, quick_hint_search_roots, PURGE_TARGETS, QUICK_HINT_EXCLUDED_TARGETS,
};
use crate::units;

/// 墙钟预算默认（秒），对齐 Mole `MOLE_TIMEOUT_HINT_SCAN_SEC`。
pub const DEFAULT_HINT_SCAN_BUDGET_SECS: u64 = 15;

const MAX_PROJECTS: usize = 200;
const MAX_NESTED_PER_PROJECT: usize = 120;
const MAX_MATCH_DISPLAY: usize = 12;
const MAX_SIZE_SAMPLES: usize = 3;
const SYSTEM_DATA_MIN_KB: u64 = 2 * 1024 * 1024; // 2 GiB
const SYSTEM_DATA_MAX_HITS: usize = 3;

const NEST_SKIP: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    "DerivedData",
    "Pods",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    ProjectArtifacts,
    SystemData,
}

impl HintKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HintKind::ProjectArtifacts => "project_artifacts",
            HintKind::SystemData => "system_data",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintItem {
    pub kind: HintKind,
    pub summary: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanHints {
    pub items: Vec<HintItem>,
    pub project_scan_skipped: bool,
}

pub trait PathSizeKb: Send + Sync {
    fn size_kb(&self, path: &Path, timeout: Duration) -> Option<u64>;
}

#[derive(Debug, Default)]
pub struct DuPathSize;

impl PathSizeKb for DuPathSize {
    fn size_kb(&self, path: &Path, timeout: Duration) -> Option<u64> {
        du_sk_kb(path, timeout)
    }
}

pub struct CleanHintsOptions<'a> {
    pub home: &'a Path,
    pub search_roots: Option<&'a [PathBuf]>,
    pub budget: Duration,
    pub list_timeout: Duration,
    pub du_timeout: Duration,
    pub size_probe: Option<Arc<dyn PathSizeKb>>,
}

impl<'a> CleanHintsOptions<'a> {
    pub fn production(home: &'a Path) -> Self {
        let budget_secs = std::env::var("VOLE_TIMEOUT_HINT_SCAN_SEC")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_HINT_SCAN_BUDGET_SECS);
        Self {
            home,
            search_roots: None,
            budget: Duration::from_secs(budget_secs),
            list_timeout: Duration::from_secs(1),
            du_timeout: Duration::from_millis(800),
            size_probe: None,
        }
    }
}

/// `PURGE_TARGETS` 减去 quick-hint 噪声排除项。
pub fn quick_hint_target_names() -> Vec<&'static str> {
    PURGE_TARGETS
        .iter()
        .copied()
        .filter(|t| !QUICK_HINT_EXCLUDED_TARGETS.contains(t))
        .collect()
}

pub fn collect_clean_hints(opts: &CleanHintsOptions<'_>) -> CleanHints {
    let probe: Arc<dyn PathSizeKb> = opts
        .size_probe
        .clone()
        .unwrap_or_else(|| Arc::new(DuPathSize));
    let mut out = CleanHints::default();

    if let Some(item) = probe_project_artifacts(opts, probe.as_ref(), &mut out.project_scan_skipped)
    {
        out.items.push(item);
    } else if out.project_scan_skipped {
        out.items.push(HintItem {
            kind: HintKind::ProjectArtifacts,
            summary: "Build artifacts · scan skipped · vole purge".into(),
            detail: None,
        });
    }

    out.items
        .extend(probe_system_data(opts.home, opts.du_timeout, probe.as_ref()));
    out
}

fn probe_project_artifacts(
    opts: &CleanHintsOptions<'_>,
    probe: &dyn PathSizeKb,
    scan_skipped: &mut bool,
) -> Option<HintItem> {
    let deadline = Instant::now() + opts.budget;
    let targets = quick_hint_target_names();
    let roots: Vec<PathBuf> = match opts.search_roots {
        Some(r) => r.to_vec(),
        None => quick_hint_search_roots(opts.home),
    };
    if roots.is_empty() {
        return None;
    }

    let max_per_root = ((MAX_PROJECTS + roots.len() - 1) / roots.len()).max(25);

    let mut count = 0usize;
    let mut truncated = false;
    let mut estimated_kb = 0u64;
    let mut estimate_samples = 0usize;
    let mut estimate_partial = false;
    let mut examples: Vec<String> = Vec::new();
    let mut scanned_projects = 0usize;
    let mut stop = false;

    for root in &roots {
        if Instant::now() >= deadline {
            truncated = true;
            *scan_skipped = true;
            break;
        }
        if !root.is_dir() {
            continue;
        }
        let mut root_projects = 0usize;

        if is_project_root_for_hints(root) {
            scanned_projects += 1;
            root_projects += 1;
            if scanned_projects > MAX_PROJECTS {
                truncated = true;
                break;
            }
            record_targets(
                root,
                &targets,
                &mut count,
                &mut estimated_kb,
                &mut estimate_samples,
                &mut estimate_partial,
                &mut examples,
                opts.home,
                opts.du_timeout,
                probe,
            );
        }

        if root_projects >= max_per_root {
            truncated = true;
            continue;
        }

        let children = match list_child_dirs(root, opts.list_timeout, deadline) {
            Ok(c) => c,
            Err(()) => {
                *scan_skipped = true;
                truncated = true;
                continue;
            }
        };

        for project_dir in children {
            if Instant::now() >= deadline {
                truncated = true;
                *scan_skipped = true;
                stop = true;
                break;
            }
            let Some(name) = project_dir.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            if root_projects >= max_per_root {
                truncated = true;
                break;
            }
            scanned_projects += 1;
            root_projects += 1;
            if scanned_projects > MAX_PROJECTS {
                truncated = true;
                stop = true;
                break;
            }

            record_targets(
                &project_dir,
                &targets,
                &mut count,
                &mut estimated_kb,
                &mut estimate_samples,
                &mut estimate_partial,
                &mut examples,
                opts.home,
                opts.du_timeout,
                probe,
            );

            if Instant::now() >= deadline {
                truncated = true;
                *scan_skipped = true;
                stop = true;
                break;
            }

            let nested = match list_child_dirs(&project_dir, opts.list_timeout, deadline) {
                Ok(c) => c,
                Err(()) => {
                    *scan_skipped = true;
                    truncated = true;
                    continue;
                }
            };
            let mut nested_count = 0usize;
            for nested_dir in nested {
                if Instant::now() >= deadline {
                    truncated = true;
                    *scan_skipped = true;
                    stop = true;
                    break;
                }
                let Some(nname) = nested_dir.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if nname.starts_with('.') || NEST_SKIP.contains(&nname) {
                    continue;
                }
                nested_count += 1;
                if nested_count > MAX_NESTED_PER_PROJECT {
                    break;
                }
                record_targets(
                    &nested_dir,
                    &targets,
                    &mut count,
                    &mut estimated_kb,
                    &mut estimate_samples,
                    &mut estimate_partial,
                    &mut examples,
                    opts.home,
                    opts.du_timeout,
                    probe,
                );
            }
            if stop {
                break;
            }
        }
        if stop {
            break;
        }
    }

    if count > MAX_MATCH_DISPLAY {
        truncated = true;
    }
    if count == 0 {
        return None;
    }

    let count_label = if truncated {
        format!("{count}+")
    } else {
        count.to_string()
    };
    let review = if estimate_samples > 0 && estimated_kb == 0 {
        "vole purge --include-empty"
    } else {
        "vole purge"
    };

    let mut detail = format!("{count_label} dirs");
    if estimate_samples > 0 {
        let human = units::bytes_bin(estimated_kb.saturating_mul(1024));
        let partial = estimate_partial || truncated || estimate_samples < count;
        if partial {
            detail.push_str(&format!(", {human}+"));
        } else {
            detail.push_str(&format!(", {human}"));
        }
    }
    let mut summary = format!("Build artifacts · {detail} · {review}");
    if *scan_skipped {
        summary.push_str(" (partial scan)");
    }

    Some(HintItem {
        kind: HintKind::ProjectArtifacts,
        summary,
        detail: if examples.is_empty() {
            None
        } else {
            Some(examples.join(", "))
        },
    })
}

fn record_targets(
    parent: &Path,
    targets: &[&str],
    count: &mut usize,
    estimated_kb: &mut u64,
    estimate_samples: &mut usize,
    estimate_partial: &mut bool,
    examples: &mut Vec<String>,
    home: &Path,
    du_timeout: Duration,
    probe: &dyn PathSizeKb,
) {
    for target in targets {
        let candidate = parent.join(target);
        if !candidate.is_dir() {
            continue;
        }
        *count += 1;
        if examples.len() < 2 {
            examples.push(display_under_home(&candidate, home));
        }
        if *estimate_samples >= MAX_SIZE_SAMPLES {
            *estimate_partial = true;
            continue;
        }
        match probe.size_kb(&candidate, du_timeout) {
            Some(kb) => {
                *estimated_kb = estimated_kb.saturating_add(kb);
                *estimate_samples += 1;
            }
            None => *estimate_partial = true,
        }
    }
}

fn probe_system_data(home: &Path, du_timeout: Duration, probe: &dyn PathSizeKb) -> Vec<HintItem> {
    let mut pairs: Vec<(&str, PathBuf)> = vec![
        (
            "Xcode DerivedData",
            home.join("Library/Developer/Xcode/DerivedData"),
        ),
        (
            "Xcode Archives",
            home.join("Library/Developer/Xcode/Archives"),
        ),
        (
            "iPhone backups",
            home.join("Library/Application Support/MobileSync/Backup"),
        ),
        (
            "Simulator data",
            home.join("Library/Developer/CoreSimulator/Devices"),
        ),
        (
            "Docker Desktop data",
            home.join("Library/Containers/com.docker.docker/Data"),
        ),
        ("Mail data", home.join("Library/Mail")),
    ];

    if let Ok(rd) = std::fs::read_dir(home.join("Library/Group Containers")) {
        for ent in rd.flatten() {
            let data = ent.path().join("data");
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.contains("dev.orbstack") && data.is_dir() {
                pairs.push(("OrbStack data", data));
                break;
            }
        }
    }

    let mut items = Vec::new();
    for (label, path) in pairs {
        if items.len() >= SYSTEM_DATA_MAX_HITS {
            break;
        }
        if !path.is_dir() {
            continue;
        }
        let Some(kb) = probe.size_kb(&path, du_timeout) else {
            continue;
        };
        if kb < SYSTEM_DATA_MIN_KB {
            continue;
        }
        let human = units::bytes_bin(kb.saturating_mul(1024));
        items.push(HintItem {
            kind: HintKind::SystemData,
            summary: format!("{label} · {human} · {}", display_under_home(&path, home)),
            detail: None,
        });
    }
    items
}

fn display_under_home(path: &Path, home: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(home) {
        format!("~/{}", rel.display())
    } else {
        path.display().to_string()
    }
}

fn list_child_dirs(parent: &Path, _timeout: Duration, deadline: Instant) -> Result<Vec<PathBuf>, ()> {
    if Instant::now() >= deadline {
        return Err(());
    }
    let rd = std::fs::read_dir(parent).map_err(|_| ())?;
    let mut out = Vec::new();
    for ent in rd.flatten() {
        if Instant::now() >= deadline {
            return Err(());
        }
        let p = ent.path();
        if p.is_dir() {
            out.push(p);
        }
    }
    Ok(out)
}

fn du_sk_kb(path: &Path, timeout: Duration) -> Option<u64> {
    let mut cmd = Command::new("du");
    cmd.args(["-skP"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let output = child.wait_with_output().ok()?;
                let text = String::from_utf8_lossy(&output.stdout);
                let kb = text.split_whitespace().next()?.parse().ok()?;
                return Some(kb);
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    struct FixedSize {
        kb: Mutex<u64>,
    }

    impl PathSizeKb for FixedSize {
        fn size_kb(&self, _path: &Path, _timeout: Duration) -> Option<u64> {
            Some(*self.kb.lock().unwrap())
        }
    }

    #[test]
    fn quick_hint_targets_exclude_bin_and_vendor() {
        let names = quick_hint_target_names();
        assert!(names.contains(&"node_modules"));
        assert!(!names.contains(&"bin"));
        assert!(!names.contains(&"vendor"));
    }

    #[test]
    fn project_artifact_hint_counts_node_modules_excludes_vendor_bin() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("hints-root");
        fs::create_dir_all(root.join("proj/node_modules")).unwrap();
        fs::create_dir_all(root.join("proj/vendor")).unwrap();
        fs::create_dir_all(root.join("proj/bin")).unwrap();
        fs::write(root.join("proj/package.json"), "{}").unwrap();
        let cfg = home.path().join(".config/vole");
        fs::create_dir_all(&cfg).unwrap();
        fs::write(cfg.join("purge_paths"), format!("{}\n", root.display())).unwrap();

        let hints = collect_clean_hints(&CleanHintsOptions {
            home: home.path(),
            search_roots: None,
            budget: Duration::from_secs(15),
            list_timeout: Duration::from_secs(1),
            du_timeout: Duration::from_millis(800),
            size_probe: Some(Arc::new(FixedSize { kb: Mutex::new(10) })),
        });
        let item = hints
            .items
            .iter()
            .find(|h| h.kind == HintKind::ProjectArtifacts)
            .expect("project artifacts hint");
        assert!(
            item.summary.contains("1") && item.summary.contains("dirs"),
            "summary={}",
            item.summary
        );
        assert!(item.summary.contains("vole purge"));
        assert!(!item.summary.to_lowercase().contains("vendor"));
        let detail = item.detail.as_deref().unwrap_or("");
        assert!(detail.contains("node_modules"), "detail={detail}");
        assert!(!detail.contains("vendor"));
        assert!(!detail.ends_with("/bin") && !detail.contains("/bin,"));
    }

    #[test]
    fn zero_budget_marks_project_scan_skipped() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("hints-root");
        fs::create_dir_all(root.join("proj/node_modules")).unwrap();
        fs::write(root.join("proj/package.json"), "{}").unwrap();
        let cfg = home.path().join(".config/vole");
        fs::create_dir_all(&cfg).unwrap();
        fs::write(cfg.join("purge_paths"), format!("{}\n", root.display())).unwrap();

        let hints = collect_clean_hints(&CleanHintsOptions {
            home: home.path(),
            search_roots: None,
            budget: Duration::ZERO,
            list_timeout: Duration::from_secs(1),
            du_timeout: Duration::from_millis(800),
            size_probe: Some(Arc::new(FixedSize { kb: Mutex::new(1) })),
        });
        assert!(hints.project_scan_skipped);
        assert!(hints
            .items
            .iter()
            .any(|h| h.summary.contains("scan skipped")));
    }

    #[test]
    fn system_data_hint_requires_large_size() {
        let home = tempfile::tempdir().unwrap();
        let dd = home.path().join("Library/Developer/Xcode/DerivedData");
        fs::create_dir_all(&dd).unwrap();

        let small = collect_clean_hints(&CleanHintsOptions {
            home: home.path(),
            search_roots: Some(&[]),
            budget: Duration::from_secs(1),
            list_timeout: Duration::from_millis(100),
            du_timeout: Duration::from_millis(100),
            size_probe: Some(Arc::new(FixedSize {
                kb: Mutex::new(1024),
            })),
        });
        assert!(!small
            .items
            .iter()
            .any(|h| h.kind == HintKind::SystemData));

        let large = collect_clean_hints(&CleanHintsOptions {
            home: home.path(),
            search_roots: Some(&[]),
            budget: Duration::from_secs(1),
            list_timeout: Duration::from_millis(100),
            du_timeout: Duration::from_millis(100),
            size_probe: Some(Arc::new(FixedSize {
                kb: Mutex::new(SYSTEM_DATA_MIN_KB),
            })),
        });
        let item = large
            .items
            .iter()
            .find(|h| h.kind == HintKind::SystemData)
            .expect("system data hint");
        assert!(item.summary.contains("DerivedData") || item.summary.contains("Xcode"));
    }
}
