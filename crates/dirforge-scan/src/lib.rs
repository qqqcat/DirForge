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
        let root_path = root.display().to_string();
        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();

        let mut aggregator = Aggregator::new(root_name, root_path.clone());
        let mut publisher = Publisher::new(tx, config.batch_size, config.snapshot_ms);
        publisher.send_planning(root_path, aggregator.summary.clone());

        walker::walk_events(root, config.profile, cancel_clone, |event| match event {
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
                    publisher
                        .send_snapshot_if_due(aggregator.make_snapshot_delta(), &aggregator.store);
                    telemetry::record_snapshot();
                    telemetry::record_snapshot_commit(snapshot_start.elapsed().as_millis() as u64);
                }
            }
        });

        publisher.flush_batch();
        let (store, summary, errors, final_delta) = aggregator.finalize();
        publisher.send_snapshot(final_delta, store.clone());
        telemetry::record_snapshot();
        publisher.send_finished(store, summary, errors);
    });

    ScanHandle { events: rx, cancel }
}
