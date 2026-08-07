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
pub const SYSTEM_SERVICES_WARN: &str = "注意：orphaned-system-services 已跳过（无法读取 /Library/LaunchDaemons、LaunchAgents 或 PrivilegedHelperTools）。当前扫描不使用 sudo（可读子集）；apply 在非交互 sudo 可用时永久删除，无凭证则 NeedsPrivilege（可先执行 sudo -v）。";

/// `~/Library/Containers` 不可列时追加（container stubs 规则降级）。
pub const CONTAINER_STUBS_WARN: &str = "注意：orphaned-container-stubs 已跳过（无法读取 ~/Library/Containers）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。";

/// `~/Library/Group Containers` 不可列时追加。
pub const GROUP_CONTAINERS_WARN: &str = "注意：group-container-caches 已跳过（无法读取 ~/Library/Group Containers）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。";

/// 候选规模上限触发时追加（非整规则 degrade）。
pub const GROUP_CONTAINERS_TRUNCATED_WARN: &str = "注意：group-container-caches 部分候选子树因条目过多已跳过（单树 >200 或整规则 >2000）。可用 Mole 清理或缩小范围后重试。";

/// Handoff pasteboard 根不可列时追加。
pub const HANDOFF_PASTEBOARD_WARN: &str = "注意：handoff-pasteboard-cache 已跳过（无法读取 Handoff shared-pasteboard）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。";

/// Handoff 条目过多截断提示。
pub const HANDOFF_PASTEBOARD_TRUNCATED_WARN: &str =
    "注意：handoff-pasteboard-cache 因条目过多已截断（整规则 >2000）。可用 Mole 清理或稍后再试。";

/// 已启用、未 `disabled` 的规则数。
pub fn enabled_rule_count(rules: &[Rule]) -> usize {
    rules.iter().filter(|r| !r.disabled).count()
}

/// plan / `--json-stream` `done` 用的覆盖说明文案。
pub fn coverage_note(enabled_rules: usize) -> String {
    format!(
        "本版本启用 {enabled_rules} 条清理规则（Mole v1.48.1 库存约 {MOLE_INVENTORY_TOTAL} 条）。\
         产品 v2 CLI（clean / uninstall / optimize）已达；用户域 orphaned app data（Caches/Logs/Saved State）、\
         Claude Desktop workspace VM orphan、Claude pending-uploads、\
         system services orphan（/Library LaunchDaemons/Agents/PHT 可读子集 plan + sudo -n apply 真删）、\
         Rosetta `/Library` update bundle（arm64 + sudo -n）、\
         Icon Services 系统缓存（sudo -n）、\
         系统 DiagnosticReports（≥7 天叶 + sudo -n）、\
         `/private/var/log` 旧日志（≤5 深 + ≥7 天 + sudo -n）、\
         `/private/var/db/diagnostics`（≥7 天 / .tracev3 ≥30 天 + sudo -n）、\
         `/private/var/db/DiagnosticPipeline`（≥7 天 + sudo -n）、\
         `/private/var/db/powerlog`（≥7 天 + sudo -n）、\
         MemoryLimitViolations（≥30 天 + sudo -n）、\
         Adobe 系统日志（Adobe/CreativeCloud/adobegc ≥7 天 + sudo -n）、\
         `/private/tmp` + `/private/var/tmp`（深度 1 + ≥7 天 + sudo -n）、\
         `/Library/Caches` `*.cache`/`*.tmp`/`*.log`（≤5 深 + ≥7 天 + sudo -n）、\
         container stubs（CleanMyMac allowlist）、\
         Group Containers logs/caches（含受保护容器 Logs / bundle 命名日志）、\
         Handoff pasteboard（mtime>60min）、\
         Toolbox keep-N、Codex staging、not_running（精确名 + cmdline）、\
         FCP / 剪映 generated、XCTestDevices 已落地。\
         仍未移植：system.sh 其余提权路径、交互提权 / 桌面特权助手。\
         如需完整清理，请继续使用 Mole：https://github.com/tw93/Mole"
    )
}

