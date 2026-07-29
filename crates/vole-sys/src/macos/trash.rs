use std::io;
use std::path::Path;
use std::time::Duration;

use crate::traits::Trash;

pub struct MacTrash;

impl Trash for MacTrash {
    fn trash_path(&self, path: &Path, _timeout: Duration) -> Result<(), io::Error> {
        trash::delete(path).map_err(io::Error::other)?;
        Ok(())
    }
}
