mod aggregator;
mod publisher;
mod walker;

use aggregator::Aggregator;
use dirforge_core::{
    ErrorKind, Node, NodeId, ScanErrorRecord, ScanProfile, ScanSummary, SnapshotDelta,
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
    pub metadata_backlog: usize,
    pub publisher_lag: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionState {
    pub focused: Option<NodeId>,
    pub expanded: Vec<NodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotView {
    pub nodes: Vec<Node>,
    pub top_files: Vec<(String, u64)>,
    pub top_dirs: Vec<(String, u64)>,
    pub selection: SelectionState,
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
        view: SnapshotView,
    },
    Finished {
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
    pub error_retry_limit: u8,
    pub network_retry_backoff_ms: u64,
    pub large_dir_entry_threshold: usize,
    pub large_dir_backoff_ms: u64,
    pub duplicate_scheduling_hint: usize,
    pub ui_backpressure_batch_budget: usize,
}

impl ScanConfig {
    pub(crate) fn tuned(self) -> ProfileTuning {
        let (batch_mult, snapshot_mult, parallel_cap, retry, retry_backoff, large_dir_backoff) =
            match self.profile {
                ScanProfile::Ssd => (1.0, 1.0, 16, 1, 10, 1),
                ScanProfile::Hdd => (0.75, 1.5, 6, 2, 25, 2),
                ScanProfile::Network => (0.5, 2.0, 3, 3, 50, 4),
            };

        let batch_size = ((self.batch_size.max(1) as f32) * batch_mult).round() as usize;
        let metadata_parallelism = self.metadata_parallelism.clamp(1, parallel_cap);
        let deep_tasks_throttle = self.deep_tasks_throttle.max(metadata_parallelism);

        ProfileTuning {
            batch_size,
            snapshot_ms: ((self.snapshot_ms.max(50) as f32) * snapshot_mult).round() as u64,
            metadata_parallelism,
            deep_tasks_throttle,
            error_retry_limit: retry,
            network_retry_backoff_ms: retry_backoff,
            large_dir_entry_threshold: (batch_size * 8).clamp(256, 8192),
            large_dir_backoff_ms: large_dir_backoff,
            duplicate_scheduling_hint: (deep_tasks_throttle / metadata_parallelism).max(1),
            ui_backpressure_batch_budget: (batch_size * 4).max(256),
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
    let tuning = config.tuned();
    let (tx, rx) = mpsc::sync_channel(tuning.ui_backpressure_batch_budget);
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
        let mut publisher = Publisher::new(tx, tuning.batch_size.max(1), tuning.snapshot_ms);
        publisher.send_planning(root_path, aggregator.summary.clone());

        let (walker_tx, walker_rx) = mpsc::sync_channel(tuning.ui_backpressure_batch_budget);
        let walker_cancel = Arc::clone(&cancel_clone);
        let walker_thread = std::thread::spawn(move || {
            walker::walk_events(root, tuning, walker_cancel, walker_tx);
        });

        let scan_started = Instant::now();
        let mut cancelled_at: Option<Instant> = None;
        while let Ok(event) = walker_rx.recv() {
            if cancelled_at.is_none() && cancel_clone.load(Ordering::SeqCst) {
                cancelled_at = Some(Instant::now());
            }

            match event {
                WalkerEvent::Error(err) => {
                    aggregator.on_error(err);
                    telemetry::record_scan_error();
                }
                WalkerEvent::Entry(entry) => {
                    telemetry::record_walker_recv_blocked(entry.recv_blocked_ms as u64);

                    let aggregator_started = Instant::now();
                    let metadata_backlog = entry.metadata_backlog;
                    let entries = aggregator.on_entry(entry);
                    telemetry::record_aggregator_processing(
                        aggregator_started.elapsed().as_millis() as u64,
                    );
                    for entry in entries {
                        publisher.on_batch_entry(entry, &aggregator.summary, metadata_backlog);
                        telemetry::record_scan_item();
                    }

                    if publisher.should_emit_snapshot() {
                        let snapshot_start = Instant::now();
                        let (delta, view) = aggregator.make_snapshot_data(false);
                        publisher.send_snapshot_if_due(delta, view);
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
        let (summary, errors, final_delta, final_view) = aggregator.finalize();
        let finished_payload_size = serde_json::to_vec(&final_view)
            .map(|payload| payload.len() as u64)
            .unwrap_or_default();
        telemetry::record_scan_finished(final_view.nodes.len() as u64, finished_payload_size);

        if cancel_clone.load(Ordering::SeqCst) {
            let cancel_elapsed = cancelled_at
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or_else(|| scan_started.elapsed().as_millis() as u64);
            telemetry::record_cancelled_scan_latency(cancel_elapsed);
        }

        publisher.send_snapshot(final_delta, final_view);
        telemetry::record_snapshot();
        publisher.send_finished(summary, errors);
    });

    ScanHandle { events: rx, cancel }
}
