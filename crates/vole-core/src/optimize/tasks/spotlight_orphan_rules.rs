//! Optimize `spotlight_orphan_rules_cleanup`（对齐 Mole `opt_prune_spotlight_orphan_rules`）。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use super::delete_paths::OptimizeCandidate;
use crate::delete::test_no_auth;
use crate::optimize::OptimizeTaskKind;
use crate::protection::is_reverse_dns_bundle_id;

const DOMAIN: &str = "com.apple.spotlight";
const KEY: &str = "EnabledPreferenceRules";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotlightOrphanError {
    TestMode,
    Unavailable,
}

pub trait SpotlightOrphanDeps: Send + Sync {
    fn list_rules(&self) -> Result<Vec<String>, SpotlightOrphanError>;
    /// `true` = keep（已安装或不确定）；`false` = 确认可删 orphan。
    fn app_installed(&self, bundle_id: &str) -> bool;
    fn write_rules(&self, keep: &[String]) -> Result<(), SpotlightOrphanError>;
    fn delete_rules(&self) -> Result<(), SpotlightOrphanError>;
}

pub struct LiveSpotlightOrphanDeps;

impl SpotlightOrphanDeps for LiveSpotlightOrphanDeps {
    fn list_rules(&self) -> Result<Vec<String>, SpotlightOrphanError> {
        if test_no_auth() {
            return Err(SpotlightOrphanError::TestMode);
        }
        let output = Command::new("defaults")
            .args(["read", DOMAIN, KEY])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| SpotlightOrphanError::Unavailable)?;
        if !output.status.success() {
            // Key absent → already clean (Mole returns 0 with "already clean").
            return Ok(Vec::new());
        }
        Ok(parse_defaults_array_text(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    fn app_installed(&self, bundle_id: &str) -> bool {
        if test_no_auth() {
            // Fail-closed: never claim "gone" under test_no_auth.
            return true;
        }
        live_app_installed(bundle_id)
    }

    fn write_rules(&self, keep: &[String]) -> Result<(), SpotlightOrphanError> {
        if test_no_auth() {
            return Err(SpotlightOrphanError::TestMode);
        }
        let mut cmd = Command::new("defaults");
        cmd.args(["write", DOMAIN, KEY, "-array"]);
        for entry in keep {
            cmd.arg(entry);
        }
        let status = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| SpotlightOrphanError::Unavailable)?;
        if status.success() {
            Ok(())
        } else {
            Err(SpotlightOrphanError::Unavailable)
        }
    }

    fn delete_rules(&self) -> Result<(), SpotlightOrphanError> {
        if test_no_auth() {
            return Err(SpotlightOrphanError::TestMode);
        }
        let status = Command::new("defaults")
            .args(["delete", DOMAIN, KEY])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| SpotlightOrphanError::Unavailable)?;
        if status.success() {
            Ok(())
        } else {
            Err(SpotlightOrphanError::Unavailable)
        }
    }
}

/// Parse `defaults read` array output into string entries.
pub fn parse_defaults_array_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim().trim_end_matches(',');
        if t.is_empty() || t == "(" || t == ")" {
            continue;
        }
        let entry = t.trim_matches('"').trim().to_string();
        if !entry.is_empty() {
            out.push(entry);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedRules {
    pub keep: Vec<String>,
    pub remove: Vec<String>,
}

/// Fail-closed classify: only reverse-DNS + confirmed uninstalled → remove.
pub fn classify_spotlight_rules(
    rules: &[String],
    app_installed: &dyn Fn(&str) -> bool,
) -> ClassifiedRules {
    let mut keep = Vec::new();
    let mut remove = Vec::new();
    for entry in rules {
        if entry.starts_with("System.") || entry.starts_with("com.apple.") {
            keep.push(entry.clone());
            continue;
        }
        if is_reverse_dns_bundle_id(entry) && !app_installed(entry) {
            remove.push(entry.clone());
        } else {
            keep.push(entry.clone());
        }
    }
    ClassifiedRules { keep, remove }
}

fn live_app_installed(bundle_id: &str) -> bool {
    if !is_reverse_dns_bundle_id(bundle_id) {
        return true;
    }
    // Spotlight fast path; IO/timeout failure → keep (fail-closed).
    match mdfind_bundle(bundle_id) {
        Ok(true) => return true,
        Ok(false) => {}
        Err(()) => return true,
    }
    scan_app_roots_for_bundle(bundle_id)
}

fn mdfind_bundle(bundle_id: &str) -> Result<bool, ()> {
    if bundle_id.contains('\'') {
        return Ok(false);
    }
    let query = format!("kMDItemCFBundleIdentifier == '{bundle_id}'");
    let output = Command::new("mdfind")
        .arg(&query)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|l| !l.trim().is_empty()))
}