/// 当次 plan 若有 orphan / system-services / stubs / group-containers / handoff degrade notice，追加固定警告。
pub fn coverage_with_orphan_notices(base: &str, notices: &[PlanNotice]) -> String {
    let mut out = base.to_string();
    if notices.contains(&PlanNotice::OrphanLibraryInaccessible) {
        out = format!("{out}\n{ORPHAN_LIBRARY_WARN}");
    }
    if notices.contains(&PlanNotice::SystemServicesInaccessible) {
        out = format!("{out}\n{SYSTEM_SERVICES_WARN}");
    }
    if notices.contains(&PlanNotice::ContainersInaccessible) {
        out = format!("{out}\n{CONTAINER_STUBS_WARN}");
    }
    if notices.contains(&PlanNotice::GroupContainersInaccessible) {
        out = format!("{out}\n{GROUP_CONTAINERS_WARN}");
    }
    if notices.contains(&PlanNotice::GroupContainersTruncated) {
        out = format!("{out}\n{GROUP_CONTAINERS_TRUNCATED_WARN}");
    }
    if notices.contains(&PlanNotice::HandoffPasteboardInaccessible) {
        out = format!("{out}\n{HANDOFF_PASTEBOARD_WARN}");
    }
    if notices.contains(&PlanNotice::HandoffPasteboardTruncated) {
        out = format!("{out}\n{HANDOFF_PASTEBOARD_TRUNCATED_WARN}");
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
        assert!(note.contains("Claude pending-uploads"));
        assert!(
            !unported.contains("claude pending-uploads"),
            "claude pending-uploads must not remain unported"
        );
        assert!(
            !unported.contains("pending-uploads"),
            "pending-uploads must not remain unported"
        );
        assert!(
            !unported.contains("system services orphan"),
            "system services orphan readable subset must not remain unported"
        );
        assert!(
            !note.contains("仍未移植：orphaned apps"),
            "must not claim user-domain orphaned is still unported"
        );
        assert!(note.contains("Icon Services 系统缓存"));
        assert!(note.contains("系统 DiagnosticReports"));
        assert!(note.contains("`/private/var/log` 旧日志"));
        assert!(note.contains("`/private/var/db/diagnostics`"));
        assert!(note.contains("`/private/var/db/DiagnosticPipeline`"));
        assert!(note.contains("`/private/var/db/powerlog`"));
        assert!(note.contains("MemoryLimitViolations"));
        assert!(note.contains("Adobe 系统日志"));
        assert!(unported.contains("交互提权") || unported.contains("桌面"));
        assert!(
            !unported.contains("Rosetta"),
            "Rosetta /Library must not remain unported"
        );
        assert!(note.contains("Rosetta `/Library` update bundle"));
        assert!(
            !unported.contains("真 sudo 删除"),
            "system-services sudo -n apply must not leave '真 sudo 删除' as wholesale unported"
        );
        assert!(
            !unported.contains("Group Containers 泛清理"),
            "group container caches coverage is shipped"
        );
        assert!(note.contains("Group Containers logs/caches"));
        assert!(note.contains("含受保护容器 Logs"));
        assert!(note.contains("Handoff pasteboard"));
        assert!(
            !unported.contains("受保护容器的组容器缓存"),
            "protected group container caches must not remain unported"
        );
        assert!(
            !note.contains("受保护容器与 bundle 命名文件除外"),
            "partial Group Containers caveat must be removed"
        );
        assert!(
            !unported.contains("Containers stubs"),
            "container stubs allowlist must not remain unported"
        );
        assert!(note.contains("container stubs（CleanMyMac allowlist）"));
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

        let stubs =
            coverage_with_orphan_notices(&base, &[crate::ops::PlanNotice::ContainersInaccessible]);
        assert!(stubs.contains(CONTAINER_STUBS_WARN));
        assert!(stubs.contains("完全磁盘访问权限"));

        let gcc = coverage_with_orphan_notices(
            &base,
            &[crate::ops::PlanNotice::GroupContainersInaccessible],
        );
        assert!(gcc.contains(GROUP_CONTAINERS_WARN));
        assert!(gcc.contains("完全磁盘访问权限"));

        let trunc = coverage_with_orphan_notices(
            &base,
            &[crate::ops::PlanNotice::GroupContainersTruncated],
        );
        assert!(trunc.contains(GROUP_CONTAINERS_TRUNCATED_WARN));
        assert!(trunc.contains("条目过多"));

        let handoff = coverage_with_orphan_notices(
            &base,
            &[crate::ops::PlanNotice::HandoffPasteboardInaccessible],
        );
        assert!(handoff.contains(HANDOFF_PASTEBOARD_WARN));
        assert!(handoff.contains("完全磁盘访问权限"));

        let handoff_trunc = coverage_with_orphan_notices(
            &base,
            &[crate::ops::PlanNotice::HandoffPasteboardTruncated],
        );
        assert!(handoff_trunc.contains(HANDOFF_PASTEBOARD_TRUNCATED_WARN));
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
