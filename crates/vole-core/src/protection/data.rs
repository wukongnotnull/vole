//! 从 mole `app_protection_data.sh` 加载的保护清单。

use glob::Pattern;
use serde::Deserialize;

const EMBEDDED_TOML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/protection.toml"
));

#[derive(Debug, Deserialize)]
struct ProtectionFile {
    system_critical_bundles: Vec<String>,
    data_protected_bundles: Vec<String>,
    #[serde(default)]
    apple_uninstallable_apps: Vec<String>,
    #[serde(default)]
    official_uninstaller_rules: Vec<String>,
}

/// `vendor|prefixes,...|fragments,...`（对齐 mole `OFFICIAL_UNINSTALLER_RULES`）。
#[derive(Debug, Clone)]
pub struct OfficialUninstallerRule {
    pub vendor: String,
    pub bundle_prefixes: Vec<String>,
    pub name_fragments: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProtectionCatalog {
    data_protected: Vec<Pattern>,
    system_critical: Vec<Pattern>,
    /// cleanup 模式合并 system + data pattern（对齐 mole `should_protect_path` step 6）。
    cleanup_patterns: Vec<Pattern>,
    apple_uninstallable: Vec<Pattern>,
    apple_uninstallable_raw: Vec<String>,
    official_uninstaller_rules: Vec<OfficialUninstallerRule>,
}

impl ProtectionCatalog {
    pub fn embedded() -> Self {
        let file: ProtectionFile = toml::from_str(EMBEDDED_TOML).expect("protection.toml parse");
        let system_critical = compile_patterns(&file.system_critical_bundles);
        let data_protected = compile_patterns(&file.data_protected_bundles);
        let apple_uninstallable = compile_patterns(&file.apple_uninstallable_apps);
        let mut cleanup_patterns = system_critical.clone();
        cleanup_patterns.extend(data_protected.clone());
        Self {
            data_protected,
            system_critical,
            cleanup_patterns,
            apple_uninstallable,
            apple_uninstallable_raw: file.apple_uninstallable_apps,
            official_uninstaller_rules: parse_official_rules(&file.official_uninstaller_rules),
        }
    }

    pub fn matches_cleanup_pattern(&self, text: &str) -> bool {
        self.cleanup_patterns.iter().any(|p| p.matches(text))
    }

    pub fn matches_system_critical(&self, text: &str) -> bool {
        self.system_critical.iter().any(|p| p.matches(text))
    }

    pub fn matches_data_protected(&self, text: &str) -> bool {
        self.data_protected.iter().any(|p| p.matches(text))
    }

    pub fn matches_apple_uninstallable(&self, text: &str) -> bool {
        self.apple_uninstallable.iter().any(|p| p.matches(text))
    }

    pub fn apple_uninstallable_patterns(&self) -> &[String] {
        &self.apple_uninstallable_raw
    }

    pub fn official_uninstaller_rules(&self) -> &[OfficialUninstallerRule] {
        &self.official_uninstaller_rules
    }

    #[cfg(test)]
    fn data_protected_patterns(&self) -> &[Pattern] {
        &self.data_protected
    }

    #[cfg(test)]
    fn cleanup_pattern_count(&self) -> usize {
        self.cleanup_patterns.len()
    }
}

fn compile_patterns(raw: &[String]) -> Vec<Pattern> {
    raw.iter().filter_map(|s| Pattern::new(s).ok()).collect()
}

fn parse_official_rules(raw: &[String]) -> Vec<OfficialUninstallerRule> {
    raw.iter()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '|');
            let vendor = parts.next()?.to_string();
            let prefixes = parts.next().unwrap_or("");
            let fragments = parts.next().unwrap_or("");
            Some(OfficialUninstallerRule {
                vendor,
                bundle_prefixes: prefixes
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
                name_fragments: fragments
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_loads() {
        let cat = ProtectionCatalog::embedded();
        assert!(!cat.data_protected_patterns().is_empty());
        assert!(cat.cleanup_pattern_count() >= cat.data_protected_patterns().len());
    }
}