fn scan_app_roots_for_bundle(bundle_id: &str) -> bool {
    let bundle_lower = bundle_id.to_ascii_lowercase();
    let parent_lower = helper_parent_id(&bundle_lower);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(h) = home {
        roots.push(h.join("Applications"));
    }
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let app = entry.path();
            if app.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            let helper = app.join("Contents/Library/LaunchServices").join(bundle_id);
            if helper.exists() {
                return true;
            }
            let info = app.join("Contents/Info.plist");
            let Ok(id) = read_cf_bundle_id(&info) else {
                continue;
            };
            let id_lower = id.to_ascii_lowercase();
            if id_lower == bundle_lower {
                return true;
            }
            if let Some(ref parent) = parent_lower {
                if id_lower == *parent {
                    return true;
                }
            }
            if microsoft_mapped_parent(bundle_id, &id) {
                return true;
            }
        }
    }
    false
}

fn helper_parent_id(bundle_lower: &str) -> Option<String> {
    for suffix in [".helper", ".daemon", ".agent", ".xpc", ".service"] {
        if let Some(parent) = bundle_lower.strip_suffix(suffix) {
            return Some(parent.to_string());
        }
    }
    None
}

fn microsoft_mapped_parent(bundle_id: &str, app_bundle: &str) -> bool {
    matches!(
        bundle_id,
        "com.microsoft.autoupdate.helper" | "com.microsoft.office.licensingV2.helper"
    ) && matches!(
        app_bundle,
        "com.microsoft.Word"
            | "com.microsoft.Excel"
            | "com.microsoft.Powerpoint"
            | "com.microsoft.Outlook"
            | "com.microsoft.OneNote"
    )
}

fn read_cf_bundle_id(info: &Path) -> Result<String, ()> {
    let output = Command::new("plutil")
        .args(["-extract", "CFBundleIdentifier", "raw"])
        .arg(info)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() {
        Err(())
    } else {
        Ok(id)
    }
}

fn action_path(home: &Path) -> PathBuf {
    home.join(".vole-optimize-action/spotlight_orphan_rules_cleanup")
}

pub fn plan_spotlight_orphan_rules_cleanup(
    home: &Path,
    deps: &dyn SpotlightOrphanDeps,
) -> Vec<OptimizeCandidate> {
    let rules = match deps.list_rules() {
        Ok(r) => r,
        Err(SpotlightOrphanError::TestMode) | Err(SpotlightOrphanError::Unavailable) => {
            return Vec::new();
        }
    };
    if rules.is_empty() {
        return Vec::new();
    }
    let classified = classify_spotlight_rules(&rules, &|id| deps.app_installed(id));
    if classified.remove.is_empty() {
        return Vec::new();
    }
    let n = classified.remove.len();
    vec![OptimizeCandidate {
        path: action_path(home),
        label: format!("Would remove {n} orphan Spotlight rule(s)"),
        size: 0,
        task_id: "spotlight_orphan_rules_cleanup",
        kind: OptimizeTaskKind::Action,
    }]
}

/// Re-scan and rewrite keep array via deps（幂等）。
pub fn run_spotlight_orphan_rules_cleanup(
    deps: &dyn SpotlightOrphanDeps,
) -> Result<(), SpotlightOrphanError> {
    let rules = deps.list_rules()?;
    let classified = classify_spotlight_rules(&rules, &|id| deps.app_installed(id));
    if classified.remove.is_empty() {
        return Ok(());
    }
    if classified.keep.is_empty() {
        deps.delete_rules()
    } else {
        deps.write_rules(&classified.keep)
    }
}

#[derive(Default)]
pub struct FakeSpotlightOrphanDeps {
    pub rules: Mutex<Vec<String>>,
    pub installed: Mutex<Vec<String>>,
    pub writes: Mutex<Vec<Vec<String>>>,
    pub deletes: Mutex<usize>,
    pub list_error: Mutex<Option<SpotlightOrphanError>>,
}

impl SpotlightOrphanDeps for FakeSpotlightOrphanDeps {
    fn list_rules(&self) -> Result<Vec<String>, SpotlightOrphanError> {
        if let Some(err) = self.list_error.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(self.rules.lock().unwrap().clone())
    }

    fn app_installed(&self, bundle_id: &str) -> bool {
        self.installed
            .lock()
            .unwrap()
            .iter()
            .any(|id| id.eq_ignore_ascii_case(bundle_id))
    }

