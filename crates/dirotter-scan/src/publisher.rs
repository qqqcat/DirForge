use crate::{BatchEntry, ScanEvent, ScanProgress, ScanStage, SnapshotView};
use dirotter_core::{ScanSummary, SnapshotDelta};
use dirotter_telemetry as telemetry;
use std::collections::VecDeque;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::time::{Duration, Instant};

pub struct Publisher {
    tx: SyncSender<ScanEvent>,
    batch_size: usize,
    snapshot_interval: Duration,
    progress_interval: Duration,
    frontier: VecDeque<String>,
    batch: Vec<BatchEntry>,
    last_snapshot: Instant,
    last_progress: Instant,
    publisher_lag: usize,
}

impl Publisher {
    pub fn new(tx: SyncSender<ScanEvent>, batch_size: usize, snapshot_ms: u64) -> Self {
        Self {
            tx,
            batch_size: batch_size.max(1),
            snapshot_interval: Duration::from_millis(snapshot_ms.max(50)),
            progress_interval: Duration::from_millis(100),
            frontier: VecDeque::new(),
            batch: Vec::with_capacity(batch_size.max(1)),
            last_snapshot: Instant::now(),
            last_progress: Instant::now() - Duration::from_millis(100),
            publisher_lag: 0,
        }
    }

    pub fn send_planning(&self, root: String, summary: ScanSummary) {
        let _ = self.tx.send(ScanEvent::Progress(ScanProgress {
            stage: ScanStage::Planning,
            current_path: Some(root),
            summary,
            queue_depth: 0,
            metadata_backlog: 0,
            publisher_lag: self.publisher_lag,
        }));
    }

    pub fn on_batch_entry(
        &mut self,
        entry: BatchEntry,
        summary: &ScanSummary,
        metadata_backlog: usize,
    ) {
        self.frontier.push_back(entry.path.clone());
        self.batch.push(entry);

        while self.frontier.len() > 32 {
            let _ = self.frontier.pop_front();
        }
        if self.batch.len() >= self.batch_size {
            let started = Instant::now();
            let size = self.batch.len();
            let batch = std::mem::take(&mut self.batch);
            match self.tx.try_send(ScanEvent::Batch(batch)) {
                Ok(()) => {
                    self.publisher_lag = self.publisher_lag.saturating_sub(size);
                    telemetry::record_scan_batch(size, started.elapsed().as_millis() as u64);
                    telemetry::record_batch_flush_size(size);
                }
                Err(TrySendError::Full(_)) => {
                    telemetry::record_ui_backpressure(1, 0);
                }
                Err(TrySendError::Disconnected(_)) => {}
            }
        } else {
            self.publisher_lag = self.publisher_lag.saturating_add(1);
        }

        self.maybe_send_progress(summary, metadata_backlog, false);
    }

    fn maybe_send_progress(&mut self, summary: &ScanSummary, metadata_backlog: usize, force: bool) {
        if !force && self.last_progress.elapsed() < self.progress_interval {
            return;
        }

        match self.tx.try_send(ScanEvent::Progress(ScanProgress {
            stage: ScanStage::Enumerating,
            current_path: self.frontier.back().cloned(),
            summary: summary.clone(),
            queue_depth: self.frontier.len(),
            metadata_backlog,
            publisher_lag: self.publisher_lag,
        })) {
            Ok(()) => self.last_progress = Instant::now(),
            Err(TrySendError::Full(_)) => {
                self.last_progress = Instant::now();
                telemetry::record_ui_backpressure(1, 0);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    pub fn should_emit_snapshot(&self) -> bool {
        self.last_snapshot.elapsed() >= self.snapshot_interval
    }

    pub fn send_snapshot_if_due(&mut self, delta: SnapshotDelta, view: SnapshotView) {
        if !self.should_emit_snapshot() {
            return;
        }
        match self.tx.try_send(ScanEvent::Snapshot { delta, view }) {
            Ok(()) => self.last_snapshot = Instant::now(),
            Err(TrySendError::Full(_)) => {
                self.last_snapshot = Instant::now();
                telemetry::record_ui_backpressure(0, 1);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    pub fn flush_batch(&mut self) {
        if !self.batch.is_empty() {
            let started = Instant::now();
            let size = self.batch.len();
            let batch = std::mem::take(&mut self.batch);
            match self.tx.try_send(ScanEvent::Batch(batch)) {
                Ok(()) => {
                    self.publisher_lag = self.publisher_lag.saturating_sub(size);
                    telemetry::record_scan_batch(size, started.elapsed().as_millis() as u64);
                    telemetry::record_batch_flush_size(size);
                }
                Err(TrySendError::Full(_)) => telemetry::record_ui_backpressure(1, 0),
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
    }

    pub fn send_snapshot(&self, delta: SnapshotDelta, view: SnapshotView) {
        let _ = self.tx.send(ScanEvent::Snapshot { delta, view });
    }

    pub fn send_finished(
        &self,
        summary: ScanSummary,
        errors: Vec<dirotter_core::ScanErrorRecord>,
        top_files: Vec<(String, u64)>,
        top_dirs: Vec<(String, u64)>,
    ) {
        let _ = self.tx.send(ScanEvent::Finished {
            summary,
            errors,
            top_files,
            top_dirs,
        });
    }
}
