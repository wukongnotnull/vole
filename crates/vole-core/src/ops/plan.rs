//! plan 生成管线：规则 → glob → 策略 → 安全闸口 → 身份快照。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::protection::AppProtection;
use crate::rules::{
    collect_path_candidates, resolve_strategy, select_custom, should_skip_for_not_running,
    PathEntry, ResolvedStrategy, Rule, Strategy,
};
use crate::safety::{
    capture_plan_entry_identity, validate_path_for_deletion, PlanEntryIdentity, ValidationError,
};
use crate::vole_proto::{SkipReason, StreamEvent};
use crate::whitelist;

use super::{OpsError, Orchestrator};

/// 默认 plan TTL（设计 5.6：15 分钟）。
pub const DEFAULT_PLAN_TTL: Duration = Duration::from_secs(900);

/// 单条 plan 条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    pub id: String,
    pub path: PathBuf,
    pub label: String,
    pub size: u64,
    pub rule_id: String,
    pub skip_reason: Option<String>,
    pub identity: Option<PlanEntryIdentity>,
}

/// 一次 plan 扫描的完整结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub generated_at: SystemTime,
    pub ttl: Duration,
    pub entries: Vec<PlanEntry>,
}

/// plan 生成配置。
#[derive(Debug, Clone)]
pub struct PlanBuilder {
    ttl: Duration,
}

