mod aggregator;
mod publisher;
mod walker;

use aggregator::Aggregator;
use dirotter_core::{
    ErrorKind, NodeId, NodeStore, ResolvedNode, ScanErrorRecord, ScanProfile, ScanSummary,
    SnapshotDelta,
};
use dirotter_telemetry as telemetry;
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

pub type RankedPath = (Arc<str>, u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanStage {
    Planning,
    Enumerating,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub stage: ScanStage,
    pub current_path: Option<Arc<str>>,
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
pub struct LiveSnapshotView {
    pub changed_node_count: usize,
    pub top_files: Vec<RankedPath>,
    pub top_dirs: Vec<RankedPath>,
    pub selection: SelectionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSnapshotView {
    pub changed_node_count: usize,
    pub nodes: Vec<ResolvedNode>,
    pub top_files: Vec<RankedPath>,
    pub top_dirs: Vec<RankedPath>,
    pub selection: SelectionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotView {
    Live(LiveSnapshotView),
    Full(FullSnapshotView),
}

impl SnapshotView {
    pub fn changed_node_count(&self) -> usize {
        match self {
            Self::Live(view) => view.changed_node_count,
            Self::Full(view) => view.changed_node_count,
        }
    }

    pub fn materialized_node_count(&self) -> usize {
        match self {
            Self::Live(_) => 0,
            Self::Full(view) => view.nodes.len(),
        }
    }

    pub fn ranked_item_count(&self) -> usize {
        match self {
            Self::Live(view) => view.top_files.len() + view.top_dirs.len(),
            Self::Full(view) => view.top_files.len() + view.top_dirs.len(),
        }
    }

    pub fn estimated_text_bytes(&self) -> u64 {
        match self {
            Self::Live(view) => {
                let top_file_text = view
                    .top_files
                    .iter()
                    .map(|(path, _)| path.len() as u64)
                    .sum::<u64>();
                let top_dir_text = view
                    .top_dirs
                    .iter()
                    .map(|(path, _)| path.len() as u64)
                    .sum::<u64>();
                top_file_text + top_dir_text
            }
            Self::Full(view) => {
                let node_text = view
                    .nodes
                    .iter()
                    .map(|node| (node.name.len() + node.path.len()) as u64)
                    .sum::<u64>();
                let top_file_text = view
                    .top_files
                    .iter()
                    .map(|(path, _)| path.len() as u64)
                    .sum::<u64>();
                let top_dir_text = view
                    .top_dirs
                    .iter()
                    .map(|(path, _)| path.len() as u64)
                    .sum::<u64>();
                node_text + top_file_text + top_dir_text
            }
        }
    }

    pub fn into_rankings(self) -> (Vec<RankedPath>, Vec<RankedPath>) {
        match self {
            Self::Live(view) => (view.top_files, view.top_dirs),
            Self::Full(view) => (view.top_files, view.top_dirs),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchEntry {
    pub path: Arc<str>,
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
        store: NodeStore,
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

    pub fn into_parts(self) -> (Receiver<ScanEvent>, Arc<AtomicBool>) {
        (self.events, self.cancel)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanMode {
    Quick,
    Deep,
    LargeDisk,
}

impl ScanMode {
    pub fn as_setting_value(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Deep => "deep",
            Self::LargeDisk => "large-disk",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value {
            "quick" => Some(Self::Quick),
            "deep" => Some(Self::Deep),
            "large-disk" => Some(Self::LargeDisk),
            _ => None,
        }
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
        Self::for_mode(ScanMode::Quick)
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
    pub fn for_mode(mode: ScanMode) -> Self {
        match mode {
            ScanMode::Quick => Self {
                profile: ScanProfile::Ssd,
                batch_size: 256,
                snapshot_ms: 75,
                metadata_parallelism: 4,
                deep_tasks_throttle: 64,
            },
            ScanMode::Deep => Self {
                profile: ScanProfile::Hdd,
                batch_size: 192,
                snapshot_ms: 60,
                metadata_parallelism: 6,
                deep_tasks_throttle: 96,
            },
            ScanMode::LargeDisk => Self {
                profile: ScanProfile::Network,
                batch_size: 640,
                snapshot_ms: 150,
                metadata_parallelism: 3,
                deep_tasks_throttle: 192,
            },
        }
    }

    pub fn effective_batch_size(self) -> usize {
        self.tuned().batch_size.max(1)
    }

    pub fn effective_snapshot_ms(self) -> u64 {
        self.tuned().snapshot_ms.max(50)
    }

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
        publisher.send_planning(root_path.clone().into(), aggregator.summary.clone());

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
                WalkerEvent::Entry {
                    entry,
                    send_blocked_ms,
                } => {
                    telemetry::record_walker_recv_blocked(send_blocked_ms as u64);

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
                        telemetry::record_snapshot_view(
                            view.changed_node_count() as u64,
                            view.materialized_node_count() as u64,
                            view.ranked_item_count() as u64,
                            view.estimated_text_bytes(),
                        );
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
        let (summary, final_store, errors, final_delta, final_view) = aggregator.finalize();
        let finished_payload_size = serde_json::to_vec(&final_view)
            .map(|payload| payload.len() as u64)
            .unwrap_or_default();
        telemetry::record_scan_finished(final_store.nodes.len() as u64, finished_payload_size);
        telemetry::record_snapshot_view(
            final_view.changed_node_count() as u64,
            final_view.materialized_node_count() as u64,
            final_view.ranked_item_count() as u64,
            final_view.estimated_text_bytes(),
        );

        if cancel_clone.load(Ordering::SeqCst) {
            let cancel_elapsed = cancelled_at
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or_else(|| scan_started.elapsed().as_millis() as u64);
            telemetry::record_cancelled_scan_latency(cancel_elapsed);
        }

        publisher.send_snapshot(final_delta, final_view);
        telemetry::record_snapshot();
        publisher.send_finished(summary, final_store, errors);
    });

    ScanHandle { events: rx, cancel }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_mode_is_the_default_config() {
        let default_config = ScanConfig::default();
        let quick_config = ScanConfig::for_mode(ScanMode::Quick);

        assert_eq!(default_config.profile, quick_config.profile);
        assert_eq!(default_config.batch_size, quick_config.batch_size);
        assert_eq!(default_config.snapshot_ms, quick_config.snapshot_ms);
        assert_eq!(
            default_config.metadata_parallelism,
            quick_config.metadata_parallelism
        );
        assert_eq!(
            default_config.deep_tasks_throttle,
            quick_config.deep_tasks_throttle
        );
    }

    #[test]
    fn scan_modes_round_trip_to_settings() {
        for mode in [ScanMode::Quick, ScanMode::Deep, ScanMode::LargeDisk] {
            assert_eq!(ScanMode::from_setting(mode.as_setting_value()), Some(mode));
        }
    }

    #[test]
    fn large_disk_mode_uses_the_most_conservative_publish_cadence() {
        let quick = ScanConfig::for_mode(ScanMode::Quick);
        let deep = ScanConfig::for_mode(ScanMode::Deep);
        let large = ScanConfig::for_mode(ScanMode::LargeDisk);

        assert!(deep.effective_batch_size() < quick.effective_batch_size());
        assert!(large.effective_batch_size() > quick.effective_batch_size());
        assert!(deep.effective_snapshot_ms() > quick.effective_snapshot_ms());
        assert!(large.effective_snapshot_ms() > deep.effective_snapshot_ms());
    }
}
