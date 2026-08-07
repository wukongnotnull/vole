//! Homebrew Cask 检测与卸载联动（W2a①）。
//!
//! 对齐 Mole `lib/uninstall/brew.sh`：多阶段检测 + `brew uninstall --cask [--zap]`。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// plan/apply 用的 brew-cask rule_id 前缀。
pub const BREW_CASK_RULE_PREFIX: &str = "uninstall:brew-cask:";

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZapMode {
    Zap,
    NoZap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaskInstallState {
    Installed,
    NotInstalled,
    Unknown,
}

/// cask token：`^[a-z0-9][a-z0-9-]*$`
pub fn is_valid_cask_token(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 从 Caskroom 路径抽取 token：`…/Caskroom/<token>/…`
pub fn extract_cask_token_from_caskroom_path(path: &Path) -> Option<String> {
    let s = path.to_str()?;
    let rest = s
        .strip_prefix("/opt/homebrew/Caskroom/")
        .or_else(|| s.strip_prefix("/usr/local/Caskroom/"))?;
    let token = rest.split('/').next().unwrap_or("");
    if is_valid_cask_token(token) {
        Some(token.to_string())
    } else {
        None
    }
}

pub fn encode_brew_cask_rule_id(mode: ZapMode, token: &str) -> String {
    let mode_s = match mode {
        ZapMode::Zap => "zap",
        ZapMode::NoZap => "nozap",
    };
    format!("{BREW_CASK_RULE_PREFIX}{mode_s}:{token}")
}

pub fn parse_brew_cask_rule_id(rule_id: &str) -> Option<(ZapMode, String)> {
    let rest = rule_id.strip_prefix(BREW_CASK_RULE_PREFIX)?;
    let (mode_s, token) = rest.split_once(':')?;
    let mode = match mode_s {
        "zap" => ZapMode::Zap,
        "nozap" => ZapMode::NoZap,
        _ => return None,
    };
    if !is_valid_cask_token(token) {
        return None;
    }
    Some((mode, token.to_string()))
}

/// 超时启发：默认 300s；>5GiB→600；>15GiB→900。
pub fn brew_uninstall_timeout_secs(_app_path: Option<&Path>, size_bytes: u64) -> u64 {
    if size_bytes > 15 * GIB {
        900
    } else if size_bytes > 5 * GIB {
        600
    } else {
        300
    }
}

fn info_matches_app(info: &str, app_path: &Path, app_bundle_name: &str) -> bool {
    let path_s = app_path.display().to_string();
    let apps_path = format!("/Applications/{app_bundle_name}");
    info.contains(&path_s) || info.contains(&apps_path) || info.contains(app_bundle_name)
}

/// Homebrew 依赖（测试可注入）。
pub trait BrewDeps: Send + Sync {
    fn brew_available(&self) -> bool;
    fn list_casks(&self) -> Result<Vec<String>, ()>;
    fn cask_info(&self, token: &str) -> Result<String, ()>;
    fn is_cask_installed(&self, token: &str) -> CaskInstallState;
    fn uninstall_cask(
        &self,
        token: &str,
        mode: ZapMode,
        app_path: Option<&Path>,
    ) -> Result<(), String>;
    fn resolve_path(&self, path: &Path) -> Option<PathBuf>;
    fn read_symlink(&self, path: &Path) -> Option<PathBuf>;
    fn find_caskroom_apps(&self, app_bundle_name: &str) -> Vec<PathBuf>;
}

#[derive(Debug, Default)]
pub struct LiveBrewDeps;

impl BrewDeps for LiveBrewDeps {
    fn brew_available(&self) -> bool {
        Command::new("brew")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_casks(&self) -> Result<Vec<String>, ()> {
        let output = Command::new("brew")
            .args(["list", "--cask"])
            .env("HOMEBREW_NO_ENV_HINTS", "1")
            .output()
            .map_err(|_| ())?;
        if !output.status.success() {
            return Err(());
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect())
    }

    fn cask_info(&self, token: &str) -> Result<String, ()> {
        let output = Command::new("brew")
            .args(["info", "--cask", token])
            .env("HOMEBREW_NO_ENV_HINTS", "1")
            .output()
            .map_err(|_| ())?;
        if !output.status.success() {
            return Err(());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn is_cask_installed(&self, token: &str) -> CaskInstallState {
        match self.list_casks() {
            Ok(list) => {
                if list.iter().any(|c| c == token) {
                    CaskInstallState::Installed
                } else {
                    CaskInstallState::NotInstalled
                }
            }
            Err(()) => CaskInstallState::Unknown,
        }
    }

    fn uninstall_cask(
        &self,
        token: &str,
        mode: ZapMode,
        app_path: Option<&Path>,
    ) -> Result<(), String> {
        if !self.brew_available() {
            return Err("brew not available".into());
        }
        if !is_valid_cask_token(token) {
            return Err("invalid cask token".into());
        }
        let size = app_path
            .and_then(|p| dir_size_bytes_shallow(p))
            .unwrap_or(0);
        let timeout = Duration::from_secs(brew_uninstall_timeout_secs(app_path, size));

        let token_owned = token.to_string();
        let zap = matches!(mode, ZapMode::Zap);
        let token_for_cmd = token_owned.clone();
        let handle = thread::spawn(move || {
            let mut cmd = Command::new("brew");
            cmd.args(["uninstall", "--cask"]);
            if zap {
                cmd.arg("--zap");
            }
            cmd.arg(&token_for_cmd)
                .env("HOMEBREW_NO_ENV_HINTS", "1")
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .env("NONINTERACTIVE", "1")
                .output()
        });

        let start = Instant::now();
        while !handle.is_finished() {
            if start.elapsed() >= timeout {
                return Err(format!(
                    "brew uninstall timed out after {}s",
                    timeout.as_secs()
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }

        let output = handle
            .join()
            .map_err(|_| "brew uninstall thread panicked".to_string())?
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            return Ok(());
        }

        // 再验：已不在 list 且 app 消失 → 视为成功（对齐 Mole）
        let cask_gone = matches!(
            self.is_cask_installed(&token_owned),
            CaskInstallState::NotInstalled
        );
        let app_gone = app_path.map(|p| !p.exists()).unwrap_or(true);
        if cask_gone && app_gone {
            return Ok(());
        }
        Err(format!(
            "brew uninstall failed (exit {:?})",
            output.status.code()
        ))
    }

    fn resolve_path(&self, path: &Path) -> Option<PathBuf> {
        fs::canonicalize(path).ok()
    }

    fn read_symlink(&self, path: &Path) -> Option<PathBuf> {
        let meta = fs::symlink_metadata(path).ok()?;
        if !meta.file_type().is_symlink() {
            return None;
        }
        let target = fs::read_link(path).ok()?;
        if target.is_absolute() {
            Some(target)
        } else {
            Some(path.parent()?.join(target))
        }
    }

    fn find_caskroom_apps(&self, app_bundle_name: &str) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for room in ["/opt/homebrew/Caskroom", "/usr/local/Caskroom"] {
            let root = Path::new(room);
            if !root.is_dir() {
                continue;
            }
            // maxdepth 3: room/token/version/App.app
            let Ok(tokens) = fs::read_dir(root) else {
                continue;
            };
            for token_ent in tokens.flatten() {
                let token_dir = token_ent.path();
                if !token_dir.is_dir() {
                    continue;
                }
                let Ok(versions) = fs::read_dir(&token_dir) else {
                    continue;
                };
                for ver_ent in versions.flatten() {
                    let candidate = ver_ent.path().join(app_bundle_name);
                    if candidate.exists() {
                        out.push(candidate);
                    }
                }
            }
        }
        out
    }
}

fn dir_size_bytes_shallow(path: &Path) -> Option<u64> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.is_file() {
        return Some(meta.len());
    }
    let mut total = 0u64;
    for entry in jwalk::WalkDir::new(path)
        .skip_hidden(false)
        .into_iter()
        .flatten()
    {
        if let Ok(m) = entry.metadata() {
            if m.is_file() {
                total = total.saturating_add(m.len());
            }
        }
    }
    Some(total)
}

/// 多阶段检测（fast→slow），对齐 Mole `get_brew_cask_name`。
pub fn detect_cask_name(deps: &dyn BrewDeps, app_path: &Path) -> Option<String> {
    if !deps.brew_available() {
        return None;
    }
    if !app_path.exists() {
        return None;
    }
    let app_bundle_name = app_path.file_name()?.to_str()?.to_string();

    // Stage 1: resolved path in Caskroom
    if let Some(resolved) = deps.resolve_path(app_path) {
        if let Some(token) = extract_cask_token_from_caskroom_path(&resolved) {
            return Some(token);
        }
    }

    // Stage 2: unique Caskroom find by bundle name
    if let Some(token) = detect_via_caskroom_search(deps, &app_bundle_name, app_path) {
        return Some(token);
    }

    // Stage 3: direct symlink into Caskroom
    if let Some(target) = deps.read_symlink(app_path) {
        if let Some(token) = extract_cask_token_from_caskroom_path(&target) {
            return Some(token);
        }
    }

    // Stage 4: brew list name match + info verify
    detect_via_brew_list(deps, app_path, &app_bundle_name)
}

fn detect_via_caskroom_search(
    deps: &dyn BrewDeps,
    app_bundle_name: &str,
    app_path: &Path,
) -> Option<String> {
    let hits = deps.find_caskroom_apps(app_bundle_name);
    let mut tokens: Vec<String> = hits
        .iter()
        .filter_map(|p| extract_cask_token_from_caskroom_path(p))
        .collect();
    tokens.sort();
    tokens.dedup();
    if tokens.len() != 1 {
        return None;
    }
    let token = tokens.pop()?;
    let list = deps.list_casks().ok()?;
    if !list.iter().any(|c| c == &token) {
        return None;
    }
    let info = deps.cask_info(&token).ok()?;
    if !info_matches_app(&info, app_path, app_bundle_name) {
        return None;
    }
    Some(token)
}

fn detect_via_brew_list(
    deps: &dyn BrewDeps,
    app_path: &Path,
    app_bundle_name: &str,
) -> Option<String> {
    let stem = app_bundle_name
        .strip_suffix(".app")
        .unwrap_or(app_bundle_name)
        .to_ascii_lowercase();
    let list = deps.list_casks().ok()?;
    let matches: Vec<_> = list
        .into_iter()
        .filter(|c| c.eq_ignore_ascii_case(&stem))
        .collect();
    if matches.len() != 1 {
        return None;
    }
    let token = matches.into_iter().next()?;
    let info = deps.cask_info(&token).ok()?;
    if !info_matches_app(&info, app_path, app_bundle_name) {
        return None;
    }
    Some(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;

    #[derive(Default)]
    struct LastUninstall {
        #[allow(dead_code)]
        token: String,
        zap: bool,
    }

    struct FakeBrewDeps {
        available: bool,
        resolve: Option<PathBuf>,
        symlink: Option<PathBuf>,
        find_hits: Vec<PathBuf>,
        list: Result<Vec<String>, ()>,
        info: Result<String, ()>,
        install_state: CaskInstallState,
        uninstall_ok: bool,
        last_uninstall: Mutex<Option<LastUninstall>>,
    }

    impl FakeBrewDeps {
        fn empty() -> Self {
            Self {
                available: false,
                resolve: None,
                symlink: None,
                find_hits: Vec::new(),
                list: Ok(vec![]),
                info: Ok(String::new()),
                install_state: CaskInstallState::Unknown,
                uninstall_ok: true,
                last_uninstall: Mutex::new(None),
            }
        }
    }

    impl BrewDeps for FakeBrewDeps {
        fn brew_available(&self) -> bool {
            self.available
        }
        fn list_casks(&self) -> Result<Vec<String>, ()> {
            self.list.clone()
        }
        fn cask_info(&self, _token: &str) -> Result<String, ()> {
            self.info.clone()
        }
        fn is_cask_installed(&self, _token: &str) -> CaskInstallState {
            self.install_state
        }
        fn uninstall_cask(
            &self,
            token: &str,
            mode: ZapMode,
            _app_path: Option<&Path>,
        ) -> Result<(), String> {
            *self.last_uninstall.lock().unwrap() = Some(LastUninstall {
                token: token.to_string(),
                zap: matches!(mode, ZapMode::Zap),
            });
            if self.uninstall_ok {
                Ok(())
            } else {
                Err("fake fail".into())
            }
        }
        fn resolve_path(&self, _path: &Path) -> Option<PathBuf> {
            self.resolve.clone()
        }
        fn read_symlink(&self, _path: &Path) -> Option<PathBuf> {
            self.symlink.clone()
        }
        fn find_caskroom_apps(&self, _app_bundle_name: &str) -> Vec<PathBuf> {
            self.find_hits.clone()
        }
    }

    #[test]
    fn token_validation() {
        assert!(is_valid_cask_token("visual-studio-code"));
        assert!(is_valid_cask_token("iterm2"));
        assert!(!is_valid_cask_token("Visual-Studio"));
        assert!(!is_valid_cask_token(""));
        assert!(!is_valid_cask_token("-bad"));
    }

    #[test]
    fn extract_token_from_caskroom() {
        assert_eq!(
            extract_cask_token_from_caskroom_path(Path::new(
                "/opt/homebrew/Caskroom/iterm2/3.5.0/iTerm.app"
            ))
            .as_deref(),
            Some("iterm2")
        );
        assert_eq!(
            extract_cask_token_from_caskroom_path(Path::new(
                "/usr/local/Caskroom/foo-bar/1.0/Foo.app"
            ))
            .as_deref(),
            Some("foo-bar")
        );
        assert!(
            extract_cask_token_from_caskroom_path(Path::new("/Applications/Foo.app")).is_none()
        );
    }

    #[test]
    fn rule_id_roundtrip() {
        let id = encode_brew_cask_rule_id(ZapMode::Zap, "iterm2");
        assert_eq!(id, "uninstall:brew-cask:zap:iterm2");
        assert_eq!(
            parse_brew_cask_rule_id(&id),
            Some((ZapMode::Zap, "iterm2".into()))
        );
        let id2 = encode_brew_cask_rule_id(ZapMode::NoZap, "iterm2");
        assert_eq!(
            parse_brew_cask_rule_id(&id2),
            Some((ZapMode::NoZap, "iterm2".into()))
        );
        assert!(parse_brew_cask_rule_id("uninstall:com.example").is_none());
    }

    #[test]
    fn detect_stage1_resolved_caskroom() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Foo.app");
        fs::create_dir_all(&app).unwrap();
        let deps = FakeBrewDeps {
            available: true,
            resolve: Some(PathBuf::from("/opt/homebrew/Caskroom/foo/1.0/Foo.app")),
            ..FakeBrewDeps::empty()
        };
        assert_eq!(detect_cask_name(&deps, &app).as_deref(), Some("foo"));
    }

    #[test]
    fn detect_none_when_brew_missing() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Foo.app");
        fs::create_dir_all(&app).unwrap();
        let deps = FakeBrewDeps {
            available: false,
            ..FakeBrewDeps::empty()
        };
        assert!(detect_cask_name(&deps, &app).is_none());
    }

    #[test]
    fn detect_stage2_ambiguous_tokens_none() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Foo.app");
        fs::create_dir_all(&app).unwrap();
        let deps = FakeBrewDeps {
            available: true,
            find_hits: vec![
                PathBuf::from("/opt/homebrew/Caskroom/a/1/Foo.app"),
                PathBuf::from("/opt/homebrew/Caskroom/b/1/Foo.app"),
            ],
            ..FakeBrewDeps::empty()
        };
        assert!(detect_cask_name(&deps, &app).is_none());
    }

    #[test]
    fn timeout_scales_with_size() {
        assert_eq!(brew_uninstall_timeout_secs(None, 0), 300);
        assert_eq!(brew_uninstall_timeout_secs(None, 6 * GIB), 600);
        assert_eq!(brew_uninstall_timeout_secs(None, 16 * GIB), 900);
    }

    #[test]
    fn fake_uninstall_records_zap_flag() {
        let deps = FakeBrewDeps {
            available: true,
            uninstall_ok: true,
            ..FakeBrewDeps::empty()
        };
        deps.uninstall_cask("foo", ZapMode::Zap, None).unwrap();
        assert!(deps.last_uninstall.lock().unwrap().as_ref().unwrap().zap);
        deps.uninstall_cask("foo", ZapMode::NoZap, None).unwrap();
        assert!(!deps.last_uninstall.lock().unwrap().as_ref().unwrap().zap);
    }
}
