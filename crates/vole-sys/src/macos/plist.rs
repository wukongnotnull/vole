use std::io;
use std::path::Path;
use std::time::Duration;

use crate::traits::Plist;

pub struct MacPlist;

impl Plist for MacPlist {
    fn read_file(&self, path: &Path, _timeout: Duration) -> Result<plist::Value, io::Error> {
        let data = std::fs::read(path)?;
        plist::from_bytes(&data).map_err(io::Error::other)
    }
}
