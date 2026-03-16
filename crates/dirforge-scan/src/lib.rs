mod aggregator;
mod publisher;
mod walker;

use aggregator::Aggregator;
use dirforge_core::{
    ErrorKind, NodeStore, ScanErrorRecord, ScanProfile, ScanSummary, SnapshotDelta,
};
use dirforge_telemetry as telemetry;
use publisher::Publisher;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
use std::time::Instant;
use walker::WalkerEvent;

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
        top_files: Vec<(String, u64)>,
        top_dirs: Vec<(String, u64)>,
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
    pub metadata_parallelism: usize,
    pub deep_tasks_throttle: usize,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            profile: ScanProfile::Ssd,
            batch_size: 256,
            snapshot_ms: 75,
            metadata_parallelism: 4,
            deep_tasks_throttle: 64,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProfileTuning {
    pub batch_size: usize,
    pub snapshot_ms: u64,
    pub metadata_parallelism: usize,
    pub deep_tasks_throttle: usize,
}

impl ScanConfig {
    pub(crate) fn tuned(self) -> ProfileTuning {
        let (batch_mult, snapshot_mult, parallel_cap, deep_divisor) = match self.profile {
            ScanProfile::Ssd => (1.0, 1.0, 16, 1),
            ScanProfile::Hdd => (0.75, 1.5, 6, 2),
            ScanProfile::Network => (0.5, 2.0, 3, 3),
        };

        ProfileTuning {
            batch_size: ((self.batch_size.max(1) as f32) * batch_mult).round() as usize,
            snapshot_ms: ((self.snapshot_ms.max(50) as f32) * snapshot_mult).round() as u64,
            metadata_parallelism: self.metadata_parallelism.clamp(1, parallel_cap),
            deep_tasks_throttle: (self.deep_tasks_throttle.max(1) / deep_divisor).max(1),
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
        let tuning = config.tuned();
        let root_path = root.display().to_string();
        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();

        let mut aggregator = Aggregator::new(root_name, root_path.clone());
        let mut publisher = Publisher::new(tx, tuning.batch_size.max(1), tuning.snapshot_ms);
        publisher.send_planning(root_path, aggregator.summary.clone());

        let (walker_tx, walker_rx) = mpsc::sync_channel(tuning.batch_size.max(64) * 4);
        let walker_cancel = Arc::clone(&cancel_clone);
        let walker_thread = std::thread::spawn(move || {
            walker::walk_events(root, tuning, walker_cancel, |event| {
                let _ = walker_tx.send(event);
            });
        });

        while let Ok(event) = walker_rx.recv() {
            match event {
                WalkerEvent::Error(err) => {
                    aggregator.on_error(err);
                    telemetry::record_scan_error();
                }
                WalkerEvent::Entry(entry) => {
                    let entry = aggregator.on_entry(entry);
                    publisher.on_batch_entry(entry, &aggregator.summary);
                    telemetry::record_scan_item();

                    if publisher.should_emit_snapshot() {
                        let snapshot_start = Instant::now();
                        let (delta, top_files, top_dirs) = aggregator.make_snapshot_data();
                        publisher.send_snapshot_if_due(delta, top_files, top_dirs);
                        telemetry::record_snapshot();
                        telemetry::record_snapshot_commit(
                            snapshot_start.elapsed().as_millis() as u64
                        );
                    }
                }
            }
        }

        let _ = walker_thread.join();

        publisher.flush_batch();
        let (store, summary, errors, final_delta, top_files, top_dirs) = aggregator.finalize();
        publisher.send_snapshot(final_delta, top_files, top_dirs);
        telemetry::record_snapshot();
        publisher.send_finished(store, summary, errors);
    });

    ScanHandle { events: rx, cancel }
}
