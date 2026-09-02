//! `clean` 内只读 hints（Mole `lib/clean/hints.sh` 主路径子集）。
//!
//! 禁止删除；超时/错误跳过提示，不阻塞 clean。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

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
    LaunchAgents,
    OrphanDotdirs,
}

impl HintKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HintKind::ProjectArtifacts => "project_artifacts",
            HintKind::SystemData => "system_data",
            HintKind::LaunchAgents => "launch_agents",
            HintKind::OrphanDotdirs => "orphan_dotdirs",
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
    pub launch_agents_scan_skipped: bool,
    pub orphan_dotdirs_scan_skipped: bool,
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

pub trait BundleExists: Send + Sync {
    fn exists(&self, bundle_id: &str) -> bool;
}

pub struct LiveBundleExists;

impl BundleExists for LiveBundleExists {
    fn exists(&self, bundle_id: &str) -> bool {
        live_bundle_id_exists(bundle_id)
    }
}

pub trait CommandExists: Send + Sync {
    fn exists(&self, name: &str) -> bool;
}

#[allow(dead_code)]
pub struct LiveCommandExists;

impl CommandExists for LiveCommandExists {
    fn exists(&self, name: &str) -> bool {
        std::process::Command::new("command")
            .args(["-v", name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

pub struct CleanHintsOptions<'a> {
    pub home: &'a Path,
    pub search_roots: Option<&'a [PathBuf]>,
    pub budget: Duration,
    pub list_timeout: Duration,
    pub du_timeout: Duration,
    pub size_probe: Option<Arc<dyn PathSizeKb>>,
    pub bundle_exists: Option<Arc<dyn BundleExists>>,
    pub command_exists: Option<Arc<dyn CommandExists>>,
    pub gui_app_texts: Option<String>,
    pub claude_plugin_tokens: Option<String>,
    pub whitelist_patterns: &'a [String],
    pub now: SystemTime,
    pub orphan_age_days: u64,
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
            bundle_exists: None,
            command_exists: None,
            gui_app_texts: None,
            claude_plugin_tokens: None,
            whitelist_patterns: &[],
            now: SystemTime::now(),
            orphan_age_days: std::env::var("VOLE_DOTDIR_ORPHAN_AGE_DAYS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60),
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
    let deadline = Instant::now() + opts.budget;
    let bundle: Arc<dyn BundleExists> = opts
        .bundle_exists
        .clone()
        .unwrap_or_else(|| Arc::new(LiveBundleExists));

    if let Some(item) = probe_project_artifacts(
        opts,
        probe.as_ref(),
        &mut out.project_scan_skipped,
        deadline,
    ) {
        out.items.push(item);
    } else if out.project_scan_skipped {
        out.items.push(HintItem {
            kind: HintKind::ProjectArtifacts,
            summary: "Build artifacts · scan skipped · vole purge".into(),
            detail: None,
        });
    }

    out.items.extend(probe_system_data(
        opts.home,
        opts.du_timeout,
        probe.as_ref(),
    ));

    let items = probe_launch_agents(
        opts.home,
        deadline,
        bundle.as_ref(),
        &mut out.launch_agents_scan_skipped,
    );
    if items.is_empty() && out.launch_agents_scan_skipped {
        out.items.push(HintItem {
            kind: HintKind::LaunchAgents,
            summary: "Stale login items · scan skipped".into(),
            detail: None,
        });
    } else {
        out.items.extend(items);
    }
    out
}

fn probe_project_artifacts(
    opts: &CleanHintsOptions<'_>,
    probe: &dyn PathSizeKb,
    scan_skipped: &mut bool,
    deadline: Instant,
) -> Option<HintItem> {
    let targets = quick_hint_target_names();
    let roots: Vec<PathBuf> = match opts.search_roots {
        Some(r) => r.to_vec(),
        None => quick_hint_search_roots(opts.home),
    };
    if roots.is_empty() {
        return None;
    }

    let max_per_root = MAX_PROJECTS.div_ceil(roots.len()).max(25);

    let mut accum = ProjectHintAccum::default();
    let mut truncated = false;
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
            accum.record(root, &targets, opts.home, opts.du_timeout, probe);
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

            accum.record(&project_dir, &targets, opts.home, opts.du_timeout, probe);

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
                accum.record(&nested_dir, &targets, opts.home, opts.du_timeout, probe);
            }
            if stop {
                break;
            }
        }
        if stop {
            break;
        }
    }

    if accum.count > MAX_MATCH_DISPLAY {
        truncated = true;
    }
    if accum.count == 0 {
        return None;
    }

    let count_label = if truncated {
        format!("{}+", accum.count)
    } else {
        accum.count.to_string()
    };
    let review = if accum.estimate_samples > 0 && accum.estimated_kb == 0 {
        "vole purge --include-empty"
    } else {
        "vole purge"
    };

    let mut detail = format!("{count_label} dirs");
    if accum.estimate_samples > 0 {
        let human = units::bytes_bin(accum.estimated_kb.saturating_mul(1024));
        let partial = accum.estimate_partial || truncated || accum.estimate_samples < accum.count;
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
        detail: if accum.examples.is_empty() {
            None
        } else {
            Some(accum.examples.join(", "))
        },
    })
}

