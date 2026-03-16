use dirforge_core::{
    NodeKind, NodeStore, ScanErrorRecord, ScanProfile, ScanSummary, SnapshotDelta,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanStage {
    Planning,
    Enumerating,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub stage: ScanStage,
    pub current_path: Option<String>,
    pub summary: ScanSummary,
    pub queue_depth: usize,
}

#[derive(Debug, Clone)]
pub struct BatchEntry {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum ScanEvent {
    Progress(ScanProgress),
    Batch(Vec<BatchEntry>),
    Snapshot {
        store: NodeStore,
        delta: SnapshotDelta,
    },
    Finished {
        store: NodeStore,
        summary: ScanSummary,
        errors: Vec<ScanErrorRecord>,
    },
}

pub struct ScanHandle {
    pub events: Receiver<ScanEvent>,
    cancel: Arc<AtomicBool>,
}

impl ScanHandle {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScanConfig {
    pub profile: ScanProfile,
    pub batch_size: usize,
    pub snapshot_ms: u64,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            profile: ScanProfile::Ssd,
            batch_size: 256,
            snapshot_ms: 75,
        }
    }
}

pub fn start_scan(root: PathBuf, config: ScanConfig) -> ScanHandle {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);

    std::thread::spawn(move || {
        let mut store = NodeStore::default();
        let mut summary = ScanSummary::default();
        let mut errors = Vec::new();
        let mut batch = Vec::with_capacity(config.batch_size.max(1));
        let mut frontier: VecDeque<String> = VecDeque::new();

        let _ = tx.send(ScanEvent::Progress(ScanProgress {
            stage: ScanStage::Planning,
            current_path: Some(root.display().to_string()),
            summary: summary.clone(),
            queue_depth: 0,
        }));

        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();
        store.add_node(
            None,
            root_name,
            root.display().to_string(),
            NodeKind::Dir,
            0,
        );

        let mut last_snapshot = Instant::now();
        let mut changed_since_snapshot = 0usize;

        for entry in WalkDir::new(&root).follow_links(false).into_iter() {
            if cancel_clone.load(Ordering::SeqCst) {
                break;
            }

            let entry = match entry {
                Ok(v) => v,
                Err(e) => {
                    summary.error_count += 1;
                    errors.push(ScanErrorRecord {
                        path: root.display().to_string(),
                        reason: format!("walkdir: {e}"),
                    });
                    continue;
                }
            };

            let path = entry.path().display().to_string();
            if path == root.display().to_string() {
                continue;
            }

            frontier.push_back(path.clone());

            let meta = match entry.metadata() {
                Ok(v) => v,
                Err(e) => {
                    summary.error_count += 1;
                    errors.push(ScanErrorRecord {
                        path: path.clone(),
                        reason: format!("metadata: {e}"),
                    });
                    continue;
                }
            };

            let parent_path = entry
                .path()
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| root.display().to_string());
            let parent = store.path_index.get(&parent_path).copied();

            if meta.is_dir() {
                summary.scanned_dirs += 1;
                store.add_node(
                    parent,
                    entry.file_name().to_string_lossy().to_string(),
                    path.clone(),
                    NodeKind::Dir,
                    0,
                );
                batch.push(BatchEntry {
                    path,
                    is_dir: true,
                    size: 0,
                });
            } else {
                summary.scanned_files += 1;
                summary.bytes_observed += meta.len();
                store.add_node(
                    parent,
                    entry.file_name().to_string_lossy().to_string(),
                    path.clone(),
                    NodeKind::File,
                    meta.len(),
                );
                batch.push(BatchEntry {
                    path,
                    is_dir: false,
                    size: meta.len(),
                });
            }
            changed_since_snapshot += 1;

            while frontier.len() > 32 {
                let _ = frontier.pop_front();
            }

            if batch.len() >= config.batch_size.max(1) {
                let _ = tx.send(ScanEvent::Batch(std::mem::take(&mut batch)));
            }

            let progress = ScanProgress {
                stage: ScanStage::Enumerating,
                current_path: frontier.back().cloned(),
                summary: summary.clone(),
                queue_depth: frontier.len(),
            };
            let _ = tx.send(ScanEvent::Progress(progress));

            if last_snapshot.elapsed() >= Duration::from_millis(config.snapshot_ms.max(50)) {
                store.rollup();
                let _ = tx.send(ScanEvent::Snapshot {
                    store: store.clone(),
                    delta: SnapshotDelta {
                        changed_nodes: changed_since_snapshot,
                        scanned_files: summary.scanned_files,
                        scanned_dirs: summary.scanned_dirs,
                    },
                });
                changed_since_snapshot = 0;
                last_snapshot = Instant::now();
            }

            // profile throttling
            match config.profile {
                ScanProfile::Ssd => {}
                ScanProfile::Hdd => std::thread::sleep(Duration::from_millis(1)),
                ScanProfile::Network => std::thread::sleep(Duration::from_millis(2)),
            }
        }

        if !batch.is_empty() {
            let _ = tx.send(ScanEvent::Batch(batch));
        }
        store.rollup();
        let _ = tx.send(ScanEvent::Snapshot {
            store: store.clone(),
            delta: SnapshotDelta {
                changed_nodes: changed_since_snapshot,
                scanned_files: summary.scanned_files,
                scanned_dirs: summary.scanned_dirs,
            },
        });
        let _ = tx.send(ScanEvent::Finished {
            store,
            summary,
            errors,
        });
    });

    ScanHandle { events: rx, cancel }
}
