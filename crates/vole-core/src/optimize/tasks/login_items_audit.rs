//! Optimize `login_items_audit`：只读审计损坏登录项（对齐 Mole `opt_login_items_audit`）。
//!
//! **不**删除登录项；删除语义仅属 `uninstall` 的 `LoginItemDeps`。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use super::delete_paths::OptimizeCandidate;
use crate::delete::test_no_auth;
use crate::login_items::percent_encode_token;
use crate::optimize::OptimizeTaskKind;

const LIVE_TIMEOUT: Duration = Duration::from_secs(20);
pub const UNAVAILABLE_LABEL: &str =
    "Login items audit unavailable (Automation / System Events) · grant access or retry";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginItemsAuditError {
    /// `VOLE_TEST_NO_AUTH` / test mode：不得触碰真 osascript / sudo。
    TestMode,
    /// AppleScript / System Events 不可用（TCC 等）。
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginItemSnapshot {
    pub name: String,
    pub path: String,
}

pub trait LoginItemsAuditDeps: Send + Sync {
    fn snapshot(&self) -> Result<Vec<LoginItemSnapshot>, LoginItemsAuditError>;
    fn app_exists(&self, name: &str, item_path: &str) -> bool;
}

pub struct LiveLoginItemsAuditDeps;

impl LoginItemsAuditDeps for LiveLoginItemsAuditDeps {
    fn snapshot(&self) -> Result<Vec<LoginItemSnapshot>, LoginItemsAuditError> {
        if test_no_auth() {
            return Err(LoginItemsAuditError::TestMode);
        }
        let script = r#"set oldDelimiters to AppleScript's text item delimiters
set tabChar to ASCII character 9
set linefeedChar to ASCII character 10
set outputLines to {}

tell application "System Events"
    repeat with loginItem in login items
        set itemName to ""
        set itemPath to ""

        try
            set itemName to name of loginItem as text
        end try

        try
            set itemPath to POSIX path of (path of loginItem as alias)
        on error
            try
                set itemPath to path of loginItem as text
            end try
        end try

        set end of outputLines to itemName & tabChar & itemPath
    end repeat
end tell

set AppleScript's text item delimiters to linefeedChar
set outputText to outputLines as text
set AppleScript's text item delimiters to oldDelimiters
return outputText"#;
        let mut cmd = Command::new("osascript");
        cmd.arg("-e").arg(script);
        let output = run_command_timeout(cmd, LIVE_TIMEOUT)
            .map_err(|_| LoginItemsAuditError::Unavailable)?;
        if !output.status.success() {
            return Err(LoginItemsAuditError::Unavailable);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(parse_snapshot_text(&text))
    }

    fn app_exists(&self, name: &str, item_path: &str) -> bool {
        live_app_exists(name, item_path)
    }
}

fn parse_snapshot_text(text: &str) -> Vec<LoginItemSnapshot> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, '\t');
        let name = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        out.push(LoginItemSnapshot { name, path });
    }
    out
}

fn live_app_exists(name: &str, item_path: &str) -> bool {
    if !item_path.is_empty() {
        let p = Path::new(item_path);
        if p.exists() || p.is_symlink() {
            return true;
        }
    }

    if name.contains('\'') {
        // Mole skips mdfind when name has single quotes.
    } else {
        let nospace: String = name.chars().filter(|c| *c != ' ').collect();
        let stripped = strip_helper_suffix(&nospace);
        for candidate in [
            format!("{name}.app"),
            format!("{nospace}.app"),
            format!("{stripped}.app"),
        ] {
            if candidate == ".app" {
                continue;
            }
            if mdfind_has_fs_name(&candidate) {
                return true;
            }
        }

        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(h) = home {
            roots.push(h.join("Applications"));
        }
        let app_names = {
            let mut v = vec![format!("{name}.app")];
            if nospace != name {
                v.push(format!("{nospace}.app"));
            }
            if stripped != nospace {
                v.push(format!("{stripped}.app"));
            }
            v
        };
        for root in &roots {
            if !root.is_dir() {
                continue;
            }
            if find_app_named(root, &app_names).is_some() {
                return true;
            }
            if find_app_by_bundle_metadata(root, name, &nospace, &stripped) {
                return true;
            }
        }
    }

    // Fallback: privileged sfltool dumpbtm only. Never call unprivileged dumpbtm
    // (pops macOS admin GUI). Skip entirely under test_no_auth.
    if test_no_auth() {
        return false;
    }
    if !sudo_n_true() {
        return false;
    }
    if let Some(btm) = sfltool_dumpbtm_path_for(name) {
        let p = Path::new(&btm);
        if p.exists() {
            return true;
        }
    }
    false
}

