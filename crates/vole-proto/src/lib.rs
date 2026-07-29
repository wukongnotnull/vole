//! 前端与 vole 之间的协议类型。
//!
//! 本 crate 是依赖图的叶子，不得依赖任何 workspace 内 crate，
//! 外部依赖也要压到最少——将来第三方前端只依赖它即可，不必背上整个 vole。
#![forbid(unsafe_code)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod analyze;
pub mod events;
pub mod plan;
pub mod report;
pub mod status;

pub use analyze::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};
pub use events::StreamEvent;
pub use plan::{Plan, PlanEntry};
pub use report::{Report, SkipSummary};
pub use status::StatusSnapshot;

/// NDJSON 协议版本。Phase 4 结束时冻结 v1，在那之前可自由破坏性修改。
pub const SCHEMA_VERSION: u32 = 1;

/// 一条规则未产出删除目标的原因。
///
/// 序列化字符串在 Phase 4 结束时随协议冻结，此后只能追加变体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    NeedsPrivilege,
    AppRunning,
    Whitelisted,
    DbLocked,
    PathVanished,
    TccDenied,
    Timeout,
}

/// 一个待删除的候选目标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub path: PathBuf,
    pub label: String,
}
