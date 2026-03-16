use dirforge_core::{NodeStore, ScanErrorRecord};
use rusqlite::{params, Connection};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct HistoryRecord {
    pub id: i64,
    pub root: String,
    pub scanned_files: u64,
    pub scanned_dirs: u64,
    pub bytes_observed: u64,
    pub error_count: u64,
    pub created_at: i64,
}

pub struct CacheStore {
    conn: Connection,
}

impl CacheStore {
    pub fn new(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS snapshots (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              root TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              payload_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scan_history (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              root TEXT NOT NULL,
              scanned_files INTEGER NOT NULL,
              scanned_dirs INTEGER NOT NULL,
              bytes_observed INTEGER NOT NULL,
              error_count INTEGER NOT NULL,
              created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scan_errors (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              history_id INTEGER NOT NULL,
              path TEXT NOT NULL,
              reason TEXT NOT NULL,
              FOREIGN KEY(history_id) REFERENCES scan_history(id)
            );
            CREATE TABLE IF NOT EXISTS settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );
            ",
        )
    }

    pub fn save_snapshot(&self, root: &str, store: &NodeStore) -> rusqlite::Result<()> {
        let payload = serde_json::to_string(store)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.conn.execute(
            "INSERT INTO snapshots(root, created_at, payload_json) VALUES(?, ?, ?)",
            params![root, now_ts(), payload],
        )?;
        Ok(())
    }

    pub fn load_latest_snapshot(&self, root: &str) -> rusqlite::Result<Option<NodeStore>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload_json FROM snapshots WHERE root = ? ORDER BY created_at DESC, id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![root])?;
        if let Some(row) = rows.next()? {
            let payload: String = row.get(0)?;
            let store: NodeStore = serde_json::from_str(&payload).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(Some(store))
        } else {
            Ok(None)
        }
    }

    pub fn record_scan_history(
        &self,
        root: &str,
        scanned_files: u64,
        scanned_dirs: u64,
        bytes_observed: u64,
        error_count: u64,
        errors: &[ScanErrorRecord],
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO scan_history(root, scanned_files, scanned_dirs, bytes_observed, error_count, created_at)
             VALUES(?, ?, ?, ?, ?, ?)",
            params![root, scanned_files, scanned_dirs, bytes_observed, error_count, now_ts()],
        )?;
        let history_id = self.conn.last_insert_rowid();
        for err in errors {
            self.conn.execute(
                "INSERT INTO scan_errors(history_id, path, reason) VALUES(?, ?, ?)",
                params![history_id, err.path, err.reason],
            )?;
        }
        Ok(history_id)
    }

    pub fn list_history(&self, limit: i64) -> rusqlite::Result<Vec<HistoryRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, root, scanned_files, scanned_dirs, bytes_observed, error_count, created_at
             FROM scan_history ORDER BY created_at DESC, id DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(HistoryRecord {
                id: r.get(0)?,
                root: r.get(1)?,
                scanned_files: r.get(2)?,
                scanned_dirs: r.get(3)?,
                bytes_observed: r.get(4)?,
                error_count: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn list_errors_by_history(
        &self,
        history_id: i64,
    ) -> rusqlite::Result<Vec<ScanErrorRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, reason FROM scan_errors WHERE history_id = ? ORDER BY id ASC")?;
        let rows = stmt.query_map(params![history_id], |r| {
            Ok(ScanErrorRecord {
                path: r.get(0)?,
                reason: r.get(1)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(r) = rows.next()? {
            Ok(Some(r.get(0)?))
        } else {
            Ok(None)
        }
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
