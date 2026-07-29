use std::path::Path;
use std::time::Duration;

use crate::traits::{Fs, FsError};

pub struct MacFs;

impl Fs for MacFs {
    fn metadata_len(&self, path: &Path, _timeout: Duration) -> Result<u64, FsError> {
        Ok(std::fs::metadata(path)?.len())
    }
}
