//! `should_protect_data`（对齐 mole `app_protection.sh`）。

use super::data::ProtectionCatalog;
use super::glob_match::bundle_matches_pattern;

/// 快速路径 glob（来自 mole `should_protect_data` case 分支）。
const BUNDLE_GUARD_PATTERNS: &[&str] = &[
    "com.apple.*",
    "loginwindow",
    "dock",
    "systempreferences",
    "finder",
    "safari",
    "org.cups.*",
    "backgroundtaskmanagement*",
    "keychain*",
    "security*",
    "bluetooth*",
    "wifi*",
    "network*",
    "tcc",
    "notification*",
    "accessibility*",
    "universalaccess*",
    "HIToolbox*",
    "*inputmethod*",
    "*InputMethod*",
    "*IME",
    "textinput*",
    "TextInput*",
    "keyboard*",
    "Keyboard*",
    "inputsource*",
    "InputSource*",
    "keylayout*",
    "KeyLayout*",
    "GlobalPreferences",
    ".GlobalPreferences",
    "org.pqrs.Karabiner*",
    "com.1password.*",
    "com.agilebits.*",
    "com.lastpass.*",
    "com.dashlane.*",
    "com.bitwarden.*",
    "com.jetbrains.*",
    "JetBrains*",
    "com.microsoft.*",
    "com.visualstudio.*",
    "com.sublimetext.*",
    "com.sublimehq.*",
    "Cursor",
    "Claude",
    "ChatGPT",
    "com.openai.codex",
    "Codex",
    "codex-runtimes",
    "Ollama",
    "com.clash.app",
    "com.nssurge.*",
    "com.v2ray.*",
    "com.clash.*",
    "ClashX*",
    "Surge*",
    "Shadowrocket*",
    "Quantumult*",
    "clash-*",
    "Clash-*",
    "*-clash",
    "*-Clash",
    "clash.*",
    "Clash.*",
    "clash_*",
    "*clash-verge*",
    "*Clash-Verge*",
    "clashverge*",
    "ClashVerge*",
    "com.docker.*",
    "com.getpostman.*",
    "com.insomnia.*",
];

const VENDOR_PREFIX_FALLBACK: &[&str] = &[
    "com.tencent.",
    "com.sogou.",
    "com.baidu.",
    "com.googlecode.",
    "im.rime.",
];

pub fn should_protect_data(bundle_id: &str, catalog: &ProtectionCatalog) -> bool {
    if bundle_id.is_empty() {
        return false;
    }

    for pat in BUNDLE_GUARD_PATTERNS {
        if !bundle_matches_pattern(bundle_id, pat) {
            continue;
        }
        if VENDOR_PREFIX_FALLBACK
            .iter()
            .any(|pfx| bundle_id.starts_with(pfx))
        {
            return catalog.matches_data_protected(bundle_id);
        }
        return true;
    }

    catalog.matches_data_protected(bundle_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_apple_and_orbstack() {
        let cat = ProtectionCatalog::embedded();
        assert!(should_protect_data("com.apple.finder", &cat));
        assert!(should_protect_data("dev.orbstack.OrbStack", &cat));
        assert!(should_protect_data("dev.kdrag0n.MacVirt", &cat));
    }

    #[test]
    fn protects_native_instruments() {
        let cat = ProtectionCatalog::embedded();
        assert!(should_protect_data(
            "com.native-instruments.NativeAccess",
            &cat
        ));
    }
}
