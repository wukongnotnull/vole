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
}

#[derive(Debug, Clone)]
pub struct ProtectionCatalog {
    data_protected: Vec<Pattern>,
    /// cleanup 模式合并 system + data pattern（对齐 mole `should_protect_path` step 6）。
    cleanup_patterns: Vec<Pattern>,
}

impl ProtectionCatalog {
    pub fn embedded() -> Self {
        let file: ProtectionFile = toml::from_str(EMBEDDED_TOML).expect("protection.toml parse");
        let system_critical = compile_patterns(&file.system_critical_bundles);
        let data_protected = compile_patterns(&file.data_protected_bundles);
        let mut cleanup_patterns = system_critical;
        cleanup_patterns.extend(data_protected.clone());
        Self {
            data_protected,
            cleanup_patterns,
        }
    }

    pub fn matches_cleanup_pattern(&self, text: &str) -> bool {
        self.cleanup_patterns.iter().any(|p| p.matches(text))
    }

    pub fn matches_data_protected(&self, text: &str) -> bool {
        self.data_protected.iter().any(|p| p.matches(text))
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
