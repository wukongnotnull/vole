//! Canonical optimize task metadata (aligned with Mole `lib/optimize/catalog.sh`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizeTaskKind {
    Delete,
    Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizeTask {
    pub id: &'static str,
    pub kind: OptimizeTaskKind,
    pub title: &'static str,
    pub description: &'static str,
    /// `true` = M3 main path; `false` = long-tail → coverage_note only.
    pub in_m3: bool,
}

const CATALOG: &[OptimizeTask] = &[
    OptimizeTask {
        id: "system_maintenance",
        kind: OptimizeTaskKind::Action,
        title: "DNS & Spotlight Check",
        description: "Refresh DNS cache & verify Spotlight status",
        in_m3: true,
    },
    OptimizeTask {
        id: "cache_refresh",
        kind: OptimizeTaskKind::Delete,
        title: "Finder Cache Refresh",
        description: "Refresh QuickLook thumbnails & icon services cache",
        in_m3: true,
    },
    OptimizeTask {
        id: "saved_state_cleanup",
        kind: OptimizeTaskKind::Delete,
        title: "App State Cleanup",
        description: "Remove old saved application states (30+ days)",
        in_m3: true,
    },
    OptimizeTask {
        id: "fix_broken_configs",
        kind: OptimizeTaskKind::Delete,
        title: "Broken Config Repair",
        description: "Fix corrupted preferences files",
        in_m3: true,
    },
    OptimizeTask {
        id: "network_optimization",
        kind: OptimizeTaskKind::Action,
        title: "Network Cache Refresh",
        description: "Optimize DNS cache & restart mDNSResponder",
        in_m3: true,
    },
    OptimizeTask {
        id: "sqlite_vacuum",
        kind: OptimizeTaskKind::Action,
        title: "Database Optimization",
        description: "Compress SQLite databases for Mail, Safari & Messages (skips if apps are running)",
        in_m3: true,
    },
    OptimizeTask {
        id: "launch_services_rebuild",
        kind: OptimizeTaskKind::Action,
        title: "LaunchServices Repair",
        description: "Repair \"Open with\" menu & file associations",
        in_m3: true,
    },
    OptimizeTask {
        id: "dock_refresh",
        kind: OptimizeTaskKind::Action,
        title: "Dock Refresh",
        description: "Fix broken icons and visual glitches in the Dock",
        in_m3: true,
    },
    OptimizeTask {
        id: "prevent_network_dsstore",
        kind: OptimizeTaskKind::Action,
        title: "Prevent Finder .DS_Store",
        description: "Set a persistent Finder preference to stop writing .DS_Store on SMB/AFP/NFS and USB volumes",
        in_m3: true,
    },
    OptimizeTask {
        id: "legacy_overrides_audit",
        kind: OptimizeTaskKind::Action,
        title: "Legacy Overrides",
        description: "Remove hidden App Nap and disk-image verification overrides left by old tweak tools",
        in_m3: true,
    },
    OptimizeTask {
        id: "memory_pressure_relief",
        kind: OptimizeTaskKind::Action,
        title: "Memory Optimization",
        description: "Release inactive memory to improve system responsiveness",
        in_m3: true,
    },
    OptimizeTask {
        id: "network_stack_optimize",
        kind: OptimizeTaskKind::Action,
        title: "Network Stack Refresh",
        description: "Flush routing table and ARP cache to resolve network issues",
        in_m3: true,
    },
    OptimizeTask {
        id: "disk_permissions_repair",
        kind: OptimizeTaskKind::Action,
        title: "Permission Repair",
        description: "Fix user directory permission issues",
        in_m3: true,
    },
    OptimizeTask {
        id: "spotlight_index_optimize",
        kind: OptimizeTaskKind::Action,
        title: "Spotlight Optimization",
        description: "Rebuild index if search is slow (smart detection)",
        in_m3: true,
    },
    OptimizeTask {
        id: "spotlight_orphan_rules_cleanup",
        kind: OptimizeTaskKind::Action,
        title: "Spotlight Orphan Rules",
        description: "Remove Spotlight search-rule entries for apps that are no longer installed",
        in_m3: true,
    },
    OptimizeTask {
        id: "periodic_maintenance",
        kind: OptimizeTaskKind::Action,
        title: "Periodic Maintenance",
        description: "Run macOS daily/weekly/monthly maintenance scripts if stale",
        in_m3: true,
    },
    OptimizeTask {
        id: "shared_file_list_repair",
        kind: OptimizeTaskKind::Action,
        title: "Shared File Lists",
        description: "Repair corrupted Finder favorites and recent documents",
        in_m3: true,
    },
    OptimizeTask {
        id: "disk_verify",
        kind: OptimizeTaskKind::Action,
        title: "Disk Health",
        description: "Verify filesystem integrity",
        in_m3: false,
    },
    OptimizeTask {
        id: "login_items_audit",
        kind: OptimizeTaskKind::Action,
        title: "Login Items",
        description: "Audit login items for broken entries",
        in_m3: true,
    },
    OptimizeTask {
        id: "quarantine_cleanup",
        kind: OptimizeTaskKind::Action,
        title: "Quarantine Database Cleanup",
        description: "Clear Gatekeeper download tracking history",
        in_m3: true,
    },
    OptimizeTask {
        id: "launch_agents_cleanup",
        kind: OptimizeTaskKind::Delete,
        title: "Launch Agents Cleanup",
        description: "Remove broken LaunchAgents whose binaries no longer exist",
        in_m3: true,
    },
    OptimizeTask {
        id: "notification_cleanup",
        kind: OptimizeTaskKind::Action,
        title: "Notifications",
        description: "Clean old delivered notifications to reduce database bloat",
        in_m3: true,
    },
    OptimizeTask {
        id: "coreduet_cleanup",
        kind: OptimizeTaskKind::Action,
        title: "Usage Data",
        description: "Clean old usage tracking data",
        in_m3: true,
    },
];