    fn write_rules(&self, keep: &[String]) -> Result<(), SpotlightOrphanError> {
        self.writes.lock().unwrap().push(keep.to_vec());
        *self.rules.lock().unwrap() = keep.to_vec();
        Ok(())
    }

    fn delete_rules(&self) -> Result<(), SpotlightOrphanError> {
        *self.deletes.lock().unwrap() += 1;
        self.rules.lock().unwrap().clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn classify_keeps_system_apple_and_installed() {
        let rules = vec![
            "System.iphoneApps".into(),
            "com.apple.Safari".into(),
            "com.installed.App".into(),
            "com.lm.william.TwinklingCard".into(),
            "not-a-bundle".into(),
        ];
        let c = classify_spotlight_rules(&rules, &|id| id == "com.installed.App");
        assert_eq!(
            c.keep,
            vec![
                "System.iphoneApps",
                "com.apple.Safari",
                "com.installed.App",
                "not-a-bundle",
            ]
        );
        assert_eq!(c.remove, vec!["com.lm.william.TwinklingCard"]);
    }

    #[test]
    fn plan_emits_one_candidate_when_orphans_exist() {
        let home = tempdir().unwrap();
        let fake = FakeSpotlightOrphanDeps {
            rules: Mutex::new(vec![
                "System.iphoneApps".into(),
                "com.installed.App".into(),
                "com.orphan.App".into(),
            ]),
            installed: Mutex::new(vec!["com.installed.App".into()]),
            ..Default::default()
        };
        let plan = plan_spotlight_orphan_rules_cleanup(home.path(), &fake);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].task_id, "spotlight_orphan_rules_cleanup");
        assert!(plan[0].label.contains("1 orphan"));
        assert!(plan[0]
            .path
            .ends_with(".vole-optimize-action/spotlight_orphan_rules_cleanup"));
    }

    #[test]
    fn plan_empty_when_clean_or_missing() {
        let home = tempdir().unwrap();
        let fake = FakeSpotlightOrphanDeps {
            rules: Mutex::new(vec!["System.iphoneApps".into(), "com.installed.App".into()]),
            installed: Mutex::new(vec!["com.installed.App".into()]),
            ..Default::default()
        };
        assert!(plan_spotlight_orphan_rules_cleanup(home.path(), &fake).is_empty());

        let empty = FakeSpotlightOrphanDeps::default();
        assert!(plan_spotlight_orphan_rules_cleanup(home.path(), &empty).is_empty());
    }

    #[test]
    fn apply_rewrites_keep_without_orphan() {
        let fake = FakeSpotlightOrphanDeps {
            rules: Mutex::new(vec![
                "System.iphoneApps".into(),
                "com.apple.Safari".into(),
                "com.installed.App".into(),
                "com.orphan.App".into(),
            ]),
            installed: Mutex::new(vec!["com.installed.App".into()]),
            ..Default::default()
        };
        run_spotlight_orphan_rules_cleanup(&fake).unwrap();
        let writes = fake.writes.lock().unwrap().clone();
        assert_eq!(writes.len(), 1);
        assert_eq!(
            writes[0],
            vec!["System.iphoneApps", "com.apple.Safari", "com.installed.App",]
        );
        assert_eq!(*fake.deletes.lock().unwrap(), 0);
    }

    #[test]
    fn apply_deletes_key_when_keep_empty() {
        let fake = FakeSpotlightOrphanDeps {
            rules: Mutex::new(vec!["com.orphan.Only".into()]),
            installed: Mutex::new(vec![]),
            ..Default::default()
        };
        run_spotlight_orphan_rules_cleanup(&fake).unwrap();
        assert_eq!(*fake.deletes.lock().unwrap(), 1);
        assert!(fake.writes.lock().unwrap().is_empty());
    }

    #[test]
    fn apply_test_mode_errors() {
        let fake = FakeSpotlightOrphanDeps {
            list_error: Mutex::new(Some(SpotlightOrphanError::TestMode)),
            ..Default::default()
        };
        let err = run_spotlight_orphan_rules_cleanup(&fake).unwrap_err();
        assert_eq!(err, SpotlightOrphanError::TestMode);
    }

    #[test]
    fn parse_defaults_array_text_strips_parens() {
        let text = "(\n    \"System.iphoneApps\",\n    \"com.example.App\"\n)\n";
        assert_eq!(
            parse_defaults_array_text(text),
            vec!["System.iphoneApps", "com.example.App"]
        );
    }
}
