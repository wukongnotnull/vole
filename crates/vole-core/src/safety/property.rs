//! Property tests：删除目标与保护清单交集恒为空（设计 7C / Phase 4a Task 6）。

use proptest::prelude::*;

use crate::protection::{should_protect_path, AppProtection, ProtectionCatalog, ProtectionMode};
use crate::safety::{validate_path_for_deletion, NoPathProtection, ValidationError};

const DANGEROUS_CORPUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fuzz_corpus/dangerous_paths.txt"
));

fn corpus_paths() -> Vec<&'static str> {
    DANGEROUS_CORPUS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// 不变量：`should_protect_path` ⇒ `validate_path_for_deletion` 失败。
fn protected_implies_validate_rejects(path: &str) {
    let catalog = ProtectionCatalog::embedded();
    if !should_protect_path(path, &catalog, ProtectionMode::Cleanup) {
        return;
    }
    let protection = AppProtection::new();
    let result = validate_path_for_deletion(path, &protection);
    assert!(
        result.is_err(),
        "protected path must not pass deletion gate: {path:?} => {result:?}"
    );
}

/// 不变量：`validate_path_for_deletion` 成功 ⇒ 不在保护清单。
fn allowed_implies_not_protected(path: &str) {
    let protection = AppProtection::new();
    if validate_path_for_deletion(path, &protection).is_err() {
        return;
    }
    let catalog = ProtectionCatalog::embedded();
    assert!(
        !should_protect_path(path, &catalog, ProtectionMode::Cleanup),
        "allowed deletion target must not be protected: {path:?}"
    );
}

fn arb_path_segment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Caches".to_string()),
        Just("Preferences".to_string()),
        Just("Application Support".to_string()),
        Just("Group Containers".to_string()),
        Just("Containers".to_string()),
        Just("Logs".to_string()),
        Just("tmp".to_string()),
        "[a-zA-Z0-9._-]{1,24}".prop_map(|s| s),
    ]
}

fn arb_absolute_path() -> impl Strategy<Value = String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/test".into());
    let h1 = home.clone();
    let h2 = home.clone();
    let h3 = home.clone();
    let h4 = home.clone();
    prop_oneof![
        (arb_path_segment(), arb_path_segment(), arb_path_segment())
            .prop_map(move |(a, b, c)| format!("{home}/Library/{a}/{b}/{c}")),
        arb_path_segment().prop_map(|s| format!("/private/tmp/vole-prop-{s}")),
        arb_path_segment().prop_map(|s| format!("/tmp/vole-prop-{s}")),
        arb_path_segment().prop_map(move |s| format!("{h1}/Library/Caches/ms-playwright/{s}")),
        arb_path_segment().prop_map(move |s| {
            format!("{h2}/Library/Group Containers/group.com.apple.notes/{s}")
        }),
        arb_path_segment().prop_map(move |s| format!("{h3}/Library/Keychains/{s}")),
        arb_path_segment().prop_map(|s| format!("/System/{s}")),
        arb_path_segment().prop_map(|s| format!("/Library/{s}")),
        arb_path_segment().prop_map(|s| format!("/Applications/{s}.app")),
        arb_path_segment().prop_map(|s| format!("/Users/../{s}")),
        Just("/".to_string()),
        Just("/System".to_string()),
        Just("/private/tmp".to_string()),
        Just(format!("{h4}/Library/Caches/ordinary-app/cache.db")),
    ]
}

#[test]
fn corpus_dangerous_paths_all_rejected() {
    let protection = AppProtection::new();
    for path in corpus_paths() {
        assert!(
            validate_path_for_deletion(path, &protection).is_err(),
            "corpus path must be rejected: {path:?}"
        );
    }
}

#[test]
fn known_safe_tmp_child_allowed_and_unprotected() {
    let path = "/private/tmp/vole-prop-ordinary-cache-item";
    let protection = AppProtection::new();
    assert!(validate_path_for_deletion(path, &protection).is_ok());
    assert!(!should_protect_path(path, &ProtectionCatalog::embedded(), ProtectionMode::Cleanup));
    let _ = NoPathProtection;
    let _ = ValidationError::Empty;
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_protected_never_allowed(path in arb_absolute_path()) {
        protected_implies_validate_rejects(&path);
    }

    #[test]
    fn prop_allowed_never_protected(path in arb_absolute_path()) {
        allowed_implies_not_protected(&path);
    }

    #[test]
    fn prop_corpus_seed_never_allowed(idx in 0usize..200usize) {
        let paths = corpus_paths();
        prop_assume!(!paths.is_empty());
        let path = paths[idx % paths.len()];
        let protection = AppProtection::new();
        prop_assert!(
            validate_path_for_deletion(path, &protection).is_err(),
            "corpus seed must be rejected: {}",
            path
        );
    }
}
