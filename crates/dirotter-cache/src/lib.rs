use dirotter_core::NodeStore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SETTINGS_SCHEMA_VERSION: u32 = 1;
use std::sync::atomic::{AtomicU64, Ordering};
const TRANSIENT_STORAGE_MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24);
static TEST_SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageErrorKind {
    Io,
    Encode,
    Decode,
}

#[derive(Debug, Clone)]
pub struct StorageError {
    pub kind: StorageErrorKind,
    pub message: String,
}

impl StorageError {
    fn new(kind: StorageErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StorageError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsDocument {
    schema_version: u32,
    values: BTreeMap<String, String>,
}

impl Default for SettingsDocument {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            values: BTreeMap::new(),
        }
    }
}

pub struct CacheStore {
    settings_path: PathBuf,
    session_root: PathBuf,
    ephemeral_settings: bool,
}

impl CacheStore {
    pub fn open_default() -> Result<Self, StorageError> {
        let _ = purge_stale_transient_storage_roots();
        let settings_dir = default_settings_dir();
        let session_root = default_session_root();
        Self::from_paths_internal(settings_dir.join("settings.json"), session_root, false)
    }

    pub fn open_ephemeral() -> Result<Self, StorageError> {
        let _ = purge_stale_transient_storage_roots();
        let root = std::env::temp_dir().join(format!(
            "dirotter-ephemeral-{}-{}",
            std::process::id(),
            now_unix_ms()
        ));
        Self::from_paths_internal(root.join("settings.json"), root.join("session"), true)
    }

    pub fn for_tests() -> Result<Self, StorageError> {
        let nonce = TEST_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "dirotter-test-storage-{}-{}",
            std::process::id(),
            nonce
        ));
        Self::from_paths_internal(root.join("settings.json"), root.join("session"), true)
    }

    pub fn from_paths(
        settings_path: impl AsRef<Path>,
        session_root: impl AsRef<Path>,
    ) -> Result<Self, StorageError> {
        Self::from_paths_internal(settings_path, session_root, false)
    }

    fn from_paths_internal(
        settings_path: impl AsRef<Path>,
        session_root: impl AsRef<Path>,
        ephemeral_settings: bool,
    ) -> Result<Self, StorageError> {
        let settings_path = settings_path.as_ref().to_path_buf();
        let session_root = session_root.as_ref().to_path_buf();

        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        fs::create_dir_all(&session_root).map_err(io_err)?;

        Ok(Self {
            settings_path,
            session_root,
            ephemeral_settings,
        })
    }

    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    pub fn session_root(&self) -> &Path {
        &self.session_root
    }

    pub fn uses_ephemeral_settings(&self) -> bool {
        self.ephemeral_settings
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let doc = self.read_settings_document()?;
        Ok(doc.values.get(key).cloned())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let mut doc = self.read_settings_document()?;
        doc.values.insert(key.to_string(), value.to_string());
        self.write_settings_document(&doc)
    }

    pub fn save_snapshot(&self, root: &str, store: &NodeStore) -> Result<(), StorageError> {
        let encoded = bincode::serialize(store)
            .map_err(|err| StorageError::new(StorageErrorKind::Encode, err.to_string()))?;
        let compressed = zstd::stream::encode_all(encoded.as_slice(), 3)
            .map_err(|err| StorageError::new(StorageErrorKind::Encode, err.to_string()))?;
        self.write_file_atomically(&snapshot_path_for(&self.session_root, root), &compressed)
    }

    pub fn load_latest_snapshot(&self, root: &str) -> Result<Option<NodeStore>, StorageError> {
        Self::load_snapshot_from_session_root(&self.session_root, root)
    }

    pub fn load_snapshot_from_session_root(
        session_root: impl AsRef<Path>,
        root: &str,
    ) -> Result<Option<NodeStore>, StorageError> {
        let path = snapshot_path_for(session_root.as_ref(), root);
        if !path.exists() {
            return Ok(None);
        }
        let compressed = fs::read(&path).map_err(io_err)?;
        let decoded = zstd::stream::decode_all(compressed.as_slice())
            .map_err(|err| StorageError::new(StorageErrorKind::Decode, err.to_string()))?;
        let store = bincode::deserialize(&decoded)
            .map_err(|err| StorageError::new(StorageErrorKind::Decode, err.to_string()))?;
        Ok(Some(store))
    }

    fn read_settings_document(&self) -> Result<SettingsDocument, StorageError> {
        if !self.settings_path.exists() {
            return Ok(SettingsDocument::default());
        }
        let bytes = fs::read(&self.settings_path).map_err(io_err)?;
        serde_json::from_slice(&bytes)
            .map_err(|err| StorageError::new(StorageErrorKind::Decode, err.to_string()))
    }

    fn write_settings_document(&self, doc: &SettingsDocument) -> Result<(), StorageError> {
        let payload = serde_json::to_vec_pretty(doc)
            .map_err(|err| StorageError::new(StorageErrorKind::Encode, err.to_string()))?;
        self.write_file_atomically(&self.settings_path, &payload)
    }

    fn write_file_atomically(&self, path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let temp_path = path.with_extension(format!(
            "{}tmp",
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!("{ext}."))
                .unwrap_or_default()
        ));
        fs::write(&temp_path, bytes).map_err(io_err)?;
        fs::rename(&temp_path, path).or_else(|rename_err| {
            if path.exists() {
                fs::remove_file(path).map_err(io_err)?;
                fs::rename(&temp_path, path).map_err(io_err)
            } else {
                Err(io_err(rename_err))
            }
        })
    }
}

