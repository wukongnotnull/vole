//! Uninstall Login Items / LoginItems helpers（对齐 Mole `remove_login_item` + bootout）。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::protection::{is_reverse_dns_bundle_id, read_bundle_id};

pub const LOGIN_ITEM_NAME_PREFIX: &str = "uninstall:login-item:name:";
pub const LOGIN_HELPER_PREFIX: &str = "uninstall:login-helper:";

const LIVE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginItemError {
    NeedsPrivilege,
    Failed(String),
}

pub trait LoginItemDeps: Send + Sync {
    fn remove_login_item(&self, display_name: &str) -> Result<(), LoginItemError>;
    fn bootout_helper(&self, uid: u32, helper_bundle_id: &str) -> Result<(), LoginItemError>;
}

pub struct LiveLoginItemDeps;

impl LoginItemDeps for LiveLoginItemDeps {
    fn remove_login_item(&self, display_name: &str) -> Result<(), LoginItemError> {
        let clean = display_name.strip_suffix(".app").unwrap_or(display_name);
        if clean.is_empty() {
            return Ok(());
        }
        if !is_safe_login_item_display_name(clean) {
            return Err(LoginItemError::Failed(
                "login item display name contains unsafe AppleScript metacharacters".into(),
            ));
        }
        let escaped = escape_applescript_string(clean);
        let script = format!(
            r#"tell application "System Events"
    try
        set itemCount to count of login items
        repeat with i from itemCount to 1 by -1
            try
                set itemName to name of login item i
                if itemName is "{escaped}" then
                    delete login item i
                end if
            end try
        end repeat
    end try
end tell"#
        );
        let mut cmd = Command::new("osascript");
        cmd.arg("-e").arg(&script);
        let output = run_command_timeout(cmd, LIVE_TIMEOUT).map_err(map_osascript_error)?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(map_osascript_error(format!("{stderr}{stdout}")))
    }

    fn bootout_helper(&self, uid: u32, helper_bundle_id: &str) -> Result<(), LoginItemError> {
        if !is_bootout_allowed(helper_bundle_id) {
            return Ok(());
        }
        let label = format!("gui/{uid}/{helper_bundle_id}");
        // Mole: best-effort；非 0 也吞掉
        let mut cmd = Command::new("launchctl");
        cmd.arg("bootout").arg(&label);
        let _ = run_command_timeout(cmd, LIVE_TIMEOUT);
        Ok(())
    }
}

pub fn is_bootout_allowed(helper_bundle_id: &str) -> bool {
    is_reverse_dns_bundle_id(helper_bundle_id)
        && !helper_bundle_id
            .to_ascii_lowercase()
            .starts_with("com.apple.")
}

