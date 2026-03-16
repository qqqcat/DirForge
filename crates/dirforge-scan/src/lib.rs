use dirforge_core::{
    ErrorKind, NodeId, NodeKind, NodeStore, ScanErrorRecord, ScanProfile, ScanSummary,
    SnapshotDelta,
};
use dirforge_telemetry as telemetry;
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
        delta: SnapshotDelta,
        store: NodeStore,
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

fn classify_error(reason: &str) -> ErrorKind {
    let r = reason.to_lowercase();
    if r.contains("permission") || r.contains("access") || r.contains("denied") {
        ErrorKind::User
    } else if r.contains("timed") || r.contains("tempor") || r.contains("network") {
        ErrorKind::Transient
    } else {
        ErrorKind::System
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
        let mut changed_since_snapshot: Vec<NodeId> = Vec::new();

        for entry in WalkDir::new(&root).follow_links(false).into_iter() {
            if cancel_clone.load(Ordering::SeqCst) {
                break;
            }

            let entry = match entry {
                Ok(v) => v,
                Err(e) => {
                    summary.error_count += 1;
                    telemetry::record_scan_error();
                    let path = e
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| root.display().to_string());
                    errors.push(ScanErrorRecord {
                        path,
                        reason: format!("walkdir: {e}"),
                        kind: classify_error(&format!("walkdir: {e}")),
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
                    telemetry::record_scan_error();
                    errors.push(ScanErrorRecord {
                        path: path.clone(),
                        reason: format!("metadata: {e}"),
                        kind: classify_error(&format!("metadata: {e}")),
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
                let node_id = store.add_node(
                    parent,
                    entry.file_name().to_string_lossy().to_string(),
                    path.clone(),
                    NodeKind::Dir,
                    0,
                );
                changed_since_snapshot.push(node_id);
                batch.push(BatchEntry {
                    path,
                    is_dir: true,
                    size: 0,
                });
            } else {
                summary.scanned_files += 1;
                summary.bytes_observed += meta.len();
                let node_id = store.add_node(
                    parent,
                    entry.file_name().to_string_lossy().to_string(),
                    path.clone(),
                    NodeKind::File,
                    meta.len(),
                );
                changed_since_snapshot.push(node_id);
                batch.push(BatchEntry {
                    path,
                    is_dir: false,
                    size: meta.len(),
                });
            }
            telemetry::record_scan_item();

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
                let top_files_delta = store
                    .top_n_largest_files(10)
                    .into_iter()
                    .map(|n| n.id)
                    .collect();
                let top_dirs_delta = store.largest_dirs(10).into_iter().map(|n| n.id).collect();
                let _ = tx.send(ScanEvent::Snapshot {
                    delta: SnapshotDelta {
                        changed_nodes: std::mem::take(&mut changed_since_snapshot),
                        summary: summary.clone(),
                        top_files_delta,
                        top_dirs_delta,
                    },
                    store: store.clone(),
                });
                telemetry::record_snapshot();
                last_snapshot = Instant::now();
            }

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
        let top_files_delta = store
            .top_n_largest_files(10)
            .into_iter()
            .map(|n| n.id)
            .collect();
        let top_dirs_delta = store.largest_dirs(10).into_iter().map(|n| n.id).collect();
        let _ = tx.send(ScanEvent::Snapshot {
            delta: SnapshotDelta {
                changed_nodes: changed_since_snapshot,
                summary: summary.clone(),
                top_files_delta,
                top_dirs_delta,
            },
            store: store.clone(),
        });
        telemetry::record_snapshot();
        let _ = tx.send(ScanEvent::Finished {
            store,
            summary,
            errors,
        });
    });

    ScanHandle { events: rx, cancel }
}
