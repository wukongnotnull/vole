//! Bundle ID glob 匹配（对齐 mole `bundle_matches_pattern`）。

use glob::Pattern;

pub fn bundle_matches_pattern(text: &str, pattern: &str) -> bool {
    Pattern::new(pattern)
        .map(|p| p.matches(text))
        .unwrap_or(false)
}