pub fn percent_encode_token(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn percent_decode_token(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let h = from_hex(bytes[i + 1])?;
            let l = from_hex(bytes[i + 2])?;
            out.push((h << 4) | l);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// 拒绝会破坏 AppleScript 字面量结构的显示名（`&`、换行等）。
pub fn is_safe_login_item_display_name(s: &str) -> bool {
    !s.is_empty()
        && !s
            .chars()
            .any(|c| matches!(c, '&' | '\n' | '\r' | '\0') || c.is_control())
}

pub fn encode_login_item_name_rule_id(display_name: &str) -> Option<String> {
    let clean = display_name.strip_suffix(".app").unwrap_or(display_name);
    if !is_safe_login_item_display_name(clean) {
        return None;
    }
    Some(format!(
        "{LOGIN_ITEM_NAME_PREFIX}{}",
        percent_encode_token(clean)
    ))
}

pub fn parse_login_item_name_rule_id(rule_id: &str) -> Option<String> {
    let rest = rule_id.strip_prefix(LOGIN_ITEM_NAME_PREFIX)?;
    if rest.is_empty() {
        return None;
    }
    percent_decode_token(rest)
}

pub fn encode_login_helper_rule_id(bundle_id: &str) -> String {
    format!("{LOGIN_HELPER_PREFIX}{bundle_id}")
}

pub fn parse_login_helper_rule_id(rule_id: &str) -> Option<String> {
    let rest = rule_id.strip_prefix(LOGIN_HELPER_PREFIX)?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    Some(rest.to_string())
}

pub fn login_name_collides(display_name: &str, sibling_display_names: &[String]) -> bool {
    let needle = display_name
        .strip_suffix(".app")
        .unwrap_or(display_name)
        .to_ascii_lowercase();
    sibling_display_names.iter().any(|s| {
        s.strip_suffix(".app")
            .unwrap_or(s)
            .eq_ignore_ascii_case(&needle)
    })
}

/// 扫描 `Contents/Library/LoginItems/*.app`，返回 (helper_app_path, bundle_id)。
pub fn discover_login_item_helper_bundle_ids(app_path: &Path) -> Vec<(PathBuf, String)> {
    let root = app_path.join("Contents/Library/LoginItems");
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("app") {
            continue;
        }
        let Some(bundle_id) = read_bundle_id(&path) else {
            continue;
        };
        if !is_bootout_allowed(&bundle_id) {
            continue;
        }
        out.push((path, bundle_id));
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// AppleScript 字符串字面量：双引号通过加倍嵌入（`"` → `""`），不是 shell `\"`。
pub fn escape_applescript_string(s: &str) -> String {
    s.replace('"', "\"\"")
}

fn map_osascript_error(err: String) -> LoginItemError {
    let lower = err.to_ascii_lowercase();
    if lower.contains("not authorized")
        || lower.contains("not allowed")
        || lower.contains("(-1743)")
        || lower.contains("osstatus -1743")
        || lower.contains("tcc")
        || lower.contains("permission")
    {
        LoginItemError::NeedsPrivilege
    } else {
        LoginItemError::Failed(err)
    }
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

/// 测试用 deps：记录调用；可配置错误。
#[derive(Default)]
pub struct FakeLoginItemDeps {
    pub removed_names: Mutex<Vec<String>>,
    pub booted_helpers: Mutex<Vec<(u32, String)>>,
    pub remove_error: Mutex<Option<LoginItemError>>,
    pub bootout_error: Mutex<Option<LoginItemError>>,
}

impl LoginItemDeps for FakeLoginItemDeps {
    fn remove_login_item(&self, display_name: &str) -> Result<(), LoginItemError> {
        if let Some(err) = self.remove_error.lock().unwrap().clone() {
            return Err(err);
        }
        self.removed_names
            .lock()
            .unwrap()
            .push(display_name.to_string());
        Ok(())
    }

    fn bootout_helper(&self, uid: u32, helper_bundle_id: &str) -> Result<(), LoginItemError> {
        if let Some(err) = self.bootout_error.lock().unwrap().clone() {
            return Err(err);
        }
        self.booted_helpers
            .lock()
            .unwrap()
            .push((uid, helper_bundle_id.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn name_rule_id_roundtrip_with_spaces() {
        let id = encode_login_item_name_rule_id("Foo Bar").unwrap();
        assert!(id.starts_with("uninstall:login-item:name:"));
        assert_eq!(
            parse_login_item_name_rule_id(&id).as_deref(),
            Some("Foo Bar")
        );
        assert!(!id.contains(' '));
    }

    #[test]
    fn encode_rejects_applescript_metacharacters() {
        assert!(encode_login_item_name_rule_id("Foo&do shell script \"id\"").is_none());
        assert!(encode_login_item_name_rule_id("Foo\nBar").is_none());
        assert!(!is_safe_login_item_display_name("a&b"));
    }

    #[test]
    fn applescript_escape_doubles_quotes() {
        assert_eq!(escape_applescript_string(r#"Foo"Bar"#), r#"Foo""Bar"#);
        assert_eq!(escape_applescript_string("plain"), "plain");
    }

    #[test]
    fn helper_rule_id_roundtrip() {
        let id = encode_login_helper_rule_id("com.example.helper");
        assert_eq!(id, "uninstall:login-helper:com.example.helper");
        assert_eq!(
            parse_login_helper_rule_id(&id).as_deref(),
            Some("com.example.helper")
        );
        assert!(parse_login_helper_rule_id("uninstall:com.example").is_none());
    }

    #[test]
    fn discover_helpers_and_skips_apple() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Carrier.app");
        write_helper(
            &app.join("Contents/Library/LoginItems/Good Helper.app"),
            "com.example.good",
        );
        write_helper(
            &app.join("Contents/Library/LoginItems/Evil.app"),
            "com.apple.Evil",
        );
        let hits = discover_login_item_helper_bundle_ids(&app);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, "com.example.good");
    }

    #[test]
    fn login_name_collides_case_insensitive() {
        assert!(login_name_collides("Foo", &["foo".into()]));
        assert!(!login_name_collides("Foo", &["Foo Beta".into()]));
    }

    #[test]
    fn fake_remove_and_bootout_record_calls() {
        let fake = FakeLoginItemDeps::default();
        fake.remove_login_item("Foo").unwrap();
        fake.bootout_helper(501, "com.example.h").unwrap();
        assert_eq!(fake.removed_names.lock().unwrap().as_slice(), ["Foo"]);
        assert_eq!(
            fake.booted_helpers.lock().unwrap().as_slice(),
            &[(501u32, "com.example.h".into())]
        );
    }

    #[test]
    fn is_bootout_allowed_rejects_apple() {
        assert!(is_bootout_allowed("com.example.helper"));
        assert!(!is_bootout_allowed("com.apple.Safari.helper"));
        assert!(!is_bootout_allowed("not a bundle"));
    }

    fn write_helper(app: &Path, bundle_id: &str) {
        let contents = app.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{bundle_id}</string>
</dict></plist>"#
        );
        fs::write(contents.join("Info.plist"), plist).unwrap();
    }
}
