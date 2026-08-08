//! 编排骨架：进度事件经 channel 发出，供 CLI/TUI/sidecar 消费。

mod apply_plan;
mod clean_hints;
mod coverage;
mod optimize_apply;
mod optimize_plan;
mod plan;
mod proto_plan;
mod installer_apply;
mod installer_plan;
mod purge_apply;
mod purge_plan;
mod uninstall_apply;
mod uninstall_plan;

use crate::vole_proto::StreamEvent;
use crossbeam_channel::Sender;
use std::sync::Arc;
use thiserror::Error;

use crate::cancel::{CancelToken, Cancelled};
use crate::orphan::{orphan_deps_for_runtime, OrphanDeps};
use crate::rules::{PgrepProcessProbe, ProcessProbe, StrategyBuildError};

pub use apply_plan::{
    apply_plan, apply_proto_plan, ApplyPlanContext, ApplyPlanError, ApplyPlanOptions,
};
pub use clean_hints::{
    collect_clean_hints, quick_hint_target_names, CleanHints, CleanHintsOptions, DuPathSize,
    HintItem, HintKind, PathSizeKb, DEFAULT_HINT_SCAN_BUDGET_SECS,
};
pub use coverage::{
    coverage_note, coverage_with_apply_permission_hint, coverage_with_orphan_notices,
    enabled_rule_count, report_has_permission_skips, APPLY_PERMISSION_WARN,
    GROUP_CONTAINERS_TRUNCATED_WARN, GROUP_CONTAINERS_WARN, HANDOFF_PASTEBOARD_TRUNCATED_WARN,
    HANDOFF_PASTEBOARD_WARN, MOLE_INVENTORY_TOTAL, ORPHAN_LIBRARY_WARN, SYSTEM_SERVICES_WARN,
    TIME_MACHINE_BUSY_WARN,
};
pub use optimize_apply::{
    apply_optimize_plan, apply_optimize_proto_plan, OptimizeApplyContext, OptimizeApplyError,
    OptimizeApplyOptions,
};
pub use optimize_plan::{build_optimize_plan, OptimizePlanError, OptimizePlanOptions};
pub use plan::{Plan, PlanBuilder, PlanEntry, PlanNotice, DEFAULT_PLAN_TTL};
pub use proto_plan::{plan_to_proto, ProtoPlanError};
pub use installer_apply::{
    apply_installer_plan, apply_installer_proto_plan, InstallerApplyContext, InstallerApplyError,
    InstallerApplyOptions,
};
pub use installer_plan::{
    build_installer_plan, resolve_default_scan_roots, InstallerPlanError, InstallerPlanOptions,
    DEFAULT_INSTALLER_SCAN_MAX_DEPTH,
};
pub use purge_apply::{
    apply_purge_plan, apply_purge_proto_plan, PurgeApplyContext, PurgeApplyError, PurgeApplyOptions,
};
pub use purge_plan::{
    build_purge_plan, is_project_root_for_hints, is_protected_purge_artifact,
    quick_hint_search_roots, PurgePlanError, PurgePlanOptions, DEFAULT_PURGE_MIN_AGE_DAYS,
    PURGE_TARGETS, QUICK_HINT_EXCLUDED_TARGETS,
};
pub use uninstall_apply::{
    apply_uninstall_plan, apply_uninstall_proto_plan, UninstallApplyContext, UninstallApplyError,
    UninstallApplyOptions,
};
pub use uninstall_plan::{
    build_uninstall_plan, build_uninstall_plan_with_brew, default_applications_dirs,
    scan_applications, UninstallPlanOptions,
};

#[derive(Debug, Error)]
pub enum OpsError {
    #[error("操作已取消")]
    Cancelled,
    #[error("strategy build failed: {0}")]
    Strategy(#[from] StrategyBuildError),
}

impl From<Cancelled> for OpsError {
    fn from(_: Cancelled) -> Self {
        OpsError::Cancelled
    }
}

pub struct Orchestrator {
    cancel: CancelToken,
    events: Option<Sender<StreamEvent>>,
    process_probe: Arc<dyn ProcessProbe>,
    pub(crate) orphan_deps: Arc<dyn OrphanDeps>,
}

impl Orchestrator {
    pub fn new(cancel: CancelToken, events: Option<Sender<StreamEvent>>) -> Self {
        Self::with_process_probe(cancel, events, Arc::new(PgrepProcessProbe))
    }

    pub fn with_process_probe(
        cancel: CancelToken,
        events: Option<Sender<StreamEvent>>,
        process_probe: Arc<dyn ProcessProbe>,
    ) -> Self {
        Self {
            cancel,
            events,
            process_probe,
            orphan_deps: orphan_deps_for_runtime(),
        }
    }

    pub fn with_orphan_deps(mut self, orphan_deps: Arc<dyn OrphanDeps>) -> Self {
        self.orphan_deps = orphan_deps;
        self
    }

    pub fn check_cancel(&self) -> Result<(), OpsError> {
        self.cancel.check()?;
        Ok(())
    }

    pub fn emit(&self, event: StreamEvent) {
        if let Some(tx) = &self.events {
            let _ = tx.send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vole_proto::StreamEvent;
    use crossbeam_channel::unbounded;

    #[test]
    fn channel_receives_progress() {
        let (tx, rx) = unbounded();
        let orch = Orchestrator::new(CancelToken::new(), Some(tx));
        orch.emit(StreamEvent::Progress {
            scanned: 42,
            current: ".".into(),
        });
        let ev = rx.try_recv().unwrap();
        assert!(matches!(ev, StreamEvent::Progress { scanned: 42, .. }));
    }

    #[test]
    fn cancel_propagates() {
        let token = CancelToken::new();
        token.cancel();
        let orch = Orchestrator::new(token, None);
        assert!(matches!(orch.check_cancel(), Err(OpsError::Cancelled)));
    }
}
