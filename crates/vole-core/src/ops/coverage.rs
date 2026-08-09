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
pub const SYSTEM_SERVICES_WARN: &str = "注意：orphaned-system-services 已跳过（无法读取 /Library/LaunchDaemons、LaunchAgents 或 PrivilegedHelperTools）。当前扫描不使用 sudo（可读子集）；apply 在非交互 sudo 可用时永久删除；TTY 下无凭证时可至多一次请求管理员权限（sudo -v）后再 sudo -n，否则 NeedsPrivilege。";

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

/// Time Machine 忙或状态未知时追加。
pub const TIME_MACHINE_BUSY_WARN: &str =
    "注意：tm-failed-backups 已跳过（Time Machine 正在备份或状态未知）。请待备份空闲后重试。";

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
         idleassetsd `CFNetworkDownload_*.tmp`（*/T/* + ≥7 天 + sudo -n）、\
         `*.code_sign_clone`（*/X/* 目录 + sudo -n，跳过 EDR）、\
         GPU Metal caches（*/C/*/com.apple.metal* 目录 stale + sudo -n，跳过 EDR）、\
         Install macOS*.app（≥14 天 + SWU fail-closed + 当前大版本 keep + sudo -n）、\
         Time Machine 失败中备份（≥48h inProgress + tmutil delete）、\
         optimize DNS/mDNS（system_maintenance / network_optimization + sudo -n）、\
         optimize memory_pressure_relief（高压时 sudo -n purge）、\
         optimize W2b③（network_stack / disk_permissions / periodic + sudo -n）、\
         optimize login_items_audit（只读审计损坏登录项；不删除；禁非特权 sfltool dumpbtm）、\
         optimize spotlight_orphan_rules_cleanup（仅删确认已卸载 app 的 Spotlight 搜索规则；System/Apple/不确定 keep）、\
         optimize spotlight_index_optimize（AC+慢探针才 sudo -n mdutil -E；与 system_maintenance 只读检查去重）、\
         optimize shared_file_list_repair（仅删 plutil -lint 失败的 .sfl2/.sfl3；跳过 ApplicationRecentDocuments；禁 sfltool）、\
         optimize disk_verify（须 VOLE_ENABLE_DISK_VERIFY=1；超时 diskutil verifyVolume /；禁 repair；可能卡住）、\
         本地快照报告（status/analyze · 仅 list）、\
         Filo production Cache、\
         Zed system-node npm cache、\
         Antigravity browser Cache、\
         Antigravity / Chrome DevTools MCP profile siblings、\
         QQ Music Mac AS（iRRCache/iLog/iCache/iTemp）、\
         Chrome DevTools MCP Cache、\
         uninstall Homebrew Cask 联动（`brew uninstall --cask`/`--zap`，sibling→nozap）、\
         uninstall Login Items（按名 osascript + LoginItems helper `launchctl bootout`，sibling/名冲突守卫）、\
         uninstall 系统 LaunchDaemons/`/Library` sudo 残留主路径（`PrivilegeBackend` unload + permanent）、\
         交互提权（TTY 下至多一次 `sudo -v` 缓存后仍 `sudo -n` 删）、\
         container stubs（CleanMyMac allowlist）、\
         Group Containers logs/caches（含受保护容器 Logs / bundle 命名日志）、\
         Handoff pasteboard（mtime>60min）、\
         Toolbox keep-N、Codex staging、not_running（精确名 + cmdline）、\
         FCP / 剪映 generated、XCTestDevices 已落地、\
         user.sh 广域 `~/Library/Caches/*` / `~/Library/Logs/*`（plan 目录递归 du + 父子重叠扣减；保护跳过子集仍 keep）。\
         桌面 SMAppService / 特权助手见 vole-macos（真机通道已验收）。\
         如需完整清理（含 Developer 大户整树等长尾），请继续使用 Mole：https://github.com/tw93/Mole"
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
    if notices.contains(&PlanNotice::TimeMachineBusy) {
        out = format!("{out}\n{TIME_MACHINE_BUSY_WARN}");
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
    fn coverage_note_mentions_zed_system_node_npm_cache() {
        let note = coverage_note(540);
        assert!(note.contains("Zed system-node npm cache"));
        assert!(note.contains("已落地"));
    }

    #[test]
    fn coverage_note_mentions_antigravity_browser_cache() {
        let note = coverage_note(540);
        assert!(note.contains("Antigravity browser Cache"));
        assert!(note.contains("已落地"));
    }

    #[test]
    fn coverage_note_mentions_chrome_devtools_mcp_cache() {
        let note = coverage_note(540);
        assert!(note.contains("Chrome DevTools MCP Cache"));
        assert!(note.contains("已落地"));
    }

    #[test]
    fn coverage_note_mentions_batch6_siblings_and_qq_music_as() {
        let note = coverage_note(540);
        assert!(note.contains("profile siblings"));
        assert!(note.contains("QQ Music Mac AS"));
        assert!(note.contains("已落地"));
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
        assert!(
            !note.contains("仍未移植"),
            "no explicit unported product gaps remain after D1 helper channel"
        );
        assert!(note.contains("Claude pending-uploads"));
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
        assert!(note.contains("交互提权（TTY 下至多一次 `sudo -v`"));
        assert!(note.contains("Install macOS*.app（≥14 天"));
        assert!(note.contains("Time Machine 失败中备份（≥48h"));
        assert!(note.contains("optimize DNS/mDNS"));
        assert!(note.contains("optimize memory_pressure_relief"));
        assert!(note.contains("optimize W2b③"));
        assert!(note.contains("optimize login_items_audit"));
        assert!(note.contains("optimize spotlight_orphan_rules_cleanup"));
        assert!(note.contains("optimize spotlight_index_optimize"));
        assert!(note.contains("optimize shared_file_list_repair"));
        assert!(note.contains("optimize disk_verify"));
        assert!(note.contains("本地快照报告（status/analyze"));
        assert!(note.contains("Filo production Cache"));
        assert!(note.contains("Zed system-node npm cache"));
        assert!(note.contains("uninstall Homebrew Cask 联动"));
        assert!(note.contains("uninstall Login Items"));
        assert!(note.contains("uninstall 系统 LaunchDaemons"));
        assert!(
            !note.contains("仍未移植：桌面 SMAppService"),
            "desktop SMAppService helper channel must not remain listed as unported"
        );
        assert!(
            note.contains("桌面 SMAppService / 特权助手见 vole-macos"),
            "coverage should point at vole-macos helper channel after D1 acceptance"
        );
        assert!(note.contains("Rosetta `/Library` update bundle"));
        assert!(note.contains("Group Containers logs/caches"));
        assert!(note.contains("含受保护容器 Logs"));
        assert!(note.contains("Handoff pasteboard"));
        assert!(note.contains("user.sh 广域"));
        assert!(note.contains("`~/Library/Caches/*`"));
        assert!(
            !note.contains("受保护容器与 bundle 命名文件除外"),
            "partial Group Containers caveat must be removed"
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
