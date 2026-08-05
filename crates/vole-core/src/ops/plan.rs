//! plan 生成管线：规则 → glob → 策略 → 安全闸口 → 身份快照。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::protection::AppProtection;
use crate::rules::{
    collect_path_candidates, resolve_strategy, select_custom, should_skip_for_guards,
    CustomDegrade, PathEntry, ResolvedStrategy, Rule, Strategy,
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
    /// 规则级降级提示（人读 / 当次 coverage；不进 proto）。
    pub notices: Vec<PlanNotice>,
}

/// plan 期间的规则级 notice（不进协议）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanNotice {
    OrphanLibraryInaccessible,
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
        let mut seen_paths: HashSet<PathBuf> = HashSet::new();
        let mut scanned: u64 = 0;
        let mut next_id: u64 = 0;
        let mut notices: Vec<PlanNotice> = Vec::new();

        for rule in rules {
            self.check_cancel()?;

            if rule.disabled {
                continue;
            }

            if should_skip_for_guards(self.process_probe.as_ref(), &rule.guards) {
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
                    let result = select_custom(
                        &custom.handler,
                        &path_entries,
                        &home,
                        rule,
                        self.orphan_deps.as_ref(),
                    );
                    if let Some(CustomDegrade::LibraryInaccessible) = result.degrade {
                        // 语义外延：此处 TccDenied 表示规则级「Library/安装扫描不可访问」
                        //（含 FDA），不仅限于 EndpointSecurityCache 路径校验失败。
                        self.emit(StreamEvent::Skipped {
                            rule_id: rule.id.clone(),
                            reason: SkipReason::TccDenied,
                        });
                        if !notices.contains(&PlanNotice::OrphanLibraryInaccessible) {
                            notices.push(PlanNotice::OrphanLibraryInaccessible);
                        }
                    }
                    result.paths
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

                // First matching rule wins when multiple rules select the same path
                // (e.g. named cache + broad `~/Library/Caches/*`).
                if seen_paths.contains(&path) {
                    continue;
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

                let label = if rule.id == crate::orphan::ORPHANED_RULE_ID {
                    crate::orphan::orphan_label(&path)
                } else {
                    rule.label.clone()
                };

                self.emit(StreamEvent::Candidate {
                    id: id.clone(),
                    path: path_str,
                    label: label.clone(),
                    size,
                    rule_id: rule.id.clone(),
                });

                seen_paths.insert(path.clone());
                entries.push(PlanEntry {
                    id,
                    path,
                    label,
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
            notices,
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
    fn plan_selects_codex_desktop_stale_staging_by_age() {
        let _guard = test_env::lock();
        let home = scratch("codex-staging");
        let staging =
            home.join("Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation");
        let old = staging.join("old-build");
        let fresh = staging.join("fresh-build");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&fresh).unwrap();
        let old_mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000); // ~2023
        let fresh_mtime = SystemTime::now();
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(old_mtime)).unwrap();
        filetime::set_file_mtime(&fresh, filetime::FileTime::from_system_time(fresh_mtime))
            .unwrap();
        std::env::set_var("VOLE_TEST_HOME", &home);

        let rule = Rule {
            id: "codex-desktop-stale-update-staging".into(),
            category: Some("developer".into()),
            label: "Codex Desktop stale update staging".into(),
            platform: vec![],
            paths: vec![
                "~/Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation/*"
                    .into(),
            ],
            impact: None,
            disabled: false,
            last_verified: None,
            strategy: StrategyConfig {
                kind: crate::rules::StrategyKind::OlderThanDays,
                keep: None,
                env_override: None,
                days: Some(30),
                names: None,
                handler: None,
            },
            guards: Default::default(),
        };

        let orch = Orchestrator::with_process_probe(
            crate::cancel::CancelToken::new(),
            None,
            Arc::new(FakeProcessProbe::default()),
        );
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(
            plan.entries.len(),
            1,
            "entries: {:?}",
            plan.entries
                .iter()
                .map(|e| e.path.display().to_string())
                .collect::<Vec<_>>()
        );
        assert!(plan.entries[0].path.ends_with("old-build"));
        std::env::remove_var("VOLE_TEST_HOME");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn plan_dedupes_same_path_keeping_first_rule() {
        let _guard = test_env::lock();
        let dir = scratch("dedupe");
        let file = dir.join("Library/Caches/com.example.app");
        touch(&file);
        let pattern = file.to_string_lossy().into_owned();

        let mut specific = all_rule("specific-cache", vec![pattern.clone()], false);
        specific.label = "Specific cache".into();
        let mut broad = all_rule("user-app-cache", vec![pattern], false);
        broad.label = "User app cache".into();

        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), None);
        let plan = orch
            .build_plan(&[specific, broad], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].rule_id, "specific-cache");
        assert_eq!(plan.entries[0].label, "Specific cache");
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
            ..Default::default()
        });
        let (tx, rx) = unbounded();
        let orch =
            Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), Some(tx), probe);
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
        let orch = Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), None, probe);
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(plan.entries.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    fn jianying_generated_rule(cache_root: &Path) -> Rule {
        Rule {
            id: "jianyingpro-generated-cache".into(),
            category: Some("app-caches".into()),
            label: "JianyingPro generated cache".into(),
            platform: vec![],
            paths: vec![cache_root.to_string_lossy().into_owned()],
            impact: None,
            disabled: false,
            last_verified: None,
            strategy: StrategyConfig {
                kind: crate::rules::StrategyKind::Custom,
                keep: None,
                env_override: None,
                days: None,
                names: None,
                handler: Some("jianyingpro_generated_caches".into()),
            },
            guards: crate::rules::GuardsConfig {
                not_running: vec!["VideoFusion-macOS".into()],
                not_running_cmdline: vec![
                    "/VideoFusion-macOS.app/Contents/MacOS/VideoFusion-macOS".into(),
                ],
                ..Default::default()
            },
        }
    }

    #[test]
    fn plan_skips_jianying_when_editor_running() {
        let _guard = test_env::lock();
        let home = scratch("jianying-running");
        let cache = home.join("Movies/JianyingPro/User Data/Cache");
        fs::create_dir_all(cache.join("recognize")).unwrap();
        std::env::set_var("VOLE_TEST_HOME", &home);

        let rule = jianying_generated_rule(&cache);
        let probe = Arc::new(FakeProcessProbe {
            running: HashSet::from(["VideoFusion-macOS".into()]),
            ..Default::default()
        });
        let (tx, rx) = unbounded();
        let orch =
            Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), Some(tx), probe);
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
        std::env::remove_var("VOLE_TEST_HOME");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn plan_skips_jianying_when_cmdline_running() {
        let _guard = test_env::lock();
        let home = scratch("jianying-cmdline");
        let cache = home.join("Movies/JianyingPro/User Data/Cache");
        fs::create_dir_all(cache.join("recognize")).unwrap();
        std::env::set_var("VOLE_TEST_HOME", &home);

        let rule = jianying_generated_rule(&cache);
        let probe = Arc::new(FakeProcessProbe {
            cmdline_running: HashSet::from([
                "/VideoFusion-macOS.app/Contents/MacOS/VideoFusion-macOS".into(),
            ]),
            ..Default::default()
        });
        let (tx, rx) = unbounded();
        let orch =
            Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), Some(tx), probe);
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert!(plan.entries.is_empty());
        assert!(matches!(
            rx.try_recv().unwrap(),
            StreamEvent::Skipped {
                reason: SkipReason::AppRunning,
                ..
            }
        ));
        std::env::remove_var("VOLE_TEST_HOME");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn plan_selects_jianying_regenerable_when_idle() {
        let _guard = test_env::lock();
        let home = scratch("jianying-idle");
        let cache = home.join("Movies/JianyingPro/User Data/Cache");
        fs::create_dir_all(cache.join("recognize")).unwrap();
        fs::create_dir_all(cache.join("effect")).unwrap();
        std::env::set_var("VOLE_TEST_HOME", &home);

        let rule = jianying_generated_rule(&cache);
        let probe = Arc::new(FakeProcessProbe::default());
        let orch = Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), None, probe);
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(plan.entries.len(), 1);
        assert!(plan.entries[0].path.ends_with("recognize"));
        assert!(!plan
            .entries
            .iter()
            .any(|e| e.path.to_string_lossy().contains("effect")));
        std::env::remove_var("VOLE_TEST_HOME");
        let _ = fs::remove_dir_all(&home);
    }

    fn xctest_devices_rule(pattern: String) -> Rule {
        let mut rule = all_rule("xcode-xctest-devices", vec![pattern], false);
        rule.label = "Xcode XCTestDevices test data".into();
        rule.guards.not_running = vec![
            "Xcode".into(),
            "Simulator".into(),
            "CoreSimulatorService".into(),
            "simdiskimaged".into(),
            "xcodebuild".into(),
            "xctest".into(),
            "XCTRunner".into(),
        ];
        rule.guards.not_running_cmdline = vec![
            "com.apple.CoreSimulator".into(),
            "com.apple.dt.XCTest".into(),
            "XCTest".into(),
        ];
        rule
    }

    #[test]
    fn plan_skips_xctest_devices_when_xcode_running() {
        let _guard = test_env::lock();
        let dir = scratch("xctest-running");
        let device = dir.join("XCTestDevices/device-1");
        fs::create_dir_all(&device).unwrap();
        let pattern = format!("{}/*", dir.join("XCTestDevices").display());

        let rule = xctest_devices_rule(pattern);
        let probe = Arc::new(FakeProcessProbe {
            running: HashSet::from(["Xcode".into()]),
            ..Default::default()
        });
        let (tx, rx) = unbounded();
        let orch =
            Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), Some(tx), probe);
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert!(plan.entries.is_empty());
        assert!(matches!(
            rx.try_recv().unwrap(),
            StreamEvent::Skipped {
                reason: SkipReason::AppRunning,
                ..
            }
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_skips_xctest_devices_when_cmdline_xctest_hits() {
        let _guard = test_env::lock();
        let dir = scratch("xctest-cmdline");
        let device = dir.join("XCTestDevices/device-1");
        fs::create_dir_all(&device).unwrap();
        let pattern = format!("{}/*", dir.join("XCTestDevices").display());

        let rule = xctest_devices_rule(pattern);
        let probe = Arc::new(FakeProcessProbe {
            cmdline_running: HashSet::from(["com.apple.dt.XCTest".into()]),
            ..Default::default()
        });
        let (tx, rx) = unbounded();
        let orch =
            Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), Some(tx), probe);
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert!(plan.entries.is_empty());
        assert!(matches!(
            rx.try_recv().unwrap(),
            StreamEvent::Skipped {
                reason: SkipReason::AppRunning,
                ..
            }
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_selects_xctest_devices_when_idle() {
        let _guard = test_env::lock();
        let dir = scratch("xctest-idle");
        let device = dir.join("XCTestDevices/device-1");
        fs::create_dir_all(&device).unwrap();
        let pattern = format!("{}/*", dir.join("XCTestDevices").display());

        let rule = xctest_devices_rule(pattern);
        let probe = Arc::new(FakeProcessProbe::default());
        let orch = Orchestrator::with_process_probe(crate::cancel::CancelToken::new(), None, probe);
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(plan.entries.len(), 1);
        assert!(plan.entries[0].path.ends_with("device-1"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn orphaned_rule(paths: Vec<String>) -> Rule {
        Rule {
            id: crate::orphan::ORPHANED_RULE_ID.into(),
            category: Some("orphaned".into()),
            label: "Orphaned app data".into(),
            platform: vec![],
            paths,
            impact: None,
            disabled: false,
            last_verified: None,
            strategy: StrategyConfig {
                kind: crate::rules::StrategyKind::Custom,
                keep: None,
                env_override: None,
                days: None,
                names: None,
                handler: Some("orphaned_app_data".into()),
            },
            guards: Default::default(),
        }
    }

    fn fake_orphan_deps_uninstalled(bundle: &str) -> Arc<dyn crate::orphan::OrphanDeps> {
        use crate::orphan::FakeOrphanDeps;
        use std::collections::HashMap;
        Arc::new(FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::from([(bundle.to_string(), Ok(false))]),
            scan_error: false,
            ..Default::default()
        })
    }

    #[test]
    fn plan_orphaned_selects_when_only_orphaned_matches() {
        let _guard = test_env::lock();
        let home = scratch("orphan-only");
        let cache = home.join("Library/Caches/com.gone.app");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("x"), b"data").unwrap();
        let old = SystemTime::now() - Duration::from_secs(40 * 86400);
        filetime::set_file_mtime(&cache, filetime::FileTime::from_system_time(old)).unwrap();
        std::env::set_var("HOME", &home);

        let rule = orphaned_rule(vec!["~/Library/Caches/*".into()]);
        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), None)
            .with_orphan_deps(fake_orphan_deps_uninstalled("com.gone.app"));
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].rule_id, "orphaned-app-data");
        assert!(plan.entries[0].label.contains("Orphaned Caches:"));
        assert!(plan.entries[0].path.ends_with("com.gone.app"));
        assert!(
            plan.notices.is_empty(),
            "successful orphan select must not set degrade notices"
        );
        std::env::remove_var("HOME");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn plan_orphaned_loses_dedup_to_named_rule() {
        let _guard = test_env::lock();
        let home = scratch("orphan-dedup");
        let cache = home.join("Library/Caches/com.example.app");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("x"), b"data").unwrap();
        let old = SystemTime::now() - Duration::from_secs(40 * 86400);
        filetime::set_file_mtime(&cache, filetime::FileTime::from_system_time(old)).unwrap();
        std::env::set_var("HOME", &home);

        let pattern = cache.to_string_lossy().into_owned();
        let mut specific = all_rule("specific-cache", vec![pattern], false);
        specific.label = "Specific cache".into();
        let orphaned = orphaned_rule(vec!["~/Library/Caches/*".into()]);

        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), None)
            .with_orphan_deps(fake_orphan_deps_uninstalled("com.example.app"));
        let plan = orch
            .build_plan(&[specific, orphaned], &AppProtection::new(), &[])
            .unwrap();

        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].rule_id, "specific-cache");
        assert_eq!(plan.entries[0].label, "Specific cache");
        assert!(plan.notices.is_empty());
        std::env::remove_var("HOME");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn plan_orphaned_emits_tcc_denied_and_notice_when_degraded() {
        let _guard = test_env::lock();
        let home = scratch("orphan-fda");
        // 无 Library/Caches → LibraryInaccessible
        std::env::set_var("HOME", &home);

        let (tx, rx) = unbounded();
        let orch = Orchestrator::new(crate::cancel::CancelToken::new(), Some(tx))
            .with_orphan_deps(fake_orphan_deps_uninstalled("com.gone.app"));
        let rule = orphaned_rule(vec!["~/Library/Caches/*".into()]);
        let plan = orch
            .build_plan(&[rule], &AppProtection::new(), &[])
            .unwrap();

        assert!(plan.entries.is_empty());
        assert!(plan
            .notices
            .contains(&PlanNotice::OrphanLibraryInaccessible));

        let mut saw_tcc = false;
        while let Ok(ev) = rx.try_recv() {
            if let StreamEvent::Skipped {
                rule_id,
                reason: SkipReason::TccDenied,
            } = ev
            {
                if rule_id == "orphaned-app-data" {
                    saw_tcc = true;
                }
            }
        }
        assert!(saw_tcc, "expected Skipped TccDenied for orphaned-app-data");
        std::env::remove_var("HOME");
        let _ = fs::remove_dir_all(&home);
    }
}