#[derive(Default)]
struct ProjectHintAccum {
    count: usize,
    estimated_kb: u64,
    estimate_samples: usize,
    estimate_partial: bool,
    examples: Vec<String>,
}

impl ProjectHintAccum {
    fn record(
        &mut self,
        parent: &Path,
        targets: &[&str],
        home: &Path,
        du_timeout: Duration,
        probe: &dyn PathSizeKb,
    ) {
        for target in targets {
            let candidate = parent.join(target);
            if !candidate.is_dir() {
                continue;
            }
            self.count += 1;
            if self.examples.len() < 2 {
                self.examples.push(display_under_home(&candidate, home));
            }
            if self.estimate_samples >= MAX_SIZE_SAMPLES {
                self.estimate_partial = true;
                continue;
            }
            match probe.size_kb(&candidate, du_timeout) {
                Some(kb) => {
                    self.estimated_kb = self.estimated_kb.saturating_add(kb);
                    self.estimate_samples += 1;
                }
                None => self.estimate_partial = true,
            }
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

const LAUNCH_AGENT_MAX_HITS: usize = 3;

fn probe_launch_agents(
    home: &Path,
    deadline: Instant,
    bundle: &dyn BundleExists,
    scan_skipped: &mut bool,
) -> Vec<HintItem> {
    let dir = home.join("Library/LaunchAgents");
    if !dir.is_dir() {
        return Vec::new();
    }
    if Instant::now() >= deadline {
        *scan_skipped = true;
        return Vec::new();
    }
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => {
            *scan_skipped = true;
            return Vec::new();
        }
    };
    let mut items = Vec::new();
    for ent in rd.flatten() {
        if Instant::now() >= deadline {
            *scan_skipped = true;
            break;
        }
        let path = ent.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.ends_with(".plist") || name.starts_with("com.apple.") {
            continue;
        }
        match classify_launch_agent(&path, home, bundle) {
            Ok(Some((reason, target))) => {
                items.push(HintItem {
                    kind: HintKind::LaunchAgents,
                    summary: format!("Stale login item · {name} · {reason}: {target}"),
                    detail: None,
                });
                if items.len() >= LAUNCH_AGENT_MAX_HITS {
                    break;
                }
            }
            Ok(None) => {}
            Err(()) => {
                *scan_skipped = true;
            }
        }
    }
    items
}

fn classify_launch_agent(
    plist: &Path,
    home: &Path,
    bundle: &dyn BundleExists,
) -> Result<Option<(&'static str, String)>, ()> {
    let value = read_plist_value(plist)?;
    let dict = value.as_dictionary().ok_or(())?;
    let program = hint_extract_launch_agent_program_path(dict);
    if program.is_none() && dict.get("MachServices").is_some() {
        return Ok(None);
    }
    if let Some(ref program) = program {
        if hint_is_system_binary(program) {
            return Ok(None);
        }
        if program.starts_with('/') && Path::new(program).is_file() && is_executable(Path::new(program))
        {
            return Ok(None);
        }
        if hint_is_app_scoped_launch_target(program, home) {
            let target = display_under_home(Path::new(program), home);
            if !Path::new(program).exists() {
                return Ok(Some(("Missing app/helper target", target)));
            }
            if !Path::new(program).is_file() || !is_executable(Path::new(program)) {
                return Ok(Some(("Program target is not executable", target)));
            }
            return Ok(None);
        }
    }
    if let Some(associated) = hint_extract_associated_bundle(dict) {
        if !bundle.exists(&associated) {
            return Ok(Some(("Associated app not found", associated)));
        }
    }
    Ok(None)
}

fn read_plist_value(path: &Path) -> Result<plist::Value, ()> {
    let data = fs::read(path).map_err(|_| ())?;
    plist::Value::from_reader(std::io::Cursor::new(data)).map_err(|_| ())
}

fn hint_extract_launch_agent_program_path(dict: &plist::Dictionary) -> Option<String> {
    if let Some(s) = dict.get("Program").and_then(plist::Value::as_string) {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    dict.get("ProgramArguments")
        .and_then(plist::Value::as_array)
        .and_then(|a| a.first())
        .and_then(plist::Value::as_string)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn hint_is_system_binary(program: &str) -> bool {
    program.starts_with("/bin/")
        || program.starts_with("/sbin/")
        || program.starts_with("/usr/bin/")
        || program.starts_with("/usr/sbin/")
        || program.starts_with("/usr/libexec/")
}

fn hint_is_app_scoped_launch_target(program: &str, home: &Path) -> bool {
    let home_apps = home.join("Applications");
    let home_as = home.join("Library/Application Support");
    program.starts_with("/Applications/Setapp/") && program.contains(".app/")
        || (program.starts_with("/Applications/") && program.contains(".app/"))
        || (program.starts_with(&format!("{}/", home_apps.display())) && program.contains(".app/"))
        || (program.starts_with(&format!("{}/", home_as.display())) && program.contains(".app/"))
        || program.starts_with("/Library/Input Methods/")
        || program.starts_with("/Library/PrivilegedHelperTools/")
}

fn hint_extract_associated_bundle(dict: &plist::Dictionary) -> Option<String> {
    let raw = dict.get("AssociatedBundleIdentifiers")?;
    if let Some(arr) = raw.as_array() {
        return arr
            .first()
            .and_then(plist::Value::as_string)
            .filter(|s| !s.is_empty() && *s != "1")
            .map(str::to_string);
    }
    raw.as_string()
        .filter(|s| !s.is_empty() && *s != "1" && !s.starts_with('{') && !s.starts_with('['))
        .map(str::to_string)
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn live_bundle_id_exists(bundle_id: &str) -> bool {
    if bundle_id.is_empty() {
        return false;
    }
    let roots = [
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Setapp"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/Library/Input Methods"),
    ];
    for root in roots {
        if scan_apps_for_bundle(&root, bundle_id) {
            return true;
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if scan_apps_for_bundle(&home.join("Applications"), bundle_id) {
            return true;
        }
        if scan_apps_for_bundle(&home.join("Library/Input Methods"), bundle_id) {
            return true;
        }
    }
    false
}

fn scan_apps_for_bundle(root: &Path, bundle_id: &str) -> bool {
    let Ok(rd) = fs::read_dir(root) else {
        return false;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.extension().and_then(|s| s.to_str()) != Some("app") {
            continue;
        }
        let info = p.join("Contents/Info.plist");
        let Ok(value) = read_plist_value(&info) else {
            continue;
        };
        if value
            .as_dictionary()
            .and_then(|d| d.get("CFBundleIdentifier"))
            .and_then(plist::Value::as_string)
            == Some(bundle_id)
        {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn probe_orphan_dotdirs(
    _opts: &CleanHintsOptions<'_>,
    _deadline: Instant,
    _scan_skipped: &mut bool,
) -> Vec<HintItem> {
    Vec::new()
}

fn list_child_dirs(
    parent: &Path,
    _timeout: Duration,
    deadline: Instant,
) -> Result<Vec<PathBuf>, ()> {
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
    use std::time::SystemTime;

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
            bundle_exists: None,
            command_exists: None,
            gui_app_texts: Some(String::new()),
            claude_plugin_tokens: Some(String::new()),
            whitelist_patterns: &[],
            now: SystemTime::now(),
            orphan_age_days: 60,
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
            bundle_exists: None,
            command_exists: None,
            gui_app_texts: Some(String::new()),
            claude_plugin_tokens: Some(String::new()),
            whitelist_patterns: &[],
            now: SystemTime::now(),
            orphan_age_days: 60,
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
            bundle_exists: None,
            command_exists: None,
            gui_app_texts: Some(String::new()),
            claude_plugin_tokens: Some(String::new()),
            whitelist_patterns: &[],
            now: SystemTime::now(),
            orphan_age_days: 60,
        });
        assert!(!small.items.iter().any(|h| h.kind == HintKind::SystemData));

        let large = collect_clean_hints(&CleanHintsOptions {
            home: home.path(),
            search_roots: Some(&[]),
            budget: Duration::from_secs(1),
            list_timeout: Duration::from_millis(100),
            du_timeout: Duration::from_millis(100),
            size_probe: Some(Arc::new(FixedSize {
                kb: Mutex::new(SYSTEM_DATA_MIN_KB),
            })),
            bundle_exists: None,
            command_exists: None,
            gui_app_texts: Some(String::new()),
            claude_plugin_tokens: Some(String::new()),
            whitelist_patterns: &[],
            now: SystemTime::now(),
            orphan_age_days: 60,
        });
        let item = large
            .items
            .iter()
            .find(|h| h.kind == HintKind::SystemData)
            .expect("system data hint");
        assert!(item.summary.contains("DerivedData") || item.summary.contains("Xcode"));
    }

    fn write_launch_plist(path: &Path, body: &str) {
        fs::write(
            path,
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
{body}
</dict>
</plist>
"#
            ),
        )
        .unwrap();
    }

    struct NoBundles;
    impl BundleExists for NoBundles {
        fn exists(&self, _bundle_id: &str) -> bool {
            false
        }
    }

    struct AllBundles;
    impl BundleExists for AllBundles {
        fn exists(&self, _bundle_id: &str) -> bool {
            true
        }
    }

    fn hints_opts<'a>(
        home: &'a Path,
        bundle: Arc<dyn BundleExists>,
        budget: Duration,
    ) -> CleanHintsOptions<'a> {
        CleanHintsOptions {
            home,
            search_roots: Some(&[]),
            budget,
            list_timeout: Duration::from_millis(100),
            du_timeout: Duration::from_millis(100),
            size_probe: Some(Arc::new(FixedSize { kb: Mutex::new(1) })),
            bundle_exists: Some(bundle),
            command_exists: None,
            gui_app_texts: Some(String::new()),
            claude_plugin_tokens: Some(String::new()),
            whitelist_patterns: &[],
            now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000),
            orphan_age_days: 60,
        }
    }

    #[test]
    fn launch_agent_kind_string_is_stable() {
        assert_eq!(HintKind::LaunchAgents.as_str(), "launch_agents");
    }

    #[test]
    fn launch_agent_reports_missing_app_backed_target() {
        let home = tempfile::tempdir().unwrap();
        let agents = home.path().join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        write_launch_plist(
            &agents.join("com.example.stale.plist"),
            r#"
  <key>Label</key><string>com.example.stale</string>
  <key>ProgramArguments</key>
  <array><string>/Applications/Missing.app/Contents/MacOS/Missing</string></array>
"#,
        );
        let hints = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(NoBundles),
            Duration::from_secs(15),
        ));
        let item = hints
            .items
            .iter()
            .find(|h| h.kind == HintKind::LaunchAgents)
            .expect("launch_agents hint");
        assert!(item.summary.contains("Stale login item"));
        assert!(item.summary.contains("com.example.stale.plist"));
        assert!(item.summary.contains("Missing app/helper target"));
        assert!(!hints.launch_agents_scan_skipped);
    }

    #[test]
    fn launch_agent_program_beats_program_arguments() {
        let home = tempfile::tempdir().unwrap();
        let agents = home.path().join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        write_launch_plist(
            &agents.join("com.example.program-precedence.plist"),
            r#"
  <key>Label</key><string>com.example.program-precedence</string>
  <key>Program</key><string>/Applications/Missing.app/Contents/MacOS/Missing</string>
  <key>ProgramArguments</key>
  <array><string>/bin/bash</string><string>--version</string></array>
"#,
        );
        let hints = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(NoBundles),
            Duration::from_secs(15),
        ));
        let item = hints
            .items
            .iter()
            .find(|h| h.kind == HintKind::LaunchAgents)
            .expect("launch_agents hint");
        assert!(item.summary.contains("Missing.app"));
        assert!(!item.summary.contains("/bin/bash"));
    }

    #[test]
    fn launch_agent_skips_mach_services_only_and_system_binary() {
        let home = tempfile::tempdir().unwrap();
        let agents = home.path().join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        write_launch_plist(
            &agents.join("com.google.keystone.agent.plist"),
            r#"
  <key>Label</key><string>com.google.keystone.agent</string>
  <key>MachServices</key><dict><key>com.google.Keystone.Agent</key><true/></dict>
"#,
        );
        write_launch_plist(
            &agents.join("com.example.shell.plist"),
            r#"
  <key>Label</key><string>com.example.shell</string>
  <key>ProgramArguments</key>
  <array><string>/bin/bash</string><string>-c</string><string>true</string></array>
"#,
        );
        write_launch_plist(
            &agents.join("com.apple.keep.plist"),
            r#"
  <key>Label</key><string>com.apple.keep</string>
  <key>Program</key><string>/Applications/Missing.app/Contents/MacOS/Missing</string>
"#,
        );
        let hints = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(NoBundles),
            Duration::from_secs(15),
        ));
        assert!(!hints.items.iter().any(|h| h.kind == HintKind::LaunchAgents));
        assert!(!hints.launch_agents_scan_skipped);
    }

    #[test]
    fn launch_agent_trusts_existing_executable_and_flags_non_exec_or_missing() {
        let home = tempfile::tempdir().unwrap();
        let agents = home.path().join("Library/LaunchAgents");
        let updater_dir = home
            .path()
            .join("Library/Application Support/Google/GoogleUpdater/GoogleUpdater.app/Contents/MacOS");
        fs::create_dir_all(&agents).unwrap();
        fs::create_dir_all(&updater_dir).unwrap();
        let updater = updater_dir.join("GoogleUpdater");
        fs::write(&updater, b"x").unwrap();
        let mut perms = fs::metadata(&updater).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
            fs::set_permissions(&updater, perms).unwrap();
        }
        let body = format!(
            r#"
  <key>Label</key><string>com.google.GoogleUpdater.wake</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>--wake</string></array>
  <key>AssociatedBundleIdentifiers</key>
  <array><string>com.google.GoogleUpdater</string></array>
"#,
            updater.display()
        );
        write_launch_plist(&agents.join("com.google.GoogleUpdater.wake.plist"), &body);

        let live = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(AllBundles),
            Duration::from_secs(15),
        ));
        assert!(!live.items.iter().any(|h| h.kind == HintKind::LaunchAgents));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(&updater).unwrap().permissions();
            p.set_mode(0o644);
            fs::set_permissions(&updater, p).unwrap();
        }
        let not_exec = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(AllBundles),
            Duration::from_secs(15),
        ));
        let item = not_exec
            .items
            .iter()
            .find(|h| h.kind == HintKind::LaunchAgents)
            .expect("non-exec");
        assert!(item.summary.contains("Program target is not executable"));

        fs::remove_file(&updater).unwrap();
        let missing = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(AllBundles),
            Duration::from_secs(15),
        ));
        let item = missing
            .items
            .iter()
            .find(|h| h.kind == HintKind::LaunchAgents)
            .expect("missing");
        assert!(item.summary.contains("Missing app/helper target"));
    }

    #[test]
    fn launch_agent_associated_bundle_missing() {
        let home = tempfile::tempdir().unwrap();
        let agents = home.path().join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        write_launch_plist(
            &agents.join("com.example.goneapp.plist"),
            r#"
  <key>Label</key><string>com.example.goneapp</string>
  <key>Program</key><string>/usr/local/libexec/custom-helper</string>
  <key>AssociatedBundleIdentifiers</key>
  <array><string>com.example.GoneApp</string></array>
"#,
        );
        let hints = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(NoBundles),
            Duration::from_secs(15),
        ));
        let item = hints
            .items
            .iter()
            .find(|h| h.kind == HintKind::LaunchAgents)
            .expect("associated");
        assert!(item.summary.contains("Associated app not found"));
        assert!(item.summary.contains("com.example.GoneApp"));
    }

    #[test]
    fn launch_agent_zero_budget_marks_skipped_and_one_skip_line() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/LaunchAgents")).unwrap();
        write_launch_plist(
            &home.path().join("Library/LaunchAgents/com.example.stale.plist"),
            r#"
  <key>Label</key><string>com.example.stale</string>
  <key>Program</key><string>/Applications/Missing.app/Contents/MacOS/Missing</string>
"#,
        );
        let hints = collect_clean_hints(&hints_opts(
            home.path(),
            Arc::new(NoBundles),
            Duration::ZERO,
        ));
        assert!(hints.launch_agents_scan_skipped);
        let skips: Vec<_> = hints
            .items
            .iter()
            .filter(|h| h.kind == HintKind::LaunchAgents)
            .collect();
        assert_eq!(skips.len(), 1);
        assert_eq!(skips[0].summary, "Stale login items · scan skipped");
    }
}