pub fn optimize_catalog() -> &'static [OptimizeTask] {
    CATALOG
}

pub fn optimize_delete_rule_id(task_id: &str) -> String {
    format!("optimize:delete:{task_id}")
}

pub fn optimize_action_rule_id(task_id: &str) -> String {
    format!("optimize:action:{task_id}")
}

pub fn parse_optimize_rule_id(rule_id: &str) -> Option<(OptimizeTaskKind, &str)> {
    if let Some(rest) = rule_id.strip_prefix("optimize:delete:") {
        return Some((OptimizeTaskKind::Delete, rest));
    }
    if let Some(rest) = rule_id.strip_prefix("optimize:action:") {
        return Some((OptimizeTaskKind::Action, rest));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_all_mole_actions() {
        let ids: Vec<_> = optimize_catalog().iter().map(|t| t.id).collect();
        for expected in [
            "system_maintenance",
            "cache_refresh",
            "saved_state_cleanup",
            "fix_broken_configs",
            "network_optimization",
            "sqlite_vacuum",
            "launch_services_rebuild",
            "dock_refresh",
            "prevent_network_dsstore",
            "legacy_overrides_audit",
            "memory_pressure_relief",
            "network_stack_optimize",
            "disk_permissions_repair",
            "spotlight_index_optimize",
            "spotlight_orphan_rules_cleanup",
            "periodic_maintenance",
            "shared_file_list_repair",
            "disk_verify",
            "login_items_audit",
            "quarantine_cleanup",
            "launch_agents_cleanup",
            "notification_cleanup",
            "coreduet_cleanup",
        ] {
            assert!(ids.contains(&expected), "missing {expected}");
        }
        assert_eq!(ids.len(), 23);
    }

    #[test]
    fn m3_main_path_flags() {
        let main: Vec<_> = optimize_catalog()
            .iter()
            .filter(|t| t.in_m3)
            .map(|t| t.id)
            .collect();
        assert!(main.contains(&"cache_refresh"));
        assert!(main.contains(&"saved_state_cleanup"));
        assert!(main.contains(&"system_maintenance"));
        assert!(main.contains(&"network_optimization"));
        assert!(main.contains(&"memory_pressure_relief"));
        assert!(main.contains(&"network_stack_optimize"));
        assert!(main.contains(&"disk_permissions_repair"));
        assert!(main.contains(&"periodic_maintenance"));
        assert!(main.contains(&"login_items_audit"));
        assert!(main.contains(&"spotlight_orphan_rules_cleanup"));
        assert!(main.contains(&"spotlight_index_optimize"));
        assert!(main.contains(&"shared_file_list_repair"));
        assert!(!main.contains(&"disk_verify"));
        assert_eq!(main.len(), 22);
    }

    #[test]
    fn rule_id_roundtrip() {
        let d = optimize_delete_rule_id("saved_state_cleanup");
        assert_eq!(d, "optimize:delete:saved_state_cleanup");
        assert_eq!(
            parse_optimize_rule_id(&d),
            Some((OptimizeTaskKind::Delete, "saved_state_cleanup"))
        );
        let a = optimize_action_rule_id("dock_refresh");
        assert_eq!(
            parse_optimize_rule_id(&a),
            Some((OptimizeTaskKind::Action, "dock_refresh"))
        );
        assert_eq!(parse_optimize_rule_id("uninstall:foo"), None);
    }
}
