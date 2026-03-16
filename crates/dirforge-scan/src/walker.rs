use crate::{classify_error, ProfileTuning};
use dirforge_core::ScanErrorRecord;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::SyncSender,
    Arc, Condvar, Mutex,
};

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

#[derive(Clone)]
struct DirQueue {
    inner: Arc<(Mutex<VecDeque<PathBuf>>, Condvar)>,
}

impl DirQueue {
    fn new(root: PathBuf) -> Self {
        let mut q = VecDeque::new();
        q.push_back(root);
        Self {
            inner: Arc::new((Mutex::new(q), Condvar::new())),
        }
    }

    fn push(&self, value: PathBuf) {
        let (lock, cv) = &*self.inner;
        let mut guard = lock.lock().expect("queue lock");
        guard.push_back(value);
        cv.notify_one();
    }

    fn pop_wait(&self, pending: &AtomicUsize, cancel: &AtomicBool) -> Option<PathBuf> {
        let (lock, cv) = &*self.inner;
        let mut guard = lock.lock().expect("queue lock");
        loop {
            if cancel.load(Ordering::SeqCst) {
                return None;
            }
            if let Some(v) = guard.pop_front() {
                return Some(v);
            }
            if pending.load(Ordering::SeqCst) == 0 {
                return None;
            }
            guard = cv.wait(guard).expect("queue wait");
        }
    }

    fn notify_all(&self) {
        let (_, cv) = &*self.inner;
        cv.notify_all();
    }
}

pub fn walk_events(
    root: PathBuf,
    tuning: ProfileTuning,
    cancel: Arc<AtomicBool>,
    event_tx: SyncSender<WalkerEvent>,
) {
    let worker_count = tuning.metadata_parallelism.max(1);
    let root_string = root.display().to_string();
    let queue = DirQueue::new(root);
    let pending_dirs = Arc::new(AtomicUsize::new(1));

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = queue.clone();
            let cancel = Arc::clone(&cancel);
            let event_tx = event_tx.clone();
            let pending_dirs = Arc::clone(&pending_dirs);
            let root_string = root_string.clone();

            scope.spawn(move || {
                while let Some(current_dir) = queue.pop_wait(&pending_dirs, &cancel) {
                    if cancel.load(Ordering::SeqCst) {
                        break;
                    }

                    process_directory(
                        &current_dir,
                        &root_string,
                        &queue,
                        &pending_dirs,
                        &cancel,
                        &event_tx,
                    );

                    pending_dirs.fetch_sub(1, Ordering::SeqCst);
                    queue.notify_all();
                }
            });
        }
    });
}

fn process_directory(
    dir: &Path,
    root_string: &str,
    queue: &DirQueue,
    pending_dirs: &AtomicUsize,
    cancel: &AtomicBool,
    event_tx: &SyncSender<WalkerEvent>,
) {
    let read_dir = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            let reason = format!("read_dir: {e}");
            let _ = event_tx.send(WalkerEvent::Error(ScanErrorRecord {
                path: dir.display().to_string(),
                reason: reason.clone(),
                kind: classify_error(&reason),
            }));
            return;
        }
    };

    for entry_result in read_dir {
        if cancel.load(Ordering::SeqCst) {
            break;
        }

        let entry = match entry_result {
            Ok(v) => v,
            Err(e) => {
                let reason = format!("read_dir entry: {e}");
                let _ = event_tx.send(WalkerEvent::Error(ScanErrorRecord {
                    path: dir.display().to_string(),
                    reason: reason.clone(),
                    kind: classify_error(&reason),
                }));
                continue;
            }
        };

        let path = entry.path();
        let path_string = path.display().to_string();
        if path_string == root_string {
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(v) => v,
            Err(e) => {
                let reason = format!("metadata: {e}");
                let _ = event_tx.send(WalkerEvent::Error(ScanErrorRecord {
                    path: path_string,
                    reason: reason.clone(),
                    kind: classify_error(&reason),
                }));
                continue;
            }
        };

        let is_dir = metadata.is_dir();
        if is_dir {
            pending_dirs.fetch_add(1, Ordering::SeqCst);
            queue.push(path.clone());
        }

        let _ = event_tx.send(WalkerEvent::Entry(EntryEvent {
            path: path.display().to_string(),
            parent_path: dir.display().to_string(),
            name: entry.file_name().to_string_lossy().to_string(),
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
        }));
    }
}
