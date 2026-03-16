use crate::{classify_error, ProfileTuning};
use dirforge_core::ScanErrorRecord;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct EntryEvent {
    pub path: String,
    pub parent_path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum WalkerEvent {
    Entry(EntryEvent),
    Error(ScanErrorRecord),
}

pub fn walk_events(
    root: PathBuf,
    tuning: ProfileTuning,
    cancel: Arc<AtomicBool>,
    mut on_event: impl FnMut(WalkerEvent),
) {
    let root_string = root.display().to_string();

    let mut processed = 0usize;
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .max_open(tuning.metadata_parallelism.max(1))
        .into_iter()
    {
        if cancel.load(Ordering::SeqCst) {
            break;
        }

        let entry = match entry {
            Ok(v) => v,
            Err(e) => {
                let reason = format!("walkdir: {e}");
                let path = e
                    .path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| root_string.clone());
                on_event(WalkerEvent::Error(ScanErrorRecord {
                    path,
                    reason: reason.clone(),
                    kind: classify_error(&reason),
                }));
                continue;
            }
        };

        let path = entry.path().display().to_string();
        if path == root_string {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(v) => v,
            Err(e) => {
                let reason = format!("metadata: {e}");
                on_event(WalkerEvent::Error(ScanErrorRecord {
                    path: path.clone(),
                    reason: reason.clone(),
                    kind: classify_error(&reason),
                }));
                continue;
            }
        };

        let parent_path = entry
            .path()
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| root_string.clone());

        on_event(WalkerEvent::Entry(EntryEvent {
            path,
            parent_path,
            name: entry.file_name().to_string_lossy().to_string(),
            is_dir: meta.is_dir(),
            size: if meta.is_dir() { 0 } else { meta.len() },
        }));

        processed += 1;
        if processed % tuning.deep_tasks_throttle.max(1) == 0 {
            std::thread::yield_now();
        }
    }
}
