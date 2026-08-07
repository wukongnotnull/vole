//! Homebrew Cask 检测与卸载联动（W2a①）。
//!
//! 对齐 Mole `lib/uninstall/brew.sh`：多阶段检测 + `brew uninstall --cask [--zap]`。

use std::path::Path;

/// plan/apply 用的 brew-cask rule_id 前缀。
pub const BREW_CASK_RULE_PREFIX: &str = "uninstall:brew-cask:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZapMode {
    Zap,
    NoZap,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
}
