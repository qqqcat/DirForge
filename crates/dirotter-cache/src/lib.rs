use dirotter_core::{ErrorKind, NodeStore, ScanErrorRecord};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: i64 = 3;

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
    db_path: PathBuf,
    conn: Connection,
}

impl CacheStore {
    pub fn new(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        let conn = Connection::open(&db_path)?;
        let store = Self { db_path, conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS schema_meta (
              id INTEGER PRIMARY KEY CHECK(id = 1),
              version INTEGER NOT NULL
            );
            INSERT INTO schema_meta(id, version)
            VALUES(1, 1)
            ON CONFLICT(id) DO NOTHING;
            ",
        )?;

        let v: i64 =
            self.conn
                .query_row("SELECT version FROM schema_meta WHERE id = 1", [], |r| {
                    r.get(0)
                })?;

        if v < 2 {
            self.conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS snapshots (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  root TEXT NOT NULL,
                  created_at INTEGER NOT NULL,
                  payload_json TEXT,
                  payload_blob BLOB,
                  payload_encoding TEXT NOT NULL DEFAULT 'json',
                  node_count INTEGER NOT NULL DEFAULT 0,
                  payload_size INTEGER NOT NULL DEFAULT 0,
                  schema_version INTEGER NOT NULL DEFAULT 1
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
                  kind TEXT NOT NULL DEFAULT 'system',
                  FOREIGN KEY(history_id) REFERENCES scan_history(id)
                );
                CREATE TABLE IF NOT EXISTS settings (
                  key TEXT PRIMARY KEY,
                  value TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS operation_audit (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  kind TEXT NOT NULL,
                  payload TEXT NOT NULL,
                  created_at INTEGER NOT NULL
                );
                UPDATE schema_meta SET version = 2 WHERE id = 1;
                ",
            )?;
        }

        self.conn.execute(
            "UPDATE schema_meta SET version = ? WHERE id = 1",
            params![SCHEMA_VERSION],
        )?;

        // backfill migration for old databases missing `kind` column
        let mut has_kind = false;
        {
            let mut stmt = self.conn.prepare("PRAGMA table_info(scan_errors)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            for col in rows {
                if col? == "kind" {
                    has_kind = true;
                }
            }
        }
        if !has_kind {
            self.conn.execute(
                "ALTER TABLE scan_errors ADD COLUMN kind TEXT NOT NULL DEFAULT 'system'",
                [],
            )?;
        }

        self.conn
            .execute("ALTER TABLE snapshots ADD COLUMN payload_blob BLOB", [])
            .ok();
        self.conn
            .execute(
                "ALTER TABLE snapshots ADD COLUMN payload_encoding TEXT NOT NULL DEFAULT 'json'",
                [],
            )
            .ok();
        self.conn
            .execute(
                "ALTER TABLE snapshots ADD COLUMN node_count INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .ok();
        self.conn
            .execute(
                "ALTER TABLE snapshots ADD COLUMN payload_size INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .ok();
        self.conn
            .execute(
                "ALTER TABLE snapshots ADD COLUMN schema_version INTEGER NOT NULL DEFAULT 1",
                [],
            )
            .ok();

        Ok(())
    }

    pub fn schema_version(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT version FROM schema_meta WHERE id = 1", [], |r| {
                r.get(0)
            })
    }

