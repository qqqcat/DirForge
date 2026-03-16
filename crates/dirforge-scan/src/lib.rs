use dirforge_core::{NodeKind, NodeStore, ScanErrorRecord, ScanSummary};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};

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
}

#[derive(Debug, Clone)]
pub enum ScanEvent {
    Progress(ScanProgress),
    Snapshot(NodeStore),
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

pub fn start_scan(root: PathBuf) -> ScanHandle {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);

    std::thread::spawn(move || {
        let mut store = NodeStore::default();
        let mut summary = ScanSummary::default();
        let mut errors = Vec::new();

        let _ = tx.send(ScanEvent::Progress(ScanProgress {
            stage: ScanStage::Planning,
            current_path: Some(root.display().to_string()),
            summary: summary.clone(),
        }));

        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();
        let root_id = store.add_node(
            None,
            root_name,
            root.display().to_string(),
            NodeKind::Dir,
            0,
        );

        walk(
            &root,
            root_id,
            &mut store,
            &mut summary,
            &mut errors,
            &tx,
            &cancel_clone,
        );

        store.rollup();
        let _ = tx.send(ScanEvent::Snapshot(store.clone()));
        let _ = tx.send(ScanEvent::Finished {
            store,
            summary,
            errors,
        });
    });

    ScanHandle { events: rx, cancel }
}

fn walk(
    dir: &Path,
    parent: dirforge_core::NodeId,
    store: &mut NodeStore,
    summary: &mut ScanSummary,
    errors: &mut Vec<ScanErrorRecord>,
    tx: &std::sync::mpsc::Sender<ScanEvent>,
    cancel: &Arc<AtomicBool>,
) {
    if cancel.load(Ordering::SeqCst) {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            summary.error_count += 1;
            errors.push(ScanErrorRecord {
                path: dir.display().to_string(),
                reason: format!("read_dir: {e}"),
            });
            return;
        }
    };

    for entry in entries {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let entry = match entry {
            Ok(v) => v,
            Err(e) => {
                summary.error_count += 1;
                errors.push(ScanErrorRecord {
                    path: dir.display().to_string(),
                    reason: format!("entry: {e}"),
                });
                continue;
            }
        };
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(v) => v,
            Err(e) => {
                summary.error_count += 1;
                errors.push(ScanErrorRecord {
                    path: path.display().to_string(),
                    reason: format!("metadata: {e}"),
                });
                continue;
            }
        };

        if meta.is_dir() {
            summary.scanned_dirs += 1;
            let id = store.add_node(
                Some(parent),
                entry.file_name().to_string_lossy().to_string(),
                path.display().to_string(),
                NodeKind::Dir,
                0,
            );
            let _ = tx.send(ScanEvent::Progress(ScanProgress {
                stage: ScanStage::Enumerating,
                current_path: Some(path.display().to_string()),
                summary: summary.clone(),
            }));
            walk(&path, id, store, summary, errors, tx, cancel);
        } else {
            summary.scanned_files += 1;
            summary.bytes_observed += meta.len();
            store.add_node(
                Some(parent),
                entry.file_name().to_string_lossy().to_string(),
                path.display().to_string(),
                NodeKind::File,
                meta.len(),
            );
            let _ = tx.send(ScanEvent::Progress(ScanProgress {
                stage: ScanStage::Enumerating,
                current_path: Some(path.display().to_string()),
                summary: summary.clone(),
            }));
        }
    }
}
