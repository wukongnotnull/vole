use std::io;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::traits::Sqlite;

pub struct MacSqlite;

impl Sqlite for MacSqlite {
    fn query_count(&self, path: &Path, sql: &str, _timeout: Duration) -> Result<i64, io::Error> {
        let conn = Connection::open(path).map_err(io::Error::other)?;
        let count: i64 = conn
            .query_row(sql, [], |row| row.get(0))
            .map_err(io::Error::other)?;
        Ok(count)
    }
}
