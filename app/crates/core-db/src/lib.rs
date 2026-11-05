//! SQLCipher vault database (blueprint 04 §4, 02 §7.3).
//!
//! * Opened with a raw 256-bit key derived from the VMK (`PRAGMA key = "x'…'"`), so SQLCipher
//!   skips its own PBKDF2; a wrong key fails fast with "file is not a database".
//! * All SQL uses bound parameters; user text never reaches the SQL string.

mod filter;
pub mod profiles;
pub mod proxies;
pub mod org;
pub mod misc;

use core_model::{AppError, CmdResult};
use core_vault::Key32;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/0001_init.sql")),
    (2, include_str!("../migrations/0002_vanta.sql")),
];

pub struct Db { conn: Mutex<Connection> }

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn new_id() -> String { uuid::Uuid::now_v7().to_string() }

pub(crate) fn sql_err(e: rusqlite::Error) -> AppError {
    match &e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("row".into()),
        rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => AppError::Conflict("constraint".into()),
        _ => { eprintln!("[vanta] sql: {e}"); AppError::Internal }
    }
}

impl Db {
    pub fn open(path: &Path, key: &Key32) -> CmdResult<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .map_err(sql_err)?;
        let hex_key = zeroize::Zeroizing::new(hex::encode(key.as_bytes()));
        conn.pragma_update(None, "key", format!("x'{}'", hex_key.as_str())).map_err(sql_err)?;
        // Fails with "file is not a database" on a wrong key.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(())).map_err(|_| AppError::WrongPassword)?;
        conn.pragma_update(None, "cipher_memory_security", "ON").ok();
        conn.pragma_update(None, "journal_mode", "WAL").map_err(sql_err)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(sql_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL").map_err(sql_err)?;
        conn.pragma_update(None, "secure_delete", "ON").map_err(sql_err)?;
        conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(sql_err)?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> { self.conn.lock().unwrap_or_else(|p| p.into_inner()) }

    pub fn schema_version(&self) -> CmdResult<u32> {
        let c = self.conn();
        let exists: bool = c.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='schema_meta'", [], |r| r.get::<_, i64>(0)).map_err(sql_err)? > 0;
        if !exists { return Ok(0); }
        let v: String = c.query_row("SELECT value FROM schema_meta WHERE key='schema_version'", [], |r| r.get(0)).map_err(sql_err)?;
        v.parse().map_err(|_| AppError::Internal)
    }

    /// Applies pending migrations, each in its own transaction (idempotent).
    fn migrate(&self) -> CmdResult<()> {
        let current = self.schema_version()?;
        for (ver, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
            let mut c = self.conn();
            let tx = c.transaction().map_err(sql_err)?;
            tx.execute_batch(sql).map_err(sql_err)?;
            tx.execute("INSERT INTO schema_meta(key,value) VALUES('schema_version',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [ver.to_string()]).map_err(sql_err)?;
            tx.commit().map_err(sql_err)?;
        }
        Ok(())
    }

    /// Re-key the database (`PRAGMA rekey`) — used by "rotate master key".
    pub fn rekey(&self, key: &Key32) -> CmdResult<()> {
        let hex_key = zeroize::Zeroizing::new(hex::encode(key.as_bytes()));
        self.conn().pragma_update(None, "rekey", format!("x'{}'", hex_key.as_str())).map_err(sql_err)
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    pub fn db() -> (tempfile::TempDir, Db) {
        let d = tempfile::tempdir().unwrap();
        let db = Db::open(&d.path().join("vault.db"), &Key32::from_array([7; 32])).unwrap();
        (d, db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_and_migrated() {
        let (d, db) = testutil::db();
        assert_eq!(db.schema_version().unwrap(), 2);
        drop(db);
        // Wrong key → WrongPassword; right key reopens idempotently.
        assert!(matches!(Db::open(&d.path().join("vault.db"), &Key32::from_array([8; 32])), Err(AppError::WrongPassword)));
        let db = Db::open(&d.path().join("vault.db"), &Key32::from_array([7; 32])).unwrap();
        assert_eq!(db.schema_version().unwrap(), 2);
        drop(db);
        // The file must not look like plain SQLite.
        let head = std::fs::read(d.path().join("vault.db")).unwrap();
        assert_ne!(&head[..15], b"SQLite format 3");
    }
}
