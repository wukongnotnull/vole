//! 平台抽象与 macOS 后端。这是 workspace 内唯一允许出现 `unsafe` 的 crate。

#[cfg(not(target_os = "macos"))]
compile_error!(
    "Vole 目前只支持 macOS。平台边界已是 trait，加其他平台请实现对应后端而非放宽此断言。"
);

/// 重导出协议类型，让上层 crate 无需直接依赖 vole-proto 即可使用。
pub use vole_proto;
