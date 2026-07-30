//! 路径校验、保护判定、文件操作、操作日志、配置与单位格式化。
//!
//! `rules` / `scan` / `ops` 目前是本 crate 的 module，达到拆分阈值再独立成 crate。
#![forbid(unsafe_code)]

pub use vole_sys::vole_proto;

#[cfg(test)]
mod clean_fixture;

pub mod analyze;
pub mod cancel;
pub mod delete;
pub mod history;
pub mod mutex;
pub mod oplog;
pub mod ops;
pub mod optimize;
pub mod protection;
pub mod rules;
pub mod safety;
pub mod scan;
pub mod status;
pub mod units;
pub mod whitelist;

#[cfg(test)]
mod test_env;
