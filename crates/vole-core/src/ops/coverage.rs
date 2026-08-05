//! Clean / apply 覆盖说明与权限响亮提示。

use super::plan::PlanNotice;
use crate::rules::Rule;
use crate::vole_proto::{Report, SkipReason};

/// Mole v1.48.1 库存总量（`scripts/inventory-mole-rules.py`）。
pub const MOLE_INVENTORY_TOTAL: u32 = 513;

/// orphan Library 不可访问时追加到当次 coverage_note / 人读 stderr 的警告。
pub const ORPHAN_LIBRARY_WARN: &str = "注意：orphaned-app-data 已跳过（无法读取 ~/Library/Caches 或安装扫描失败）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。";

/// uninstall / optimize / clean apply 出现权限或保护跳过时的警告。
pub const APPLY_PERMISSION_WARN: &str = "注意：部分条目因权限或系统保护被跳过。若涉及用户库数据，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole；系统路径可能需 sudo，请改用 Mole 或具备相应权限的环境后重试。";

/// system services 三树皆不可读时追加；不提 FDA。
pub const SYSTEM_SERVICES_WARN: &str = "注意：orphaned-system-services 已跳过（无法读取 /Library/LaunchDaemons、LaunchAgents 或 PrivilegedHelperTools）。当前扫描不使用 sudo，结果为可读子集；完整清理请使用 Mole 或具备相应权限的环境。系统路径候选即使出现在 plan 中，apply 也会 NeedsPrivilege 硬跳过（发现优先，Vole 不删除）。";

/// 已启用、未 `disabled` 的规则数。
pub fn enabled_rule_count(rules: &[Rule]) -> usize {
    rules.iter().filter(|r| !r.disabled).count()
}

/// plan / `--json-stream` `done` 用的覆盖说明文案。
pub fn coverage_note(enabled_rules: usize) -> String {
    format!(
        "本版本启用 {enabled_rules} 条清理规则（Mole v1.48.1 库存约 {MOLE_INVENTORY_TOTAL} 条）。\
         产品 v2 CLI（clean / uninstall / optimize）已达；用户域 orphaned app data（Caches/Logs/Saved State）、\
         Claude Desktop workspace VM orphan、system services orphan（/Library LaunchDaemons/Agents/PHT 可读子集；发现优先，删除请用 Mole/sudo）、\
         Toolbox keep-N、Codex staging、not_running（精确名 + cmdline）、\
         FCP / 剪映 generated、XCTestDevices 已落地。\
         仍未移植：真 sudo 删除、Containers stubs、其它 sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）。\
         如需完整清理，请继续使用 Mole：https://github.com/tw93/Mole"
    )
}

/// 当次 plan 若有 orphan / system-services degrade notice，追加固定警告。
pub fn coverage_with_orphan_notices(base: &str, notices: &[PlanNotice]) -> String {
    let mut out = base.to_string();
    if notices.contains(&PlanNotice::OrphanLibraryInaccessible) {
        out = format!("{out}\n{ORPHAN_LIBRARY_WARN}");
    }
    if notices.contains(&PlanNotice::SystemServicesInaccessible) {
        out = format!("{out}\n{SYSTEM_SERVICES_WARN}");
    }
    out
}

/// apply report 是否含权限/保护类 skip。
pub fn report_has_permission_skips(report: &Report) -> bool {
    report
        .skipped_by_reason
        .iter()
        .any(|s| matches!(s.reason, SkipReason::TccDenied | SkipReason::NeedsPrivilege))
}

/// 有权限类 skip 时追加 `APPLY_PERMISSION_WARN`（用于 `--json` report）。
pub fn coverage_with_apply_permission_hint(base: Option<&str>, report: &Report) -> Option<String> {
    if !report_has_permission_skips(report) {
        return base.map(str::to_string);
    }
    match base.map(str::trim).filter(|s| !s.is_empty()) {
        Some(b) => Some(format!("{b}\n{APPLY_PERMISSION_WARN}")),
        None => Some(APPLY_PERMISSION_WARN.to_string()),
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
        assert!(note.contains("可读子集"));
        assert!(note.contains("仍未移植"));
        let unported = note.split("仍未移植：").nth(1).expect("unported section");
        assert!(
            !unported.contains("Claude"),
            "Claude VM must not remain in the unported list"
        );
        assert!(
            !unported.contains("system services orphan"),
            "system services orphan readable subset must not remain unported"
        );
        assert!(
            !note.contains("仍未移植：orphaned apps"),
            "must not claim user-domain orphaned is still unported"
        );
        assert!(unported.contains("真 sudo 删除"));
        assert!(unported.contains("Containers stubs"));
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

        let sys = coverage_with_orphan_notices(
            &base,
            &[crate::ops::PlanNotice::SystemServicesInaccessible],
        );
        assert!(sys.contains(SYSTEM_SERVICES_WARN));
        assert!(!sys.contains("完全磁盘访问权限"));
        assert!(sys.contains("NeedsPrivilege"));
    }

    #[test]
    fn apply_permission_hint_helpers() {
        use crate::vole_proto::SkipSummary;

        let empty = Report::default();
        assert!(!report_has_permission_skips(&empty));
        assert_eq!(coverage_with_apply_permission_hint(None, &empty), None);
        assert_eq!(
            coverage_with_apply_permission_hint(Some("base"), &empty).as_deref(),
            Some("base")
        );

        let whitelist_only = Report {
            skipped_by_reason: vec![SkipSummary {
                reason: SkipReason::Whitelisted,
                count: 1,
                rule_ids: vec!["r".into()],
            }],
            ..Report::default()
        };
        assert!(!report_has_permission_skips(&whitelist_only));

        let tcc = Report {
            skipped_by_reason: vec![SkipSummary {
                reason: SkipReason::TccDenied,
                count: 2,
                rule_ids: vec!["a".into()],
            }],
            ..Report::default()
        };
        assert!(report_has_permission_skips(&tcc));
        let note = coverage_with_apply_permission_hint(Some("plan note"), &tcc).unwrap();
        assert!(note.starts_with("plan note\n"));
        assert!(note.contains(APPLY_PERMISSION_WARN));

        let priv_only = Report {
            skipped_by_reason: vec![SkipSummary {
                reason: SkipReason::NeedsPrivilege,
                count: 1,
                rule_ids: vec!["b".into()],
            }],
            ..Report::default()
        };
        assert!(report_has_permission_skips(&priv_only));
        assert_eq!(
            coverage_with_apply_permission_hint(None, &priv_only).as_deref(),
            Some(APPLY_PERMISSION_WARN)
        );
    }
}
