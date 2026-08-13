//! `vole clean` 人读 plan：独占目录按应用归类，共享类型目录按系统缓存 / 系统日志 / 临时文件归类。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use vole_core::ops::PlanEntry;
use vole_core::protection::is_reverse_dns_bundle_id;
use vole_core::units;

const JUNK_LABEL_TOKENS: &[&str] = &[
    "cache",
    "caches",
    "data",
    "log",
    "logs",
    "old",
    "version",
    "versions",
    "model",
    "models",
    "temp",
    "tmp",
    "support",
    "application",
    "rebuildable",
    "gpu",
    "metal",
    "stale",
    "user",
    "app",
];

const NAMED_FOLDER_PARENTS: &[&str] = &[
    "caches",
    "logs",
    "containers",
    "group containers",
    "saved application state",
    "application support",
];

const METAL_LEAVES: &[&str] = &[
    "com.apple.metal",
    "com.apple.metalfe",
    "com.apple.gpuarchiver",
];

struct KnownApp {
    needles: &'static [&'static str],
    id: &'static str,
    title: &'static str,
}

const KNOWN_APPS: &[KnownApp] = &[
    KnownApp {
        needles: &[
            "vs code",
            "vscode",
            "com.microsoft.vscode",
            "/code/cache",
            "/code/cacheddata",
            "application support/code",
        ],
        id: "vscode",
        title: "VS Code",
    },
    KnownApp {
        needles: &["xcode", "deriveddata", "com.apple.dt.xcode"],
        id: "xcode",
        title: "Xcode",
    },
    KnownApp {
        needles: &["safari", "com.apple.safari"],
        id: "safari",
        title: "Safari",
    },
    KnownApp {
        needles: &["chrome", "com.google.chrome", "chromium", "/caches/google"],
        id: "chrome",
        title: "Chrome",
    },
    KnownApp {
        needles: &["firefox", "org.mozilla.firefox"],
        id: "firefox",
        title: "Firefox",
    },
    KnownApp {
        needles: &["edge", "com.microsoft.edgemac"],
        id: "edge",
        title: "Edge",
    },
    KnownApp {
        needles: &["claude code", "claude"],
        id: "claude-code",
        title: "Claude Code",
    },
    KnownApp {
        needles: &["ollama"],
        id: "ollama",
        title: "Ollama",
    },
    KnownApp {
        needles: &["cursor"],
        id: "cursor",
        title: "Cursor",
    },
    KnownApp {
        needles: &["slack"],
        id: "slack",
        title: "Slack",
    },
    KnownApp {
        needles: &["discord"],
        id: "discord",
        title: "Discord",
    },
    KnownApp {
        needles: &["wechat", "微信"],
        id: "wechat",
        title: "微信",
    },
    KnownApp {
        needles: &["docker"],
        id: "docker",
        title: "Docker",
    },
    KnownApp {
        needles: &["parallels"],
        id: "parallels",
        title: "Parallels",
    },
    KnownApp {
        needles: &["node", "npm", "yarn", "pnpm", "node_modules"],
        id: "node",
        title: "Node.js",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppGroup {
    pub id: String,
    pub title: String,
}

impl AppGroup {
    pub fn other() -> Self {
        Self {
            id: "other".into(),
            title: "其他".into(),
        }
    }
}

pub struct GroupedEntries<'a> {
    pub app: AppGroup,
    pub entries: Vec<&'a PlanEntry>,
    pub total_bytes: u64,
}

pub fn app_group_for(path: &Path, label: &str, rule_id: &str) -> AppGroup {
    if let Some(group) = group_from_shared_type_dir(path) {
        return group;
    }

    let path_str = path.to_string_lossy();
    let haystack = format!("{path_str} {label} {rule_id}").to_ascii_lowercase();

    for app in KNOWN_APPS {
        if app.needles.iter().any(|n| haystack.contains(n)) {
            return AppGroup {
                id: app.id.to_string(),
                title: app.title.to_string(),
            };
        }
    }

    if let Some(group) = group_from_bundle_token(&path_str) {
        return group;
    }

    if let Some(group) = group_from_named_folder(&path_str) {
        return group;
    }

    if let Some(group) = group_from_label(label) {
        return group;
    }

    AppGroup::other()
}

pub fn group_plan_entries(entries: &[PlanEntry]) -> Vec<GroupedEntries<'_>> {
    let mut buckets: HashMap<String, (AppGroup, Vec<&PlanEntry>, u64)> = HashMap::new();
    for entry in entries {
        let app = app_group_for(&entry.path, &entry.label, &entry.rule_id);
        let bucket = buckets
            .entry(app.id.clone())
            .or_insert_with(|| (app, Vec::new(), 0));
        bucket.1.push(entry);
        bucket.2 = bucket.2.saturating_add(entry.size);
    }

    let mut groups: Vec<GroupedEntries<'_>> = buckets
        .into_values()
        .map(|(app, entries, total_bytes)| GroupedEntries {
            app,
            entries,
            total_bytes,
        })
        .collect();
    groups.sort_by(|lhs, rhs| {
        rhs.total_bytes
            .cmp(&lhs.total_bytes)
            .then_with(|| lhs.app.title.cmp(&rhs.app.title))
            .then_with(|| lhs.app.id.cmp(&rhs.app.id))
    });
    groups
}