    pub fn save_snapshot(&self, root: &str, store: &NodeStore) -> rusqlite::Result<()> {
        let encoded = bincode::serialize(store)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let compressed = zstd::stream::encode_all(encoded.as_slice(), 3)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let payload_size = compressed.len() as i64;
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let write_result = (|| {
            self.conn
                .execute("DELETE FROM snapshots WHERE root = ?", params![root])?;
            self.conn.execute(
                "INSERT INTO snapshots(root, created_at, payload_json, payload_blob, payload_encoding, node_count, payload_size, schema_version) VALUES(?, ?, NULL, ?, 'zstd+bincode', ?, ?, ?)",
                params![root, now_ts(), compressed, store.nodes.len() as i64, payload_size, SCHEMA_VERSION],
            )?;
            Ok(())
        })();

        match write_result {
            Ok(()) => {
                self.conn.execute_batch("COMMIT; PRAGMA optimize;")?;
                Ok(())
            }
            Err(err) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(err)
            }
        }
    }

    pub fn load_latest_snapshot(&self, root: &str) -> rusqlite::Result<Option<NodeStore>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload_blob, payload_json, payload_encoding FROM snapshots WHERE root = ? ORDER BY created_at DESC, id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![root])?;
        if let Some(row) = rows.next()? {
            let blob: Option<Vec<u8>> = row.get(0)?;
            let payload_json: Option<String> = row.get(1)?;
            let encoding: Option<String> = row.get(2)?;

            if let (Some(bytes), Some(enc)) = (blob, encoding) {
                if enc == "zstd+bincode" {
                    let decompressed = zstd::stream::decode_all(bytes.as_slice()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(e),
                        )
                    })?;
                    let store: NodeStore = bincode::deserialize(&decompressed).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(e),
                        )
                    })?;
                    return Ok(Some(store));
                }
            }

            if let Some(payload) = payload_json {
                let store: NodeStore = serde_json::from_str(&payload).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                return Ok(Some(store));
            }

            Ok(None)
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
                "INSERT INTO scan_errors(history_id, path, reason, kind) VALUES(?, ?, ?, ?)",
                params![history_id, err.path, err.reason, kind_to_str(err.kind)],
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
        let mut stmt = self.conn.prepare(
            "SELECT path, reason, kind FROM scan_errors WHERE history_id = ? ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![history_id], |r| {
            Ok(ScanErrorRecord {
                path: r.get(0)?,
                reason: r.get(1)?,
                kind: str_to_kind(&r.get::<_, String>(2)?),
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

    pub fn add_audit_event(&self, kind: &str, payload: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO operation_audit(kind, payload, created_at) VALUES(?, ?, ?)",
            params![kind, payload, now_ts()],
        )?;
        Ok(())
    }

    pub fn export_diagnostics_json(&self) -> rusqlite::Result<String> {
        let schema = self.schema_version()?;
        let history_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM scan_history", [], |r| r.get(0))?;
        let error_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM scan_errors", [], |r| r.get(0))?;
        let settings_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))?;

        Ok(format!(
            r#"{{"schema_version":{},"history_count":{},"error_count":{},"settings_count":{}}}"#,
            schema, history_count, error_count, settings_count
        ))
    }
}

fn kind_to_str(k: ErrorKind) -> &'static str {
    match k {
        ErrorKind::User => "user",
        ErrorKind::Transient => "transient",
        ErrorKind::System => "system",
    }
}

fn str_to_kind(s: &str) -> ErrorKind {
    match s {
        "user" => ErrorKind::User,
        "transient" => ErrorKind::Transient,
        _ => ErrorKind::System,
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use dirotter_core::NodeKind;

    #[test]
    fn schema_migrates_and_writes() {
        let path =
            std::env::temp_dir().join(format!("dirotter_cache_test_{}.db", std::process::id()));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let cache = CacheStore::new(&path).expect("cache");
        assert!(cache.schema_version().expect("schema") >= 2);

        cache.set_setting("k", "v").expect("set");
        assert_eq!(cache.get_setting("k").expect("get"), Some("v".to_string()));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_snapshot_replaces_previous_snapshot_for_same_root() {
        let path = std::env::temp_dir().join(format!(
            "dirotter_cache_snapshot_test_{}.db",
            std::process::id()
        ));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let cache = CacheStore::new(&path).expect("cache");
        let mut store = NodeStore::default();
        let root = store.add_node(
            None,
            "root".to_string(),
            "C:\\".to_string(),
            NodeKind::Dir,
            0,
        );
        store.nodes[root.0].size_subtree = 1;
        store.nodes[root.0].file_count = 1;
        store.nodes[root.0].dir_count = 1;

        cache.save_snapshot("C:\\", &store).expect("first snapshot");
        store.nodes[0].size_subtree = 2;
        cache
            .save_snapshot("C:\\", &store)
            .expect("second snapshot");

        let snapshot_count: i64 = cache
            .conn
            .query_row(
                "SELECT COUNT(*) FROM snapshots WHERE root = ?",
                params!["C:\\"],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(snapshot_count, 1);

        let latest = cache
            .load_latest_snapshot("C:\\")
            .expect("load")
            .expect("snapshot");
        assert_eq!(latest.nodes[0].size_subtree, 2);

        let _ = std::fs::remove_file(path);
    }
}