impl Drop for CacheStore {
    fn drop(&mut self) {
        if is_transient_storage_root(&self.session_root) {
            let _ = fs::remove_dir_all(&self.session_root);
            if self.ephemeral_settings {
                if let Some(parent) = self.settings_path.parent() {
                    let _ = fs::remove_dir_all(parent);
                }
            }
        }
    }
}

fn io_err(err: std::io::Error) -> StorageError {
    StorageError::new(StorageErrorKind::Io, err.to_string())
}

fn snapshot_path_for(session_root: &Path, root: &str) -> PathBuf {
    let digest = blake3::hash(root.as_bytes()).to_hex().to_string();
    session_root.join(format!("{digest}.snapshot"))
}

fn default_settings_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA"))
        {
            return PathBuf::from(base).join("DirOtter");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("DirOtter");
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(base) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(base).join("dirotter");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config").join("dirotter");
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dirotter-data")
}

fn default_session_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dirotter-session-{}-{}",
        std::process::id(),
        now_unix_ms()
    ))
}

fn purge_stale_transient_storage_roots() -> Result<(), StorageError> {
    let temp_dir = std::env::temp_dir();
    let now = SystemTime::now();
    let entries = fs::read_dir(&temp_dir).map_err(io_err)?;

    for entry in entries {
        let entry = entry.map_err(io_err)?;
        let path = entry.path();
        if !path.is_dir() || !is_transient_storage_root(&path) {
            continue;
        }

        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .unwrap_or(UNIX_EPOCH);
        let age = now.duration_since(modified).unwrap_or_default();
        if age >= TRANSIENT_STORAGE_MAX_AGE {
            let _ = fs::remove_dir_all(path);
        }
    }

    Ok(())
}

fn is_transient_storage_root(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.starts_with("dirotter-session-")
        || name.starts_with("dirotter-ephemeral-")
        || name.starts_with("dirotter-test-storage-")
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dirotter_core::NodeKind;

    #[test]
    fn settings_round_trip() {
        let store = CacheStore::for_tests().expect("store");

        store.set_setting("language", "en").expect("set language");
        store.set_setting("theme", "dark").expect("set theme");

        assert_eq!(
            store.get_setting("language").expect("get language"),
            Some("en".to_string())
        );
        assert_eq!(
            store.get_setting("theme").expect("get theme"),
            Some("dark".to_string())
        );
    }

    #[test]
    fn snapshot_round_trip() {
        let store = CacheStore::for_tests().expect("store");
        let mut tree = NodeStore::default();
        let root = tree.add_node(
            None,
            "root".to_string(),
            "C:\\".to_string(),
            NodeKind::Dir,
            0,
        );
        tree.nodes[root.0].size_subtree = 2;
        tree.nodes[root.0].file_count = 1;
        tree.nodes[root.0].dir_count = 1;

        store.save_snapshot("C:\\", &tree).expect("save snapshot");
        let loaded = store
            .load_latest_snapshot("C:\\")
            .expect("load snapshot")
            .expect("snapshot");

        assert_eq!(loaded.nodes[0].size_subtree, 2);
        assert_eq!(loaded.nodes.len(), 1);
    }
}