pub fn format_grouped_plan_lines(entries: &[PlanEntry]) -> Vec<String> {
    let hyperlink = std::io::IsTerminal::is_terminal(&std::io::stdout());
    format_grouped_plan_lines_with(entries, hyperlink)
}

fn format_grouped_plan_lines_with(entries: &[PlanEntry], hyperlink: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let groups = group_plan_entries(entries);
    let mut type_count = 0usize;
    let mut total_bytes = 0u64;
    for group in &groups {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        let rows = type_dir_rows(&group.entries);
        type_count = type_count.saturating_add(rows.len());
        total_bytes = total_bytes.saturating_add(group.total_bytes);
        lines.push(format!(
            "{}  ({} {} · {})",
            group.app.title,
            rows.len(),
            if rows.len() == 1 { "type" } else { "types" },
            units::bytes_bin(group.total_bytes)
        ));
        for row in rows {
            let short = shorten_display_path(&row.dir);
            let path_text = if hyperlink {
                osc8_file_hyperlink(&row.dir, &short)
            } else {
                short
            };
            lines.push(format!(
                "  {}  {}  {}",
                row.label,
                path_text,
                units::bytes_bin(row.size)
            ));
        }
    }
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(format_plan_summary(
        groups.len(),
        type_count,
        entries.len(),
        total_bytes,
        hyperlink,
    ));
    lines
}

fn format_plan_summary(
    group_count: usize,
    type_count: usize,
    item_count: usize,
    total_bytes: u64,
    emphasize: bool,
) -> String {
    let size = units::bytes_bin(total_bytes);
    let size = if emphasize {
        format!("\x1b[1;38;2;224;180;86m{size}\x1b[0m")
    } else {
        size
    };
    format!(
        "{size} · {} · {} · {}",
        english_count(group_count, "group", "groups"),
        english_count(type_count, "type", "types"),
        english_count(item_count, "item", "items"),
    )
}

fn english_count(n: usize, singular: &str, plural: &str) -> String {
    format!("{} {}", n, if n == 1 { singular } else { plural })
}

struct TypeDirRow {
    label: String,
    dir: PathBuf,
    size: u64,
}

fn type_dir_rows(entries: &[&PlanEntry]) -> Vec<TypeDirRow> {
    let mut buckets: HashMap<(String, PathBuf), u64> = HashMap::new();
    for entry in entries {
        let key = (entry.label.clone(), type_directory(&entry.path));
        *buckets.entry(key).or_insert(0) += entry.size;
    }
    let mut rows: Vec<TypeDirRow> = buckets
        .into_iter()
        .map(|((label, dir), size)| TypeDirRow { label, dir, size })
        .collect();
    rows.sort_by(|lhs, rhs| {
        rhs.size
            .cmp(&lhs.size)
            .then_with(|| lhs.label.cmp(&rhs.label))
            .then_with(|| lhs.dir.cmp(&rhs.dir))
    });
    rows
}

const TYPE_DIR_NAMES: &[&str] = &[
    "caches",
    "cache",
    "logs",
    "log",
    "tmp",
    "temp",
    "temporaryitems",
];