fn strip_helper_suffix(nospace: &str) -> String {
    for suffix in ["Client", "Helper", "Agent", "Launcher", "Service"] {
        if let Some(rest) = nospace.strip_suffix(suffix) {
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    nospace.to_string()
}

fn mdfind_has_fs_name(app_name: &str) -> bool {
    let query = format!("kMDItemFSName == '{app_name}'");
    let output = Command::new("mdfind")
        .arg(&query)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(o) if o.status.success() => !String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        _ => false,
    }
}

fn find_app_named(root: &Path, app_names: &[String]) -> Option<PathBuf> {
    let mut cmd = Command::new("find");
    cmd.arg(root)
        .arg("-maxdepth")
        .arg("6")
        .arg("-type")
        .arg("d");
    cmd.arg("(");
    for (i, name) in app_names.iter().enumerate() {
        if i > 0 {
            cmd.arg("-o");
        }
        cmd.arg("-name").arg(name);
    }
    cmd.arg(")");
    cmd.arg("-print").arg("-quit");
    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(PathBuf::from(line))
    }
}

fn find_app_by_bundle_metadata(root: &Path, name: &str, nospace: &str, stripped: &str) -> bool {
    let mut cmd = Command::new("find");
    cmd.arg(root)
        .arg("-maxdepth")
        .arg("6")
        .arg("-type")
        .arg("d")
        .arg("-name")
        .arg("*.app")
        .arg("-print0");
    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::null()).output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    for path in output.stdout.split(|&b| b == 0) {
        if path.is_empty() {
            continue;
        }
        let Ok(app_path) = std::str::from_utf8(path) else {
            continue;
        };
        if bundle_metadata_matches(Path::new(app_path), name, nospace, stripped) {
            return true;
        }
    }
    false
}

fn bundle_metadata_matches(app_path: &Path, name: &str, nospace: &str, stripped: &str) -> bool {
    let info = app_path.join("Contents/Info.plist");
    if !info.is_file() {
        return false;
    }
    for key in ["CFBundleDisplayName", "CFBundleName", "CFBundleExecutable"] {
        if let Some(value) = plutil_raw(&info, key) {
            if name_matches(&value, name, nospace, stripped) {
                return true;
            }
        }
    }
    false
}

fn name_matches(
    actual: &str,
    expected: &str,
    expected_nospace: &str,
    expected_stripped: &str,
) -> bool {
    if actual.is_empty() {
        return false;
    }
    let actual_nospace: String = actual.chars().filter(|c| *c != ' ').collect();
    if actual == expected {
        return true;
    }
    if actual_nospace == expected_nospace {
        return true;
    }
    if !expected_stripped.is_empty() && actual_nospace == expected_stripped {
        return true;
    }
    false
}