impl Default for PlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanBuilder {
    pub fn new() -> Self {
        Self {
            ttl: DEFAULT_PLAN_TTL,
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn build(
        &self,
        orch: &Orchestrator,
        rules: &[Rule],
        protection: &AppProtection,
        whitelist_patterns: &[String],
    ) -> Result<Plan, OpsError> {
        orch.build_plan_with(rules, protection, whitelist_patterns, self.ttl)
    }
}

impl Orchestrator {
    /// 从规则列表生成 plan（默认 TTL 15 分钟）。
    pub fn build_plan(
        &self,
        rules: &[Rule],
        protection: &AppProtection,
        whitelist_patterns: &[String],
    ) -> Result<Plan, OpsError> {
        self.build_plan_with(rules, protection, whitelist_patterns, DEFAULT_PLAN_TTL)
    }

    pub(crate) fn build_plan_with(
        &self,
        rules: &[Rule],
        protection: &AppProtection,
        whitelist_patterns: &[String],
        ttl: Duration,
    ) -> Result<Plan, OpsError> {
        let home = home_dir();
        let mut entries = Vec::new();
        let mut scanned: u64 = 0;
        let mut next_id: u64 = 0;

        for rule in rules {
            self.check_cancel()?;

            if rule.disabled {
                continue;
            }

            if should_skip_for_not_running(
                self.process_probe.as_ref(),
                &rule.guards.not_running,
            ) {
                self.emit(StreamEvent::Skipped {
                    rule_id: rule.id.clone(),
                    reason: SkipReason::AppRunning,
                });
                continue;
            }

            let strategy = match resolve_strategy(&rule.strategy) {
                Ok(s) => s,
                Err(err) => return Err(OpsError::Strategy(err)),
            };

            let expanded = collect_path_candidates(rule, &home);
            let path_entries = build_path_entries(&expanded);
            let selected = match &strategy {
                ResolvedStrategy::Custom(custom) => {
                    select_custom(&custom.handler, &path_entries, &home, rule)
                }
                other => other.select(&path_entries),
            };

            for path in selected {
                self.check_cancel()?;

                scanned += 1;
                if scanned.is_multiple_of(16) {
                    self.emit(StreamEvent::Progress {
                        scanned,
                        current: path.display().to_string(),
                    });
                }

                let path_str = path.display().to_string();

                if let Err(err) = validate_path_for_deletion(&path_str, protection) {
                    self.emit_skipped(&rule.id, &err);
                    continue;
                }

                if whitelist::is_match(&path, whitelist_patterns) {
                    self.emit(StreamEvent::Skipped {
                        rule_id: rule.id.clone(),
                        reason: SkipReason::Whitelisted,
                    });
                    continue;
                }

                let identity = match capture_plan_entry_identity(&path) {
                    Ok(id) => id,
                    Err(_) => {
                        self.emit(StreamEvent::Skipped {
                            rule_id: rule.id.clone(),
                            reason: SkipReason::PathVanished,
                        });
                        continue;
                    }
                };

                let size = path_size(&path);
                let id = format!("{}-{}", rule.id, next_id);
                next_id += 1;

                self.emit(StreamEvent::Candidate {
                    id: id.clone(),
                    path: path_str,
                    label: rule.label.clone(),
                    size,
                    rule_id: rule.id.clone(),
                });

                entries.push(PlanEntry {
                    id,
                    path,
                    label: rule.label.clone(),
                    size,
                    rule_id: rule.id.clone(),
                    skip_reason: None,
                    identity: Some(identity),
                });
            }
        }

        self.emit(StreamEvent::Progress {
            scanned,
            current: String::new(),
        });

        Ok(Plan {
            generated_at: SystemTime::now(),
            ttl,
            entries,
        })
    }

    fn emit_skipped(&self, rule_id: &str, err: &ValidationError) {
        self.emit(StreamEvent::Skipped {
            rule_id: rule_id.to_string(),
            reason: skip_reason_for_validation(err),
        });
    }
}

fn home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("VOLE_TEST_HOME") {
        return PathBuf::from(home);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn build_path_entries(paths: &[PathBuf]) -> Vec<PathEntry> {
    paths
        .iter()
        .map(|path| {
            let mtime = fs::symlink_metadata(path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            PathEntry::new(path.clone(), mtime)
        })
        .collect()
}

fn path_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn skip_reason_for_validation(err: &ValidationError) -> SkipReason {
    match err {
        ValidationError::EndpointSecurityCache => SkipReason::TccDenied,
        ValidationError::ProtectedPath => SkipReason::NeedsPrivilege,
        ValidationError::CriticalSystemPath
        | ValidationError::SymlinkToCritical
        | ValidationError::AncestorResolvesToCritical => SkipReason::NeedsPrivilege,
        _ => SkipReason::PathVanished,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{FakeProcessProbe, StrategyConfig};
    use crate::test_env;
    use crossbeam_channel::unbounded;
    use std::collections::HashSet;
    use std::fs;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration as StdDuration;

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("vole-plan-{tag}-{}", std::process::id()))
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    fn all_rule(id: &str, paths: Vec<String>, disabled: bool) -> Rule {
        Rule {
            id: id.into(),
            category: None,
            label: format!("label-{id}"),
            platform: vec![],
            paths,
            impact: None,
            disabled,
            last_verified: None,
            strategy: StrategyConfig::default(),
            guards: Default::default(),
        }
    }

    #[test]
    fn disabled_rule_produces_no_entries() {
        let _guard = test_env::lock();
        let dir = scratch("disabled");
        let file = dir.join("skip-me");
        touch(&file);
        let pattern = file.to_string_lossy().into_owned();

        let rules = vec![all_rule("disabled-rule", vec![pattern], true)];
        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), None);
        let plan = orch.build_plan(&rules, &AppProtection::new(), &[]).unwrap();

        assert!(plan.entries.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn protected_path_is_skipped() {
        let _guard = test_env::lock();
        let home = scratch("protected-home");
        std::env::set_var("HOME", &home);
        let protected = home.join("Library/Caches/ms-playwright/chromium-123");
        touch(&protected);
        let pattern = protected.to_string_lossy().into_owned();

        let (tx, rx) = unbounded();
        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), Some(tx));
        let rules = vec![all_rule("prot", vec![pattern], false)];
        let plan = orch.build_plan(&rules, &AppProtection::new(), &[]).unwrap();

        assert!(plan.entries.is_empty());
        let ev = rx.try_recv().unwrap();
        assert!(matches!(
            ev,
            StreamEvent::Skipped {
                reason: SkipReason::NeedsPrivilege,
                ..
            }
        ));
        std::env::remove_var("HOME");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn whitelisted_path_is_skipped() {
        let _guard = test_env::lock();
        let dir = scratch("whitelist");
        let file = dir.join("real/Caches/cache.db");
        touch(&file);
        let pattern = file.to_string_lossy().into_owned();
        let whitelist = vec![format!("{}*", dir.display())];

        let (tx, rx) = unbounded();
        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), Some(tx));
        let rules = vec![all_rule("wl", vec![pattern], false)];
        let plan = orch
            .build_plan(&rules, &AppProtection::new(), &whitelist)
            .unwrap();

        assert!(plan.entries.is_empty());
        let ev = rx.try_recv().unwrap();
        assert!(matches!(
            ev,
            StreamEvent::Skipped {
                reason: SkipReason::Whitelisted,
                ..
            }
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ordinary_tmp_child_becomes_plan_entry_with_identity() {
        let _guard = test_env::lock();
        let dir = scratch("ordinary");
        let file = dir.join("real/Caches/cache.db");
        touch(&file);
        let pattern = file.to_string_lossy().into_owned();

        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), None);
        let rules = vec![all_rule("ok", vec![pattern], false)];
        let plan = orch.build_plan(&rules, &AppProtection::new(), &[]).unwrap();

        assert_eq!(plan.entries.len(), 1);
        let entry = &plan.entries[0];
        assert_eq!(entry.rule_id, "ok");
        assert_eq!(entry.label, "label-ok");
        assert!(entry.skip_reason.is_none());
        assert!(entry.identity.is_some());
        assert_eq!(entry.size, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_mid_run_returns_cancelled() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let _guard = test_env::lock();
        let dir = scratch("cancel");
        // Enough rules that cancel is observed at a rule boundary even on fast runners.
        let mut rules = Vec::new();
        for i in 0..32 {
            let file = dir.join(format!("r{i}/cache.db"));
            touch(&file);
            rules.push(all_rule(
                &format!("r{i}"),
                vec![file.to_string_lossy().into_owned()],
                false,
            ));
        }

        let token = crate::cancel::CancelToken::new();
        let token2 = token.clone();
        let orch = Orchestrator::new(token, None);
        let done = Arc::new(AtomicBool::new(false));
        let done2 = Arc::clone(&done);

        let handle = thread::spawn(move || {
            while !done2.load(Ordering::Relaxed) {
                token2.cancel();
                thread::sleep(StdDuration::from_micros(50));
            }
        });

        let result = orch.build_plan(&rules, &AppProtection::new(), &[]);
        done.store(true, Ordering::Relaxed);
        handle.join().unwrap();

        assert!(
            matches!(result, Err(OpsError::Cancelled)),
            "expected Cancelled, got {result:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_builder_uses_custom_ttl() {
        let builder = PlanBuilder::new().with_ttl(Duration::from_secs(60));
        assert_eq!(builder.ttl(), Duration::from_secs(60));
    }

    #[test]
    fn plan_skips_rule_when_not_running_guard_hits() {
        let _guard = test_env::lock();
        let dir = scratch("not-running-hit");
        let file = dir.join("real/.claude/sessions/old");
        touch(&file);
        let pattern = file.to_string_lossy().into_owned();

        let mut rule = all_rule("claude-like", vec![pattern], false);
        rule.guards.not_running = vec!["claude".into()];

        let probe = Arc::new(FakeProcessProbe {
            running: HashSet::from(["claude".into()]),
            unknown: HashSet::new(),
        });
        let (tx, rx) = unbounded();
        let orch = Orchestrator::with_process_probe(
            crate::cancel::CancelToken::new(),
            Some(tx),
            probe,
        );
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert!(plan.entries.is_empty());
        let ev = rx.try_recv().unwrap();
        assert!(matches!(
            ev,
            StreamEvent::Skipped {
                reason: SkipReason::AppRunning,
                ..
            }
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_selects_when_process_idle() {
        let _guard = test_env::lock();
        let dir = scratch("not-running-idle");
        let file = dir.join("real/.claude/sessions/old");
        touch(&file);
        let pattern = file.to_string_lossy().into_owned();

        let mut rule = all_rule("claude-like-idle", vec![pattern], false);
        rule.guards.not_running = vec!["claude".into()];

        let probe = Arc::new(FakeProcessProbe::default());
        let orch = Orchestrator::with_process_probe(
            crate::cancel::CancelToken::new(),
            None,
            probe,
        );
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(plan.entries.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