fn type_directory(path: &Path) -> PathBuf {
    if let Some(bucket) = var_folders_type_bucket(path) {
        return bucket;
    }
    if let Some(dir) = first_type_dir_from_root(path) {
        if is_under_user_library(path) {
            return type_dir_plus_app_child(path, &dir);
        }
        return dir;
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if is_metal_leaf(name) || looks_like_file(name) {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                return parent.to_path_buf();
            }
        }
    }
    path.to_path_buf()
}

fn first_type_dir_from_root(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut acc = PathBuf::new();
    for component in path.components() {
        acc.push(component);
        if let Component::Normal(name) = component {
            if is_type_dir_name(name) {
                return Some(acc);
            }
        }
    }
    None
}

fn is_type_dir_name(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|s| {
        TYPE_DIR_NAMES
            .iter()
            .any(|token| s.eq_ignore_ascii_case(token))
    })
}

fn is_under_user_library(path: &Path) -> bool {
    use std::path::Component;
    let comps: Vec<Component<'_>> = path.components().collect();
    comps.windows(3).any(|window| {
        matches!(window[0], Component::Normal(name) if name == "Users")
            && matches!(window[1], Component::Normal(_))
            && matches!(window[2], Component::Normal(name) if name == "Library")
    })
}

fn type_dir_plus_app_child(path: &Path, type_dir: &Path) -> PathBuf {
    let Ok(rest) = path.strip_prefix(type_dir) else {
        return type_dir.to_path_buf();
    };
    let Some(std::path::Component::Normal(child)) = rest.components().next() else {
        return type_dir.to_path_buf();
    };
    let child_name = child.to_string_lossy();
    if child_name.is_empty() || looks_like_file(&child_name) || is_metal_leaf(&child_name) {
        return type_dir.to_path_buf();
    }
    type_dir.join(child)
}

/// Darwin per-user temp/cache: `/var/folders/<xx>/<hash>/(C|T|X)/…`.
/// `C`/`T`/`X` 才是类型目录，后面的应用子目录不再展开。
fn var_folders_type_bucket(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let comps: Vec<Component<'_>> = path.components().collect();
    let folders_idx = comps
        .iter()
        .position(|c| matches!(c, Component::Normal(name) if *name == "folders"))?;
    if folders_idx == 0 {
        return None;
    }
    if !matches!(
        comps.get(folders_idx - 1),
        Some(Component::Normal(name)) if *name == "var"
    ) {
        return None;
    }
    let bucket_idx = folders_idx.checked_add(3)?;
    match comps.get(bucket_idx) {
        Some(Component::Normal(name)) if matches!(name.to_str(), Some("C" | "T" | "X")) => {}
        _ => return None,
    }
    Some(comps.iter().take(bucket_idx + 1).collect())
}

fn group_from_shared_type_dir(path: &Path) -> Option<AppGroup> {
    if is_under_user_library(path) {
        return None;
    }
    if let Some(bucket) = var_folders_type_bucket(path) {
        let letter = bucket.file_name().and_then(|n| n.to_str()).unwrap_or("C");
        return Some(if letter == "T" {
            shared_tmp_group()
        } else {
            shared_cache_group()
        });
    }
    let dir = type_directory(path);
    if is_under_user_library(&dir) {
        return None;
    }
    match shorten_display_path(&dir).as_str() {
        "/var/log" | "/Library/Logs" => Some(shared_logs_group()),
        "/tmp" | "/var/tmp" => Some(shared_tmp_group()),
        "/Library/Caches" => Some(shared_cache_group()),
        _ => None,
    }
}

fn shared_cache_group() -> AppGroup {
    AppGroup {
        id: "shared-cache".into(),
        title: "系统缓存".into(),
    }
}

fn shared_logs_group() -> AppGroup {
    AppGroup {
        id: "shared-logs".into(),
        title: "系统日志".into(),
    }
}

fn shared_tmp_group() -> AppGroup {
    AppGroup {
        id: "shared-tmp".into(),
        title: "临时文件".into(),
    }
}

fn looks_like_file(name: &str) -> bool {
    const EXTS: &[&str] = &[
        "plist", "log", "tmp", "cache", "old", "txt", "json", "asl", "gz", "crash",
    ];
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

fn shorten_display_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("/private/") {
        format!("/{rest}")
    } else {
        s.into_owned()
    }
}

fn osc8_file_hyperlink(path: &Path, label: &str) -> String {
    format!(
        "\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\",
        file_url_from_path(path),
        label
    )
}

