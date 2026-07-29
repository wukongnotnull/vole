//! 删除管线（对齐 mole `file_ops.sh` 的 `safe_remove` / `mole_delete`）。

mod config;
mod deletion_log;
mod mole_delete;
mod safe_remove;
mod size;
mod trash;

pub use config::{
    delete_mode_from_env, deletion_log_path, dry_run_enabled, test_no_auth, test_trash_dir,
    DeleteMode, DeleteModeParseError,
};
pub use deletion_log::DeletionLogger;
pub use mole_delete::{
    mole_delete, mole_delete_with_env_mode, warn_invalid_delete_mode_once, MoleDeleteError,
    MoleDeleteOptions,
};
pub use safe_remove::{
    safe_remove, safe_remove_symlink, FsRemover, PathRemover, SafeRemoveError, SafeRemoveOptions,
    ShellRemover,
};
pub use size::{measure_path_size_kb, PathSizeKb};
pub use trash::{move_to_trash, TrashMoveError};
