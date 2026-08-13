//! `uninstall` apply：TTL + TOCTOU + Uninstall 模式保护 + `mole_delete_verified`。
//! brew-cask 条目走 `BrewDeps::uninstall_cask`，失败仅在 cask 已卸载时回退 delete。
//! login-item / login-helper 侧车走 `LoginItemDeps`（不 mole_delete）。
//! system-leftover 侧车走既有 `PrivilegeBackend`（sudo -n + TTY sudo -v）。

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::Trash;

use crate::brew_cask::{
    detect_cask_name, parse_brew_cask_rule_id, BrewDeps, CaskInstallState, LiveBrewDeps,
};
use crate::delete::{
    mole_delete_verified, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions,
};
use crate::login_items::{
    is_bootout_allowed, login_name_collides, parse_login_helper_rule_id,
    parse_login_item_name_rule_id, LiveLoginItemDeps, LoginItemDeps, LoginItemError,
};
use crate::oplog::OperationLogger;
use crate::privilege::{
    path_allowed_for_privilege, NoPrivilege, PrivilegeBackend, PrivilegeError, SudoNoninteractive,
};
use crate::protection::{
    find_bundle_siblings, read_bundle_id, read_display_name, AppProtection, SiblingPresence,
    UninstallPathProtection,
};
use crate::safety::{
    verify_plan_entry_for_apply, PlanApplyError, PlanEntryIdentity, ValidationError,
};
use crate::system_leftovers::{
    parse_system_leftover_rule_id, SystemLeftoverKind, SYSTEM_LEFTOVER_PREFIX,
};
use crate::vole_proto::{
    Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, Report, SkipReason, SkipSummary, StreamEvent,
    SCHEMA_VERSION,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UninstallApplyError {
    #[error("plan expired; rescan with `vole uninstall --plan`")]
    Expired,
    #[error("unsupported plan schema version {got} (expected {expected})")]
    UnsupportedSchema { expected: u32, got: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct UninstallApplyOptions {
    pub permanent: bool,
}

pub struct UninstallApplyContext<'a> {
    pub protection: &'a AppProtection,
    pub whitelist_patterns: &'a [String],
    pub options: UninstallApplyOptions,
    pub trash: &'a dyn Trash,
    pub deletion_log: &'a DeletionLogger,
    pub oplog: &'a mut OperationLogger,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
    /// 注入 brew；None 则用 LiveBrewDeps（仅 brew-cask 条目会碰）。
    pub brew: Option<&'a dyn BrewDeps>,
    /// 注入 login items；None 则用 LiveLoginItemDeps。
    pub login_items: Option<&'a dyn LoginItemDeps>,
    /// 注入特权；None 则用 SudoNoninteractive（仅 system-leftover 会碰）。
    pub privilege: Option<&'a dyn PrivilegeBackend>,
    /// TTY `sudo -v` 本会话是否已尝试（至多一次）。
    pub privilege_acquire_attempted: bool,
    /// 与 plan 一致的 Applications 搜索根（sibling 守卫用）。
    pub applications_dirs: &'a [PathBuf],
}

pub fn apply_uninstall_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    options: UninstallApplyOptions,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, UninstallApplyError> {
    let deletion_log = DeletionLogger::from_env();
    let mut oplog = OperationLogger::new("uninstall");
    let _ = oplog.session_start();
    let live = LiveBrewDeps;
    let live_login = LiveLoginItemDeps;
    let sudo = SudoNoninteractive;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let apps_dirs = applications_dirs_for_apply(&home);
    let mut ctx = UninstallApplyContext {
        protection,
        whitelist_patterns: &[],
        options,
        trash: &vole_sys::macos::MacTrash,
        deletion_log: &deletion_log,
        oplog: &mut oplog,
        on_event,
        now: SystemTime::now(),
        brew: Some(&live),
        login_items: Some(&live_login),
        privilege: Some(&sudo),
        privilege_acquire_attempted: false,
        applications_dirs: &apps_dirs,
    };
    let report = apply_uninstall_proto_plan(plan, &mut ctx)?;
    let _ = oplog.session_end(
        report.succeeded,
        report.trashed_bytes / 1024 + report.deleted_bytes / 1024,
    );
    Ok(report)
}

fn ensure_privilege_ready(
    ctx: &mut UninstallApplyContext<'_>,
    backend: &dyn PrivilegeBackend,
) -> bool {
    if backend.probe_noninteractive() {
        return true;
    }
    if ctx.privilege_acquire_attempted {
        return false;
    }
    ctx.privilege_acquire_attempted = true;
    if io::stdin().is_terminal() {
        let _ = writeln!(io::stderr(), "正在请求管理员权限以清理系统路径…");
    }
    backend.acquire_interactive() && backend.probe_noninteractive()
}

pub fn apply_uninstall_proto_plan(
    plan: &ProtoPlan,
    ctx: &mut UninstallApplyContext<'_>,
) -> Result<Report, UninstallApplyError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(UninstallApplyError::UnsupportedSchema {
            expected: SCHEMA_VERSION,
            got: plan.schema_version,
        });
    }
    if plan_is_expired(plan, ctx.now) {
        return Err(UninstallApplyError::Expired);
    }

    let delete_mode = if ctx.options.permanent {
        DeleteMode::Permanent
    } else {
        DeleteMode::Trash
    };

    let mode_protection = UninstallPathProtection::new(ctx.protection);
    let mut succeeded = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;
    let mut trashed_bytes = 0u64;
    let mut deleted_bytes = 0u64;
    let mut skip_tracker = SkipTracker::default();

    for (idx, entry) in plan.entries.iter().enumerate() {
        if let Some(event) = &ctx.on_event {
            event(StreamEvent::Progress {
                scanned: idx as u64 + 1,
                current: entry.path.display().to_string(),
            });
        }

        if entry.skip_reason.is_some() {
            skipped += 1;
            let reason = entry
                .skip_reason
                .clone()
                .unwrap_or(SkipReason::PathVanished);
            if let Some(event) = &ctx.on_event {
                event(StreamEvent::Skipped {
                    rule_id: entry.rule_id.clone(),
                    reason: reason.clone(),
                });
            }
            skip_tracker.record(reason, &entry.rule_id);
            continue;
        }

        if !entry.rule_id.starts_with("uninstall:") {
            skipped += 1;
            skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            continue;
        }

        let path = entry.path.display().to_string();
        let identity = proto_identity(entry);
        let is_login_name = parse_login_item_name_rule_id(&entry.rule_id).is_some();
        let is_login_helper = parse_login_helper_rule_id(&entry.rule_id).is_some();
        let is_login_action = is_login_name || is_login_helper;

        if let Err(err) = verify_plan_entry_for_apply(&path, &identity, &mode_protection) {
            let reason = skip_reason_for_apply(&err);
            let allow_vanished = is_login_action && matches!(reason, SkipReason::PathVanished);
            if !allow_vanished {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: reason.clone(),
                    });
                }
                skip_tracker.record(reason, &entry.rule_id);
                continue;
            }
        }

        let live_login_fallback = LiveLoginItemDeps;
        let login_deps: &dyn LoginItemDeps = match ctx.login_items {
            Some(d) => d,
            None => &live_login_fallback,
        };

        if let Some(name) = parse_login_item_name_rule_id(&entry.rule_id) {
            // plan 不可信：rule_id 名必须与 entry.path 现读显示名一致（路径已消失则 skip）。
            if !login_item_name_matches_path(entry.path.as_path(), &name)
                || login_item_name_blocked(entry.path.as_path(), &name, ctx.applications_dirs)
            {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::Whitelisted,
                    });
                }
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
                continue;
            }
            match login_deps.remove_login_item(&name) {
                Ok(()) => {
                    succeeded += 1;
                }
                Err(LoginItemError::NeedsPrivilege) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::NeedsPrivilege,
                        });
                    }
                    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                }
                Err(LoginItemError::Failed(_)) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::PathVanished,
                        });
                    }
                    skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                }
            }
            continue;
        }

        if let Some(helper_id) = parse_login_helper_rule_id(&entry.rule_id) {
            if !is_bootout_allowed(&helper_id)
                || login_helper_blocked(entry.path.as_path(), ctx.applications_dirs)
            {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::Whitelisted,
                    });
                }
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
                continue;
            }
            let uid = current_uid();
            match login_deps.bootout_helper(uid, &helper_id) {
                Ok(()) => {
                    succeeded += 1;
                }
                Err(LoginItemError::NeedsPrivilege) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::NeedsPrivilege,
                        });
                    }
                    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                }
                Err(LoginItemError::Failed(_)) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::PathVanished,
                        });
                    }
                    skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                }
            }
            continue;
        }

        if entry.rule_id.starts_with(SYSTEM_LEFTOVER_PREFIX) {
            let fallback = NoPrivilege;
            let backend = ctx.privilege.unwrap_or(&fallback);
            let Some((kind, bundle_id, decoded)) = parse_system_leftover_rule_id(&entry.rule_id)
            else {
                skipped += 1;
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
                continue;
            };
            if decoded != entry.path {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            // plan/apply 窗口内可能新增 sibling：再检并 fail-closed 跳过。
            if system_leftover_sibling_blocks(&bundle_id) {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::Whitelisted,
                    });
                }
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
                continue;
            }
            if !path_allowed_for_privilege(&entry.path) {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::Whitelisted,
                    });
                }
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
                continue;
            }
            if !ensure_privilege_ready(ctx, backend) {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::NeedsPrivilege,
                    });
                }
                skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                let _ = writeln!(
                    io::stderr(),
                    "注意：系统残留条目需要非交互 sudo（可先执行 sudo -v）后重试。"
                );
                continue;
            }
            if matches!(kind, SystemLeftoverKind::Launchd)
                && entry.path.extension().and_then(|e| e.to_str()) == Some("plist")
            {
                let _ = backend.launchctl_unload(&entry.path);
            }
            match backend.remove_permanent(&entry.path) {
                Ok(()) => {
                    succeeded += 1;
                    deleted_bytes = deleted_bytes.saturating_add(entry.size);
                }
                Err(PrivilegeError::Unavailable) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::NeedsPrivilege,
                        });
                    }
                    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                }
                Err(PrivilegeError::Refused) | Err(PrivilegeError::CommandFailed(_)) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::PathVanished,
                        });
                    }
                    skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                }
            }
            continue;
        }

        if let Some((mode, token)) = parse_brew_cask_rule_id(&entry.rule_id) {
            let live_fallback = LiveBrewDeps;
            let brew: &dyn BrewDeps = match ctx.brew {
                Some(b) => b,
                None => &live_fallback,
            };
            // plan 不可信：token 必须与路径现检一致，否则不调 brew。
            match detect_cask_name(brew, entry.path.as_path()) {
                Some(detected) if detected == token => {}
                _ => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::PathVanished,
                        });
                    }
                    skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                    continue;
                }
            }
            match brew.uninstall_cask(&token, mode, Some(entry.path.as_path())) {
                Ok(()) => {
                    succeeded += 1;
                    // brew 可能已带走体积；不重复记账字节亦可
                    continue;
                }
                Err(_) => match brew.is_cask_installed(&token) {
                    CaskInstallState::NotInstalled => {
                        // 回退 mole_delete
                    }
                    CaskInstallState::Installed | CaskInstallState::Unknown => {
                        skipped += 1;
                        if let Some(event) = &ctx.on_event {
                            event(StreamEvent::Skipped {
                                rule_id: entry.rule_id.clone(),
                                reason: SkipReason::PathVanished,
                            });
                        }
                        skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                        continue;
                    }
                },
            }
        }

        let delete_opts = MoleDeleteOptions {
            mode: delete_mode,
            dry_run: false,
            needs_sudo: false,
            privilege: None,
        };

        match mole_delete_verified(
            &path,
            &identity,
            &mode_protection,
            ctx.whitelist_patterns,
            delete_opts,
            ctx.trash,
            ctx.deletion_log,
            ctx.oplog,
        ) {
            Ok(outcome) => {
                succeeded += 1;
                match delete_mode {
                    DeleteMode::Trash => trashed_bytes += outcome.bytes,
                    DeleteMode::Permanent => deleted_bytes += outcome.bytes,
                }
            }
            Err(MoleDeleteError::Whitelisted) => {
                skipped += 1;
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            }
            Err(MoleDeleteError::Rejected)
            | Err(MoleDeleteError::IdentityMismatch)
            | Err(MoleDeleteError::Vanished) => {
                skipped += 1;
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    let report = Report {
        succeeded,
        skipped,
        failed,
        skipped_by_reason: skip_tracker.into_summaries(),
        trashed_bytes,
        deleted_bytes,
        coverage_note: plan.coverage_note.clone(),
    };

    if let Some(event) = &ctx.on_event {
        event(StreamEvent::Done {
            report: report.clone(),
        });
    }

    Ok(report)
}

fn plan_is_expired(plan: &ProtoPlan, now: SystemTime) -> bool {
    let ttl = Duration::from_secs(plan.ttl_secs);
    plan.created_at
        .checked_add(ttl)
        .is_none_or(|expires| now > expires)
}

fn proto_identity(entry: &ProtoPlanEntry) -> PlanEntryIdentity {
    let mtime = entry
        .mtime
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64;
    PlanEntryIdentity {
        dev: entry.dev,
        ino: entry.ino,
        mtime,
    }
}

fn skip_reason_for_apply(err: &PlanApplyError) -> SkipReason {
    match err {
        PlanApplyError::Policy(ValidationError::EndpointSecurityCache) => SkipReason::TccDenied,
        PlanApplyError::Policy(ValidationError::ProtectedPath)
        | PlanApplyError::Policy(ValidationError::CriticalSystemPath)
        | PlanApplyError::Policy(ValidationError::SymlinkToCritical)
        | PlanApplyError::Policy(ValidationError::AncestorResolvesToCritical) => {
            SkipReason::NeedsPrivilege
        }
        _ => SkipReason::PathVanished,
    }
}

fn current_uid() -> u32 {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(0)
}

fn applications_dirs_for_apply(home: &Path) -> Vec<PathBuf> {
    if let Ok(raw) = std::env::var("VOLE_APPLICATIONS_DIR") {
        let dirs: Vec<PathBuf> = raw
            .split(':')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
        if !dirs.is_empty() {
            return dirs;
        }
    }
    crate::ops::uninstall_plan::default_applications_dirs(home)
}

fn sibling_presence_for_app(app_path: &Path, search_roots: &[PathBuf]) -> SiblingPresence {
    let Some(bundle_id) = read_bundle_id(app_path) else {
        return SiblingPresence::default();
    };
    let mut roots: Vec<PathBuf> = search_roots.to_vec();
    if let Some(parent) = app_path.parent() {
        if !roots.iter().any(|r| r == parent) {
            roots.push(parent.to_path_buf());
        }
    }
    find_bundle_siblings(&bundle_id, app_path, &roots)
}

fn sibling_display_names(siblings: &SiblingPresence) -> Vec<String> {
    siblings
        .other_app_paths
        .iter()
        .map(|p| {
            read_display_name(p).unwrap_or_else(|| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn login_item_name_matches_path(app_path: &Path, rule_name: &str) -> bool {
    if !app_path.exists() {
        return false;
    }
    let expected = read_display_name(app_path).unwrap_or_else(|| {
        app_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string()
    });
    let expected = expected.strip_suffix(".app").unwrap_or(expected.as_str());
    let got = rule_name.strip_suffix(".app").unwrap_or(rule_name);
    !expected.is_empty() && expected == got
}

fn login_item_name_blocked(app_path: &Path, display_name: &str, search_roots: &[PathBuf]) -> bool {
    if !app_path.exists() {
        return false;
    }
    let siblings = sibling_presence_for_app(app_path, search_roots);
    let names = sibling_display_names(&siblings);
    login_name_collides(display_name, &names)
}

fn login_helper_blocked(app_path: &Path, search_roots: &[PathBuf]) -> bool {
    if !app_path.exists() {
        return false;
    }
    sibling_presence_for_app(app_path, search_roots).has_siblings()
}

fn system_leftover_sibling_blocks(bundle_id: &str) -> bool {
    let home = std::env::var_os("VOLE_TEST_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let roots = crate::ops::uninstall_plan::default_applications_dirs(&home);
    // except 用占位路径：返回所有同 bundle 安装；≥2 即共享残留不可删。
    let phantom = PathBuf::from("/__vole_no_such_app__.app");
    let sib = find_bundle_siblings(bundle_id, &phantom, &roots);
    sib.other_app_paths.len() >= 2
}

#[derive(Default)]
struct SkipTracker {
    entries: Vec<SkipSummary>,
}

impl SkipTracker {
    fn record(&mut self, reason: SkipReason, rule_id: &str) {
        if let Some(summary) = self.entries.iter_mut().find(|s| s.reason == reason) {
            summary.count += 1;
            if !summary.rule_ids.iter().any(|id| id == rule_id) {
                summary.rule_ids.push(rule_id.to_string());
            }
            return;
        }
        self.entries.push(SkipSummary {
            reason,
            count: 1,
            rule_ids: vec![rule_id.to_string()],
        });
    }

    fn into_summaries(self) -> Vec<SkipSummary> {
        self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brew_cask::{encode_brew_cask_rule_id, ZapMode};
    use crate::delete::DeletionLogger;
    use crate::login_items::{
        encode_login_helper_rule_id, encode_login_item_name_rule_id, FakeLoginItemDeps,
        LoginItemError,
    };
    use crate::oplog::OperationLogger;
    use crate::protection::AppProtection;
    use crate::safety::capture_plan_entry_identity;
    use crate::test_env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use vole_sys::macos::MacTrash;

    struct RecordingBrew {
        uninstall_ok: bool,
        install_state: CaskInstallState,
        last: Mutex<Option<(String, bool)>>,
        /// Stage1 resolve；须导向与 rule_id token 一致的 Caskroom 路径。
        resolve: Option<PathBuf>,
    }

    impl BrewDeps for RecordingBrew {
        fn brew_available(&self) -> bool {
            true
        }
        fn list_casks(&self) -> Option<Vec<String>> {
            Some(vec![])
        }
        fn cask_info(&self, _: &str) -> Option<String> {
            Some(String::new())
        }
        fn is_cask_installed(&self, _: &str) -> CaskInstallState {
            self.install_state
        }
        fn uninstall_cask(
            &self,
            token: &str,
            mode: ZapMode,
            _: Option<&Path>,
        ) -> Result<(), String> {
            *self.last.lock().unwrap() = Some((token.to_string(), matches!(mode, ZapMode::Zap)));
            if self.uninstall_ok {
                Ok(())
            } else {
                Err("fail".into())
            }
        }
        fn resolve_path(&self, _: &Path) -> Option<PathBuf> {
            self.resolve.clone()
        }
        fn read_symlink(&self, _: &Path) -> Option<PathBuf> {
            None
        }
        fn find_caskroom_apps(&self, _: &str) -> Vec<PathBuf> {
            vec![]
        }
    }

    fn foo_caskroom() -> PathBuf {
        PathBuf::from("/opt/homebrew/Caskroom/foo/1.0/Foo.app")
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vole-uninstall-apply-{tag}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn apply_trashes_entry_and_rejects_expired() {
        let _guard = test_env::lock();
        let root = scratch("ok");
        let file = root.join("victim.txt");
        fs::write(&file, b"hi").unwrap();
        let identity = capture_plan_entry_identity(&file).unwrap();
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: file.clone(),
            label: "victim".into(),
            size: 2,
            rule_id: "uninstall:com.example.foo".into(),
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: None,
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert!(!file.exists());

        let expired = ProtoPlan {
            created_at: UNIX_EPOCH,
            ttl_secs: 1,
            entries: vec![],
            ..plan
        };
        let err = apply_uninstall_proto_plan(&expired, &mut ctx).unwrap_err();
        assert_eq!(err, UninstallApplyError::Expired);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_brew_cask_calls_uninstall_zap() {
        let _guard = test_env::lock();
        let root = scratch("brew-ok");
        let file = root.join("Foo.app");
        fs::create_dir_all(&file).unwrap();
        let identity = capture_plan_entry_identity(&file).unwrap();
        let rule = encode_brew_cask_rule_id(ZapMode::Zap, "foo");
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: file.clone(),
            label: "Foo [Brew:foo]".into(),
            size: 1,
            rule_id: rule,
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };
        let brew = RecordingBrew {
            uninstall_ok: true,
            install_state: CaskInstallState::NotInstalled,
            last: Mutex::new(None),
            resolve: Some(foo_caskroom()),
        };
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: Some(&brew),
            login_items: None,
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        let last = brew.last.lock().unwrap().clone().unwrap();
        assert_eq!(last.0, "foo");
        assert!(last.1);
        // brew 成功不走 mole_delete，文件仍在
        assert!(file.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_brew_fail_still_installed_skips_delete() {
        let _guard = test_env::lock();
        let root = scratch("brew-keep");
        let file = root.join("Foo.app");
        fs::create_dir_all(&file).unwrap();
        let identity = capture_plan_entry_identity(&file).unwrap();
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: file.clone(),
            label: "Foo".into(),
            size: 1,
            rule_id: encode_brew_cask_rule_id(ZapMode::Zap, "foo"),
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };
        let brew = RecordingBrew {
            uninstall_ok: false,
            install_state: CaskInstallState::Installed,
            last: Mutex::new(None),
            resolve: Some(foo_caskroom()),
        };
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: Some(&brew),
            login_items: None,
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(file.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_brew_fail_cask_gone_falls_back_delete() {
        let _guard = test_env::lock();
        let root = scratch("brew-fallback");
        let file = root.join("Foo.app");
        fs::create_dir_all(&file).unwrap();
        let identity = capture_plan_entry_identity(&file).unwrap();
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: file.clone(),
            label: "Foo".into(),
            size: 1,
            rule_id: encode_brew_cask_rule_id(ZapMode::NoZap, "foo"),
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };
        let brew = RecordingBrew {
            uninstall_ok: false,
            install_state: CaskInstallState::NotInstalled,
            last: Mutex::new(None),
            resolve: Some(foo_caskroom()),
        };
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: Some(&brew),
            login_items: None,
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert!(!file.exists());
        let last = brew.last.lock().unwrap().clone().unwrap();
        assert!(!last.1); // nozap
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_brew_skips_when_token_mismatches_path() {
        let _guard = test_env::lock();
        let root = scratch("brew-mismatch");
        let file = root.join("Foo.app");
        fs::create_dir_all(&file).unwrap();
        let identity = capture_plan_entry_identity(&file).unwrap();
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: file.clone(),
            label: "Foo".into(),
            size: 1,
            // 声称卸载 victim，但路径实际解析为 foo
            rule_id: encode_brew_cask_rule_id(ZapMode::Zap, "victim"),
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };
        let brew = RecordingBrew {
            uninstall_ok: true,
            install_state: CaskInstallState::Installed,
            last: Mutex::new(None),
            resolve: Some(foo_caskroom()),
        };
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: Some(&brew),
            login_items: None,
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(brew.last.lock().unwrap().is_none());
        assert!(file.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_login_item_calls_remove() {
        let _guard = test_env::lock();
        let root = scratch("login-item");
        let app = root.join("Foo.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.foo</string>
<key>CFBundleName</key><string>Foo</string>
</dict></plist>"#,
        )
        .unwrap();
        let identity = capture_plan_entry_identity(&app).unwrap();
        let rule = encode_login_item_name_rule_id("Foo").unwrap();
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: app.clone(),
            label: "Login Item: Foo".into(),
            size: 0,
            rule_id: rule,
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };
        let fake = FakeLoginItemDeps::default();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: Some(&fake),
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(fake.removed_names.lock().unwrap().as_slice(), ["Foo"]);
        assert!(app.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_login_helper_calls_bootout() {
        let _guard = test_env::lock();
        let root = scratch("login-helper");
        let app = root.join("Foo.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.foo</string>
</dict></plist>"#,
        )
        .unwrap();
        let identity = capture_plan_entry_identity(&app).unwrap();
        let rule = encode_login_helper_rule_id("com.example.foo.helper");
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: app.clone(),
            label: "helper".into(),
            size: 0,
            rule_id: rule,
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };
        let fake = FakeLoginItemDeps::default();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: Some(&fake),
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        let booted = fake.booted_helpers.lock().unwrap().clone();
        assert_eq!(booted.len(), 1);
        assert_eq!(booted[0].1, "com.example.foo.helper");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_login_item_needs_privilege_skips_loudly() {
        let _guard = test_env::lock();
        let root = scratch("login-priv");
        let app = root.join("Foo.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.foo</string>
</dict></plist>"#,
        )
        .unwrap();
        let leftover = root.join("leftover.txt");
        fs::write(&leftover, b"x").unwrap();
        let id_app = capture_plan_entry_identity(&app).unwrap();
        let id_left = capture_plan_entry_identity(&leftover).unwrap();
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![
                ProtoPlanEntry {
                    id: "li".into(),
                    path: app.clone(),
                    label: "Login Item".into(),
                    size: 0,
                    rule_id: encode_login_item_name_rule_id("Foo").unwrap(),
                    skip_reason: None,
                    dev: id_app.dev,
                    ino: id_app.ino,
                    mtime: UNIX_EPOCH + Duration::from_secs(id_app.mtime.max(0) as u64),
                    blockers: Vec::new(),
                },
                ProtoPlanEntry {
                    id: "left".into(),
                    path: leftover.clone(),
                    label: "leftover".into(),
                    size: 1,
                    rule_id: "uninstall:leftover:com.example.foo".into(),
                    skip_reason: None,
                    dev: id_left.dev,
                    ino: id_left.ino,
                    mtime: UNIX_EPOCH + Duration::from_secs(id_left.mtime.max(0) as u64),
                    blockers: Vec::new(),
                },
            ],
            coverage_note: None,
        };
        let fake = FakeLoginItemDeps::default();
        *fake.remove_error.lock().unwrap() = Some(LoginItemError::NeedsPrivilege);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: Some(&fake),
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(report.skipped, 1);
        assert!(report
            .skipped_by_reason
            .iter()
            .any(|s| s.reason == SkipReason::NeedsPrivilege));
        assert!(!leftover.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_skips_login_item_when_rule_name_mismatches_path() {
        let _guard = test_env::lock();
        let root = scratch("login-mismatch");
        let app = root.join("Foo.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.foo</string>
<key>CFBundleName</key><string>Foo</string>
</dict></plist>"#,
        )
        .unwrap();
        let identity = capture_plan_entry_identity(&app).unwrap();
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "1".into(),
                path: app.clone(),
                label: "evil".into(),
                size: 0,
                rule_id: encode_login_item_name_rule_id("VictimOtherApp").unwrap(),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: Vec::new(),
            }],
            coverage_note: None,
        };
        let fake = FakeLoginItemDeps::default();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: Some(&fake),
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(fake.removed_names.lock().unwrap().is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_skips_bootout_when_sibling_in_other_apps_root() {
        let _guard = test_env::lock();
        let root = scratch("login-sib-roots");
        let apps_a = root.join("ApplicationsA");
        let apps_b = root.join("ApplicationsB");
        fs::create_dir_all(&apps_a).unwrap();
        fs::create_dir_all(&apps_b).unwrap();
        let foo = apps_a.join("Foo.app");
        let copy = apps_b.join("Foo Copy.app");
        for (app, name) in [(&foo, "Foo"), (&copy, "Foo Copy")] {
            fs::create_dir_all(app.join("Contents")).unwrap();
            fs::write(
                app.join("Contents/Info.plist"),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.foo</string>
<key>CFBundleName</key><string>{name}</string>
</dict></plist>"#
                ),
            )
            .unwrap();
        }
        let identity = capture_plan_entry_identity(&foo).unwrap();
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "h".into(),
                path: foo.clone(),
                label: "helper".into(),
                size: 0,
                rule_id: encode_login_helper_rule_id("com.example.foo.helper"),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: Vec::new(),
            }],
            coverage_note: None,
        };
        let fake = FakeLoginItemDeps::default();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let roots = [apps_a.clone(), apps_b.clone()];
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: Some(&fake),
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &roots,
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(fake.booted_helpers.lock().unwrap().is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_skips_bootout_when_sibling_present() {
        let _guard = test_env::lock();
        let root = scratch("login-sib");
        let apps = root.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        let foo = apps.join("Foo.app");
        let copy = apps.join("Foo Copy.app");
        for (app, name) in [(&foo, "Foo"), (&copy, "Foo Copy")] {
            fs::create_dir_all(app.join("Contents")).unwrap();
            fs::write(
                app.join("Contents/Info.plist"),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.foo</string>
<key>CFBundleName</key><string>{name}</string>
</dict></plist>"#
                ),
            )
            .unwrap();
        }
        let identity = capture_plan_entry_identity(&foo).unwrap();
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "h".into(),
                path: foo.clone(),
                label: "helper".into(),
                size: 0,
                rule_id: encode_login_helper_rule_id("com.example.foo.helper"),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: Vec::new(),
            }],
            coverage_note: None,
        };
        let fake = FakeLoginItemDeps::default();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: Some(&fake),
            privilege: None,
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(fake.booted_helpers.lock().unwrap().is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_system_leftover_unload_and_remove() {
        let _guard = crate::test_env::lock();
        let root = scratch("sys-leftover-ok");
        let lib = root.join("Library");
        fs::create_dir_all(lib.join("LaunchDaemons")).unwrap();
        let plist = lib.join("LaunchDaemons/com.example.sys.plist");
        fs::write(&plist, b"{}").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let identity = capture_plan_entry_identity(&plist).unwrap();
        let rule = crate::system_leftovers::encode_system_leftover_rule_id(
            crate::system_leftovers::SystemLeftoverKind::Launchd,
            "com.example.sys",
            &plist,
        );
        let entry = ProtoPlanEntry {
            id: "sys1".into(),
            path: plist.clone(),
            label: "System leftover: com.example.sys.plist".into(),
            size: 2,
            rule_id: rule,
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };

        let fake = crate::privilege::RecordingPrivilege::allowing();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: None,
            privilege: Some(&fake),
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(fake.unloaded.lock().unwrap().len(), 1);
        assert_eq!(fake.removed.lock().unwrap().len(), 1);
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
    }

    #[test]
    fn apply_system_leftover_needs_privilege_when_denied() {
        let _guard = crate::test_env::lock();
        let root = scratch("sys-leftover-deny");
        let lib = root.join("Library");
        fs::create_dir_all(lib.join("LaunchDaemons")).unwrap();
        let plist = lib.join("LaunchDaemons/com.example.deny.plist");
        fs::write(&plist, b"{}").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let identity = capture_plan_entry_identity(&plist).unwrap();
        let rule = crate::system_leftovers::encode_system_leftover_rule_id(
            crate::system_leftovers::SystemLeftoverKind::Launchd,
            "com.example.sys",
            &plist,
        );
        let entry = ProtoPlanEntry {
            id: "sys2".into(),
            path: plist.clone(),
            label: "System leftover".into(),
            size: 2,
            rule_id: rule,
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };

        let fake = crate::privilege::RecordingPrivilege::denying();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: None,
            privilege: Some(&fake),
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert!(report.skipped >= 1);
        assert!(fake.removed.lock().unwrap().is_empty());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
    }

    #[test]
    fn apply_system_leftover_skips_when_sibling_appears() {
        let _guard = crate::test_env::lock();
        let root = scratch("sys-leftover-sib");
        let home = root.join("home");
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        // two copies same bundle
        for name in ["Foo.app", "Foo Copy.app"] {
            let app = apps.join(name);
            fs::create_dir_all(app.join("Contents")).unwrap();
            let info = app.join("Contents/Info.plist");
            let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.sib</string>
<key>CFBundleName</key><string>Foo</string>
</dict></plist>"#;
            fs::write(&info, body).unwrap();
        }
        let lib = root.join("Library");
        fs::create_dir_all(lib.join("LaunchDaemons")).unwrap();
        let plist = lib.join("LaunchDaemons/com.example.sib.plist");
        fs::write(&plist, b"{}").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);
        std::env::set_var("VOLE_TEST_HOME", &home);
        std::env::set_var("HOME", &home);

        let identity = capture_plan_entry_identity(&plist).unwrap();
        let rule = crate::system_leftovers::encode_system_leftover_rule_id(
            crate::system_leftovers::SystemLeftoverKind::Launchd,
            "com.example.sib",
            &plist,
        );
        let entry = ProtoPlanEntry {
            id: "sys3".into(),
            path: plist.clone(),
            label: "System leftover".into(),
            size: 2,
            rule_id: rule,
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
            blockers: Vec::new(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };
        let fake = crate::privilege::RecordingPrivilege::allowing();
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("uninstall");
        let trash = MacTrash;
        let mut ctx = UninstallApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: UninstallApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            brew: None,
            login_items: None,
            privilege: Some(&fake),
            privilege_acquire_attempted: false,
            applications_dirs: &[],
        };
        let report = apply_uninstall_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert!(fake.removed.lock().unwrap().is_empty());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        std::env::remove_var("VOLE_TEST_HOME");
        // leave HOME as was — restore to original via remove and rely on lock
        std::env::remove_var("HOME");
    }
}