fn file_url_from_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut url = String::from("file://");
    for &b in raw.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                url.push(b as char);
            }
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    url
}

fn group_from_bundle_token(path: &str) -> Option<AppGroup> {
    let token = bundle_token_from_path(path)?;
    let canonical = collapse_helper_suffix(&token);
    let last = canonical
        .rsplit('.')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(&canonical);
    Some(AppGroup {
        id: canonical.to_ascii_lowercase(),
        title: pretty_title(last),
    })
}

fn bundle_token_from_path(path: &str) -> Option<String> {
    let mut parts: Vec<String> = path
        .split('/')
        .filter(|p| !p.is_empty())
        .map(normalize_path_component)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }

    let popped_metal =
        is_metal_leaf(parts.last().map(String::as_str).unwrap_or("")) && parts.len() >= 2;
    if popped_metal {
        parts.pop();
    }

    if popped_metal {
        let leaf = parts.last()?.as_str();
        if !leaf.is_empty() && !is_metal_leaf(leaf) && !is_junk_token(leaf) {
            return Some(leaf.to_string());
        }
    }

    parts
        .into_iter()
        .rev()
        .find(|part| is_bundle_like(part) && !is_metal_leaf(part))
}

fn normalize_path_component(raw: &str) -> String {
    let mut s = raw.to_string();
    for suffix in [".plist", ".savedState", ".app"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.to_string();
            break;
        }
    }
    s
}

fn is_metal_leaf(name: &str) -> bool {
    METAL_LEAVES
        .iter()
        .any(|leaf| name.eq_ignore_ascii_case(leaf))
}

fn is_bundle_like(candidate: &str) -> bool {
    if looks_like_file(candidate) {
        return false;
    }
    let dots = candidate.bytes().filter(|b| *b == b'.').count();
    dots >= 2 && is_reverse_dns_bundle_id(candidate)
}

fn collapse_helper_suffix(bundle: &str) -> String {
    const MARKERS: &[&str] = &[".helper", ".Helper"];
    for marker in MARKERS {
        if let Some(idx) = bundle.find(marker) {
            return bundle[..idx].to_string();
        }
    }
    bundle.to_string()
}

fn group_from_named_folder(path: &str) -> Option<AppGroup> {
    let mut parts: Vec<String> = path
        .split('/')
        .filter(|p| !p.is_empty())
        .map(normalize_path_component)
        .filter(|p| !p.is_empty())
        .collect();
    if is_metal_leaf(parts.last().map(String::as_str).unwrap_or("")) && parts.len() >= 2 {
        parts.pop();
    }
    let leaf = parts.last()?.as_str();
    if leaf.is_empty() || is_metal_leaf(leaf) || is_junk_token(leaf) {
        return None;
    }
    let parent = parts.get(parts.len().wrapping_sub(2)).map(String::as_str)?;
    if !NAMED_FOLDER_PARENTS
        .iter()
        .any(|p| parent.eq_ignore_ascii_case(p))
    {
        return None;
    }
    Some(AppGroup {
        id: leaf.to_ascii_lowercase(),
        title: pretty_title(leaf),
    })
}

fn is_junk_token(token: &str) -> bool {
    JUNK_LABEL_TOKENS
        .iter()
        .any(|j| j.eq_ignore_ascii_case(token))
}

fn group_from_label(label: &str) -> Option<AppGroup> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return None;
    }
    let kept: Vec<&str> = trimmed
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace() || c == ':')
        .map(|t| t.trim_matches(|c: char| c.is_ascii_punctuation()))
        .filter(|t| !t.is_empty() && !is_junk_token(t))
        .collect();
    if kept.is_empty() {
        return None;
    }
    if let Some(token) = kept.iter().copied().find(|t| is_bundle_like(t)) {
        let canonical = collapse_helper_suffix(token);
        let last = canonical.rsplit('.').next().unwrap_or(&canonical);
        return Some(AppGroup {
            id: canonical.to_ascii_lowercase(),
            title: pretty_title(last),
        });
    }
    let title = kept.join(" ");
    if title.eq_ignore_ascii_case("other") {
        return None;
    }
    Some(AppGroup {
        id: title.to_ascii_lowercase(),
        title,
    })
}