fn plutil_raw(plist: &Path, key: &str) -> Option<String> {
    let output = Command::new("plutil")
        .arg("-extract")
        .arg(key)
        .arg("raw")
        .arg(plist)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn sudo_n_true() -> bool {
    Command::new("sudo")
        .args(["-n", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn sfltool_dumpbtm_path_for(name: &str) -> Option<String> {
    let output = Command::new("sudo")
        .args(["-n", "sfltool", "dumpbtm"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let name_lower = name.to_ascii_lowercase();
    for line in text.lines() {
        if !line.to_ascii_lowercase().contains(&name_lower) {
            continue;
        }
        if let Some(path) = extract_app_path(line) {
            return Some(path);
        }
    }
    None
}

fn extract_app_path(line: &str) -> Option<String> {
    // Prefer file:// URL payload so `file:///Users/...` becomes `/Users/...`.
    let search = if let Some(rest) = line.split_once("file://").map(|(_, r)| r) {
        rest
    } else {
        line
    };
    let bytes = search.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' {
            if let Some(end) = search[i..].find(".app") {
                let end = i + end + 4;
                let mut path = search[i..end].to_string();
                while path.starts_with("//") {
                    path.remove(0);
                }
                return Some(path);
            }
        }
        i += 1;
    }
    None
}

fn run_command_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().map_err(|e| e.to_string());
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("command timed out after {}s", timeout.as_secs()));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

pub fn unavailable_sentinel(home: &Path) -> OptimizeCandidate {
    OptimizeCandidate {
        path: home.join(".vole-optimize-action/login_items_audit"),
        label: UNAVAILABLE_LABEL.into(),
        size: 0,
        task_id: "login_items_audit",
        kind: OptimizeTaskKind::Action,
    }
}

pub fn broken_candidate(home: &Path, name: &str) -> OptimizeCandidate {
    let encoded = percent_encode_token(name);
    OptimizeCandidate {
        path: home.join(format!(".vole-optimize-action/login_items_audit/{encoded}")),
        label: format!(
            "Broken login item: {name} (app not found) · remove via System Settings > General > Login Items"
        ),
        size: 0,
        task_id: "login_items_audit",
        kind: OptimizeTaskKind::Action,
    }
}

pub fn plan_login_items_audit(
    home: &Path,
    deps: &dyn LoginItemsAuditDeps,
) -> Vec<OptimizeCandidate> {
    match deps.snapshot() {
        Err(LoginItemsAuditError::TestMode) => Vec::new(),
        Err(LoginItemsAuditError::Unavailable) => vec![unavailable_sentinel(home)],
        Ok(items) => {
            let mut out = Vec::new();
            for item in items {
                if deps.app_exists(&item.name, &item.path) {
                    continue;
                }
                out.push(broken_candidate(home, &item.name));
            }
            out
        }
    }
}

pub fn is_unavailable_audit_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n == "login_items_audit")
}

/// Hermetic fake for plan/apply tests.
pub struct FakeLoginItemsAuditDeps {
    pub snapshot: Mutex<Result<Vec<LoginItemSnapshot>, LoginItemsAuditError>>,
    pub exists: Mutex<std::collections::HashMap<String, bool>>,
    pub exists_calls: Mutex<Vec<(String, String)>>,
}

impl Default for FakeLoginItemsAuditDeps {
    fn default() -> Self {
        Self {
            snapshot: Mutex::new(Ok(Vec::new())),
            exists: Mutex::new(std::collections::HashMap::new()),
            exists_calls: Mutex::new(Vec::new()),
        }
    }
}

impl FakeLoginItemsAuditDeps {
    pub fn with_items(items: Vec<LoginItemSnapshot>, exists: Vec<(&str, bool)>) -> Self {
        let mut map = std::collections::HashMap::new();
        for (name, ok) in exists {
            map.insert(name.to_string(), ok);
        }
        Self {
            snapshot: Mutex::new(Ok(items)),
            exists: Mutex::new(map),
            exists_calls: Mutex::new(Vec::new()),
        }
    }
}

impl LoginItemsAuditDeps for FakeLoginItemsAuditDeps {
    fn snapshot(&self) -> Result<Vec<LoginItemSnapshot>, LoginItemsAuditError> {
        self.snapshot.lock().unwrap().clone()
    }

    fn app_exists(&self, name: &str, item_path: &str) -> bool {
        self.exists_calls
            .lock()
            .unwrap()
            .push((name.to_string(), item_path.to_string()));
        *self.exists.lock().unwrap().get(name).unwrap_or(&false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn plan_emits_broken_only() {
        let home = tempfile::tempdir().unwrap();
        let fake = FakeLoginItemsAuditDeps::with_items(
            vec![
                LoginItemSnapshot {
                    name: "GoodApp".into(),
                    path: "/Applications/GoodApp.app".into(),
                },
                LoginItemSnapshot {
                    name: "Missing Helper".into(),
                    path: "/Applications/Missing Helper.app".into(),
                },
            ],
            vec![("GoodApp", true), ("Missing Helper", false)],
        );
        let plan = plan_login_items_audit(home.path(), &fake);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].label.contains("Broken login item: Missing Helper"));
        assert!(plan[0].label.contains("System Settings"));
        assert!(plan[0]
            .path
            .ends_with(".vole-optimize-action/login_items_audit/Missing%20Helper"));
    }

    #[test]
    fn plan_healthy_emits_nothing() {
        let home = tempfile::tempdir().unwrap();
        let fake = FakeLoginItemsAuditDeps::with_items(
            vec![LoginItemSnapshot {
                name: "GoodApp".into(),
                path: "/Applications/GoodApp.app".into(),
            }],
            vec![("GoodApp", true)],
        );
        assert!(plan_login_items_audit(home.path(), &fake).is_empty());
    }

    #[test]
    fn plan_test_mode_emits_nothing() {
        let home = tempfile::tempdir().unwrap();
        let fake = FakeLoginItemsAuditDeps {
            snapshot: Mutex::new(Err(LoginItemsAuditError::TestMode)),
            ..Default::default()
        };
        assert!(plan_login_items_audit(home.path(), &fake).is_empty());
    }

    #[test]
    fn plan_unavailable_emits_sentinel() {
        let home = tempfile::tempdir().unwrap();
        let fake = FakeLoginItemsAuditDeps {
            snapshot: Mutex::new(Err(LoginItemsAuditError::Unavailable)),
            ..Default::default()
        };
        let plan = plan_login_items_audit(home.path(), &fake);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].label, UNAVAILABLE_LABEL);
        assert!(is_unavailable_audit_path(&plan[0].path));
    }

    #[test]
    fn parse_snapshot_lines() {
        let items = parse_snapshot_text("Foo\t/Applications/Foo.app\nBar\t\n\n");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Foo");
        assert_eq!(items[0].path, "/Applications/Foo.app");
        assert_eq!(items[1].name, "Bar");
        assert!(items[1].path.is_empty());
    }

    #[test]
    fn extract_app_path_from_btm_line() {
        let line = "item=Foo url=file:///Users/x/Applications/Foo.app flags=1";
        assert_eq!(
            extract_app_path(line).as_deref(),
            Some("/Users/x/Applications/Foo.app")
        );
    }
}
