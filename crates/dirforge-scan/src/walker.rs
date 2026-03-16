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
    pub metadata_backlog: usize,
    pub recv_blocked_ms: u32,
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
    let max_backlog =
        (tuning.deep_tasks_throttle * tuning.duplicate_scheduling_hint).max(worker_count);
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

                    while pending_dirs.load(Ordering::SeqCst) > max_backlog {
                        if cancel.load(Ordering::SeqCst) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }

                    process_directory(
                        &current_dir,
                        &root_string,
                        &queue,
                        &pending_dirs,
                        &cancel,
                        &event_tx,
                        tuning,
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
    tuning: ProfileTuning,
) {
    let mut read_dir_attempt = 0u8;
    let read_dir = loop {
        match fs::read_dir(dir) {
            Ok(entries) => break entries,
            Err(e) => {
                let reason = format!("read_dir: {e}");
                let _ = event_tx.send(WalkerEvent::Error(ScanErrorRecord {
                    path: dir.display().to_string(),
                    reason: reason.clone(),
                    kind: classify_error(&reason),
                }));

                if read_dir_attempt >= tuning.error_retry_limit {
                    return;
                }

                read_dir_attempt = read_dir_attempt.saturating_add(1);
                std::thread::sleep(std::time::Duration::from_millis(
                    tuning.network_retry_backoff_ms * read_dir_attempt as u64,
                ));
            }
        }
    };

    let mut dir_entries = 0usize;
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

        let mut metadata_attempt = 0u8;
        let metadata = loop {
            match fs::symlink_metadata(&path) {
                Ok(v) => break Some(v),
                Err(e) => {
                    let reason = format!("metadata: {e}");
                    let _ = event_tx.send(WalkerEvent::Error(ScanErrorRecord {
                        path: path_string.clone(),
                        reason: reason.clone(),
                        kind: classify_error(&reason),
                    }));

                    if metadata_attempt >= tuning.error_retry_limit {
                        break None;
                    }

                    metadata_attempt = metadata_attempt.saturating_add(1);
                    std::thread::sleep(std::time::Duration::from_millis(
                        tuning.network_retry_backoff_ms * metadata_attempt as u64,
                    ));
                }
            }
        };

        let Some(metadata) = metadata else {
            continue;
        };

        let is_dir = metadata.is_dir();
        if is_dir {
            pending_dirs.fetch_add(1, Ordering::SeqCst);
            queue.push(path.clone());
        }

        let send_started = std::time::Instant::now();
        let _ = event_tx.send(WalkerEvent::Entry(EntryEvent {
            path: path.display().to_string(),
            parent_path: dir.display().to_string(),
            name: entry.file_name().to_string_lossy().to_string(),
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
            metadata_backlog: pending_dirs.load(Ordering::SeqCst),
            recv_blocked_ms: send_started.elapsed().as_millis() as u32,
        }));

        dir_entries += 1;
        if dir_entries > tuning.large_dir_entry_threshold {
            std::thread::sleep(std::time::Duration::from_millis(
                tuning.large_dir_backoff_ms,
            ));
            dir_entries = 0;
        }
    }
}