fn pretty_title(raw: &str) -> String {
    let mut spaced = String::new();
    let chars: Vec<char> = raw.chars().collect();
    for (i, ch) in chars.iter().copied().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() && chars[i - 1].is_ascii_lowercase() {
            spaced.push(' ');
        }
        spaced.push(ch);
    }
    let mut out = spaced;
    if let Some(first) = out.chars().next() {
        let rest: String = out.chars().skip(1).collect();
        out = format!("{}{rest}", first.to_uppercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(path: &str, label: &str, rule_id: &str, size: u64) -> PlanEntry {
        PlanEntry {
            id: path.to_string(),
            path: PathBuf::from(path),
            label: label.into(),
            size,
            rule_id: rule_id.into(),
            skip_reason: None,
            identity: None,
        }
    }

    #[test]
    fn gpu_metal_cache_groups_under_shared_var_folders_type_dir() {
        let group = app_group_for(
            Path::new("/private/var/folders/zc/x/C/com.example.FooBar/com.apple.metal"),
            "Rebuildable GPU Metal caches",
            "gpu-metal-caches",
        );
        assert_eq!(group.title, "系统缓存");
        assert_eq!(group.id, "shared-cache");
    }

    #[test]
    fn helper_metal_cache_joins_shared_var_folders_group() {
        let group = app_group_for(
            Path::new(
                "/private/var/folders/zc/x/C/com.example.FooBar.helper.GPU/com.apple.metalfe",
            ),
            "Rebuildable GPU Metal caches",
            "gpu-metal-caches",
        );
        assert_eq!(group.id, "shared-cache");
        assert_eq!(group.title, "系统缓存");
    }

    #[test]
    fn chrome_user_cache_stays_separate_from_shared_metal_type_dir() {
        let metal = entry(
            "/private/var/folders/zc/x/C/com.google.Chrome/com.apple.metal",
            "Rebuildable GPU Metal caches",
            "gpu-metal-caches",
            100,
        );
        let cache = entry(
            "/Users/me/Library/Caches/Google",
            "User app cache",
            "user-app-cache",
            200,
        );
        let items = [metal, cache];
        let groups = group_plan_entries(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].app.title, "Chrome");
        assert_eq!(groups[0].entries.len(), 1);
        assert_eq!(groups[0].total_bytes, 200);
        assert_eq!(groups[1].app.title, "系统缓存");
        assert_eq!(groups[1].entries.len(), 1);
        assert_eq!(groups[1].total_bytes, 100);
    }

    #[test]
    fn shared_var_folders_type_dir_merges_across_apps() {
        let items = [
            entry(
                "/private/var/folders/zc/hash/C/com.example.St/com.apple.metal",
                "Rebuildable GPU Metal caches",
                "gpu-metal-caches",
                268,
            ),
            entry(
                "/private/var/folders/zc/hash/C/com.example.Codeswitch/com.apple.metal",
                "Rebuildable GPU Metal caches",
                "gpu-metal-caches",
                264,
            ),
            entry(
                "/private/var/folders/zc/hash/C/com.xunlei.Thunder/com.apple.metal",
                "Rebuildable GPU Metal caches",
                "gpu-metal-caches",
                264,
            ),
        ];
        let lines = format_grouped_plan_lines_with(&items, false);
        assert_eq!(lines[0], "系统缓存  (1 type · 796 B)");
        assert_eq!(
            lines[1],
            "  Rebuildable GPU Metal caches  /var/folders/zc/hash/C  796 B"
        );
        assert_eq!(
            lines.last().map(String::as_str),
            Some("796 B · 1 group · 1 type · 3 items")
        );
        assert!(lines.iter().all(|l| !l.contains("Codeswitch")));
        assert!(lines.iter().all(|l| !l.contains("Thunder")));
    }

    #[test]
    fn format_grouped_plan_lines_ends_with_summary() {
        let items = [
            entry(
                "/Users/me/Library/Caches/Google",
                "User app cache",
                "user-app-cache",
                200,
            ),
            entry(
                "/private/var/folders/zc/x/C/com.google.Chrome/com.apple.metal",
                "Rebuildable GPU Metal caches",
                "gpu-metal-caches",
                100,
            ),
            entry("/private/tmp/orphan-file", "Stale temp", "tmp", 50),
        ];
        let lines = format_grouped_plan_lines_with(&items, false);
        assert_eq!(
            lines.last().map(String::as_str),
            Some("350 B · 3 groups · 3 types · 3 items")
        );
        assert!(
            lines
                .windows(2)
                .any(|w| w[0].is_empty() && w[1].starts_with("350 B")),
            "summary should be separated from the list, got {lines:?}"
        );
    }

    #[test]
    fn format_plan_summary_emphasizes_total_size_when_tty() {
        let items = [entry(
            "/Users/me/Library/Caches/Google",
            "User app cache",
            "user-app-cache",
            200,
        )];
        let lines = format_grouped_plan_lines_with(&items, true);
        let summary = lines.last().expect("summary");
        assert!(
            summary.contains("\x1b[1;38;2;224;180;86m200 B\x1b[0m"),
            "total size should be bold gold, got {summary:?}"
        );
        assert!(summary.contains("1 group · 1 type · 1 item"));
        assert!(!summary.contains("\x1b]8;;"));
    }

    #[test]
    fn groups_sort_by_size_descending() {
        let small = entry(
            "/Users/me/Library/Caches/com.example.SmallApp",
            "User app cache",
            "user-app-cache",
            10,
        );
        let large = entry(
            "/Users/me/Library/Caches/com.example.LargeApp",
            "User app cache",
            "user-app-cache",
            999,
        );
        let items = [small, large];
        let groups = group_plan_entries(&items);
        assert_eq!(groups[0].app.title, "Large App");
        assert_eq!(groups[1].app.title, "Small App");
    }

    #[test]
    fn shared_tmp_goes_to_temp_category() {
        let group = app_group_for(Path::new("/private/tmp/orphan-file"), "Stale temp", "tmp");
        assert_eq!(group.title, "临时文件");
        assert_eq!(group.id, "shared-tmp");
    }

    #[test]
    fn var_folders_t_bucket_goes_to_temp_category() {
        let group = app_group_for(
            Path::new("/private/var/folders/zc/x/T/com.apple.idleassetsd/foo"),
            "Stale temp",
            "tmp",
        );
        assert_eq!(group.title, "临时文件");
        assert_eq!(group.id, "shared-tmp");
    }

    #[test]
    fn system_log_paths_share_one_category() {
        let items = [
            entry(
                "/private/var/log/asl/BB.2027.06.30.G80.asl",
                "System private var logs",
                "private-var-log",
                100,
            ),
            entry(
                "/Library/Logs/DiagnosticReports/App.crash",
                "Diagnostic reports",
                "diagnostic-reports-system",
                50,
            ),
        ];
        let groups = group_plan_entries(&items);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].app.title, "系统日志");
        assert_eq!(groups[0].app.id, "shared-logs");
        assert_eq!(groups[0].total_bytes, 150);
        let lines = format_grouped_plan_lines_with(&items, false);
        assert_eq!(lines[0], "系统日志  (2 types · 150 B)");
        assert!(lines.iter().any(|l| l.contains("/var/log")));
        assert!(lines.iter().any(|l| l.contains("/Library/Logs")));
        assert!(lines.iter().all(|l| !l.contains("DiagnosticReports")));
        assert!(lines.iter().all(|l| !l.contains("asl")));
    }

    #[test]
    fn system_library_caches_join_shared_cache_category() {
        let group = app_group_for(
            Path::new("/Library/Caches/foo.cache"),
            "System caches",
            "library-caches",
        );
        assert_eq!(group.title, "系统缓存");
        assert_eq!(group.id, "shared-cache");
    }

    #[test]
    fn library_cache_folder_name_becomes_app_group() {
        let group = app_group_for(
            Path::new("/Users/me/Library/Caches/Homebrew"),
            "User app cache",
            "user-app-cache",
        );
        assert_eq!(group.title, "Homebrew");
        assert_eq!(group.id, "homebrew");
    }

    #[test]
    fn saved_application_state_uses_bundle_not_savedstate_suffix() {
        let group = app_group_for(
            Path::new("/Users/me/Library/Saved Application State/com.youdao.YoudaoDict.savedState"),
            "Saved application states",
            "saved-application-states",
        );
        assert_eq!(group.title, "Youdao Dict");
        assert_eq!(group.id, "com.youdao.youdaodict");
    }

    #[test]
    fn shorten_display_path_keeps_var_folders_absolute() {
        let raw = "/private/var/folders/zc/71mnngz96gb6qpm2yznd3n4c0000gn/C/com.vpn07.app/com.apple.gpuarchiver";
        assert_eq!(
            shorten_display_path(Path::new(raw)),
            "/var/folders/zc/71mnngz96gb6qpm2yznd3n4c0000gn/C/com.vpn07.app/com.apple.gpuarchiver"
        );
    }

    #[test]
    fn shorten_display_path_keeps_home_absolute() {
        assert_eq!(
            shorten_display_path(Path::new("/Users/wukong/Library/Caches/com.apple.Safari")),
            "/Users/wukong/Library/Caches/com.apple.Safari"
        );
    }

    #[test]
    fn shorten_display_path_strips_private_tmp() {
        assert_eq!(
            shorten_display_path(Path::new("/private/tmp/orphan-file")),
            "/tmp/orphan-file"
        );
    }

    #[test]
    fn osc8_file_hyperlink_wraps_short_label_with_real_file_url() {
        let path = Path::new(
            "/private/var/folders/zc/71mnngz96gb6qpm2yznd3n4c0000gn/C/com.vpn07.app/com.apple.gpuarchiver",
        );
        let linked = osc8_file_hyperlink(
            path,
            "/var/folders/zc/71mnngz96gb6qpm2yznd3n4c0000gn/C/com.vpn07.app/com.apple.gpuarchiver",
        );
        assert!(
            linked.contains("\x1b]8;;file:///private/var/folders/zc/71mnngz96gb6qpm2yznd3n4c0000gn/C/com.vpn07.app/com.apple.gpuarchiver\x1b\\"),
            "missing OSC 8 open: {linked:?}"
        );
        assert!(
            linked.contains("/var/folders/zc/71mnngz96gb6qpm2yznd3n4c0000gn/C/com.vpn07.app/com.apple.gpuarchiver"),
            "visible label missing: {linked:?}"
        );
        assert!(
            linked.ends_with("\x1b]8;;\x1b\\"),
            "missing OSC 8 close: {linked:?}"
        );
    }

    #[test]
    fn file_url_from_path_percent_encodes_spaces() {
        assert_eq!(
            file_url_from_path(Path::new("/Users/me/Saved Application State")),
            "file:///Users/me/Saved%20Application%20State"
        );
    }

    #[test]
    fn metal_sibling_leaves_collapse_to_parent_type_dir() {
        let items = [
            entry(
                "/private/var/folders/zc/hash/C/com.example.FooBar/com.apple.metal",
                "Rebuildable GPU Metal caches",
                "gpu-metal-caches",
                100,
            ),
            entry(
                "/private/var/folders/zc/hash/C/com.example.FooBar/com.apple.metalfe",
                "Rebuildable GPU Metal caches",
                "gpu-metal-caches",
                50,
            ),
            entry(
                "/private/var/folders/zc/hash/C/com.example.FooBar/com.apple.gpuarchiver",
                "Rebuildable GPU Metal caches",
                "gpu-metal-caches",
                25,
            ),
        ];
        let lines = format_grouped_plan_lines_with(&items, false);
        assert_eq!(lines[0], "系统缓存  (1 type · 175 B)");
        assert_eq!(
            lines[1],
            "  Rebuildable GPU Metal caches  /var/folders/zc/hash/C  175 B"
        );
        assert!(lines.iter().all(|l| !l.contains("com.example.FooBar")));
        assert!(lines.iter().all(|l| !l.contains("com.apple.metal")));
    }

    #[test]
    fn format_grouped_plan_lines_can_hyperlink_short_path() {
        let items = vec![entry(
            "/private/var/folders/zc/hash/C/com.vpn07.app/com.apple.gpuarchiver",
            "Rebuildable GPU Metal caches",
            "gpu-metal-caches",
            100,
        )];
        let lines = format_grouped_plan_lines_with(&items, true);
        assert!(
            lines.iter().any(
                |l| l.contains("\x1b]8;;file:///private/var/folders/zc/hash/C\x1b\\")
                    && l.contains("/var/folders/zc/hash/C")
                    && !l.contains("com.vpn07.app")
            ),
            "expected clickable absolute type directory, got {lines:?}"
        );
        assert!(lines.iter().all(|l| !l.contains("com.apple.gpuarchiver")));
    }

    #[test]
    fn format_grouped_plan_lines_prints_app_header_then_paths() {
        let items = vec![
            entry(
                "/Users/me/Library/Caches/com.example.FooBar",
                "User app cache",
                "user-app-cache",
                2048,
            ),
            entry("/private/tmp/orphan-file", "Stale temp", "tmp", 100),
        ];
        let lines = format_grouped_plan_lines_with(&items, false);
        assert_eq!(lines[0], "Foo Bar  (1 type · 2.0 KB)");
        assert_eq!(
            lines[1],
            "  User app cache  /Users/me/Library/Caches/com.example.FooBar  2.0 KB"
        );
        assert!(lines.contains(&String::new()));
        assert!(lines.iter().any(|l| l.starts_with("临时文件  (1 type")));
        assert!(lines.iter().any(|l| l.contains("Stale temp  /tmp  100 B")));
        assert!(lines.iter().all(|l| !l.contains("orphan-file")));
        assert!(lines.iter().all(|l| !l.contains("user-app-cache")));
    }

    #[test]
    fn user_library_cache_keeps_app_subdirectory() {
        let items = vec![entry(
            "/Users/me/Library/Caches/Google",
            "User app cache",
            "user-app-cache",
            2048,
        )];
        let lines = format_grouped_plan_lines_with(&items, false);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("User app cache  /Users/me/Library/Caches/Google  2.0 KB")),
            "user-domain type dir must keep the app folder, got {lines:?}"
        );
        assert!(lines
            .iter()
            .all(|l| !l.contains("/Users/me/Library/Caches  2.0 KB")));
    }

    #[test]
    fn private_var_log_files_stop_at_log_type_dir() {
        let items = [
            entry(
                "/private/var/log/asl/BB.2027.06.30.G80.asl",
                "System private var logs",
                "private-var-log",
                8700,
            ),
            entry(
                "/private/var/log/asl/BB.2027.01.31.G80.asl",
                "System private var logs",
                "private-var-log",
                7500,
            ),
            entry(
                "/private/var/log/system.log",
                "System private var logs",
                "private-var-log",
                100,
            ),
        ];
        let lines = format_grouped_plan_lines_with(&items, false);
        assert!(
            lines[0].starts_with("系统日志  (1 type"),
            "shared log bucket must be 系统日志, got {lines:?}"
        );
        let type_rows: Vec<_> = lines
            .iter()
            .filter(|l| l.contains("System private var logs"))
            .collect();
        assert_eq!(
            type_rows.len(),
            1,
            "expected one merged type row, got {lines:?}"
        );
        assert!(
            type_rows[0].contains("  System private var logs  /var/log  "),
            "type dir must be /var/log, got {lines:?}"
        );
        assert!(
            lines
                .iter()
                .all(|l| !l.contains("asl") && !l.contains("BB.2027") && !l.contains("system.log")),
            "must not expand past log, got {lines:?}"
        );
    }

    #[test]
    fn private_var_log_hyperlink_targets_log_dir() {
        let items = vec![entry(
            "/private/var/log/asl/BB.2027.06.30.G80.asl",
            "System private var logs",
            "private-var-log",
            8700,
        )];
        let lines = format_grouped_plan_lines_with(&items, true);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("\x1b]8;;file:///private/var/log\x1b\\")
                    && l.contains("/var/log")
                    && !l.contains("asl")),
            "expected clickable /private/var/log, got {lines:?}"
        );
    }

    #[test]
    fn system_library_logs_stop_at_logs_type_dir() {
        let items = [entry(
            "/Library/Logs/DiagnosticReports/App.crash",
            "Diagnostic reports",
            "diagnostic-reports-system",
            4096,
        )];
        let lines = format_grouped_plan_lines_with(&items, false);
        assert!(
            lines.iter().any(|l| l.starts_with("系统日志  (1 type")),
            "system Logs group must be 系统日志, got {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("Diagnostic reports  /Library/Logs  ")),
            "system Logs is the type dir, got {lines:?}"
        );
        assert!(lines.iter().all(|l| !l.contains("DiagnosticReports")));
        assert!(lines.iter().all(|l| !l.contains("App.crash")));
    }
}
