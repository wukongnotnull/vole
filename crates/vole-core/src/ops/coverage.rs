//! Clean 规则覆盖说明（plan 阶段提示未移植 mole 类别）。

use super::plan::PlanNotice;
use crate::rules::Rule;

/// Mole v1.48.1 库存总量（`scripts/inventory-mole-rules.py`）。
pub const MOLE_INVENTORY_TOTAL: u32 = 513;

/// orphan Library 不可访问时追加到当次 coverage_note / 人读 stderr 的警告。
pub const ORPHAN_LIBRARY_WARN: &str = "注意：orphaned-app-data 已跳过（无法读取 ~/Library/Caches 或安装扫描失败）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。";

/// 已启用、未 `disabled` 的规则数。
pub fn enabled_rule_count(rules: &[Rule]) -> usize {
    rules.iter().filter(|r| !r.disabled).count()
}

/// plan / `--json-stream` `done` 用的覆盖说明文案。
pub fn coverage_note(enabled_rules: usize) -> String {
    format!(
        "本版本启用 {enabled_rules} 条清理规则（Mole v1.48.1 库存约 {MOLE_INVENTORY_TOTAL} 条）。\
         产品 v2 CLI（clean / uninstall / optimize）已达；用户域 orphaned app data（Caches/Logs/Saved State）、\
         Claude Desktop workspace VM orphan、Toolbox keep-N、Codex staging、not_running（精确名 + cmdline）、\
         FCP / 剪映 generated、XCTestDevices 已落地。\
         仍未移植：system services orphan、Containers stubs、sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）。\
         如需完整清理，请继续使用 Mole：https://github.com/tw93/Mole"
    )
}

/// 当次 plan 若有 orphan degrade notice，追加固定警告（用于 json / plan-out）。
pub fn coverage_with_orphan_notices(base: &str, notices: &[PlanNotice]) -> String {
    if notices.contains(&PlanNotice::OrphanLibraryInaccessible) {
        format!("{base}\n{ORPHAN_LIBRARY_WARN}")
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{GuardsConfig, Rule, StrategyConfig};

    fn stub_rule(disabled: bool) -> Rule {
        Rule {
            id: "x".into(),
            category: None,
            label: "Test".into(),
            platform: vec![],
            paths: vec!["~/tmp/*".into()],
            impact: None,
            disabled,
            last_verified: None,
            strategy: StrategyConfig::default(),
            guards: GuardsConfig::default(),
        }
    }

    #[test]
    fn enabled_rule_count_excludes_disabled() {
        let rules = vec![stub_rule(false), stub_rule(true), stub_rule(false)];
        assert_eq!(enabled_rule_count(&rules), 2);
    }

    #[test]
    fn coverage_note_mentions_mole_and_count() {
        let note = coverage_note(150);
        assert!(note.contains("150"));
        assert!(note.contains("513"));
        assert!(note.contains("tw93/Mole"));
        assert!(note.contains("产品 v2 CLI"));
        assert!(note.contains("Toolbox keep-N"));
        assert!(note.contains("已落地"));
        assert!(note.contains("orphaned app data"));
        assert!(note.contains("Claude Desktop workspace VM orphan"));
        assert!(note.contains("system services orphan"));
        assert!(note.contains("仍未移植"));
        let unported = note.split("仍未移植：").nth(1).expect("unported section");
        assert!(
            !unported.contains("Claude"),
            "Claude VM must not remain in the unported list"
        );
        assert!(
            !note.contains("仍未移植：orphaned apps"),
            "must not claim user-domain orphaned is still unported"
        );
    }

    #[test]
    fn coverage_with_orphan_notices_appends_only_when_present() {
        let base = coverage_note(10);
        let plain = coverage_with_orphan_notices(&base, &[]);
        assert_eq!(plain, base);
        assert!(!plain.contains("orphaned-app-data 已跳过"));

        let with = coverage_with_orphan_notices(
            &base,
            &[crate::ops::PlanNotice::OrphanLibraryInaccessible],
        );
        assert!(with.contains(&base));
        assert!(with.contains(ORPHAN_LIBRARY_WARN));
        assert!(with.contains("完全磁盘访问权限"));
    }
}
