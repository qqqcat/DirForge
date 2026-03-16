use crate::{BatchEntry, ScanEvent, ScanProgress, ScanStage};
use dirforge_core::{ScanSummary, SnapshotDelta};
use dirforge_telemetry as telemetry;
use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

pub struct Publisher {
    tx: Sender<ScanEvent>,
    batch_size: usize,
    snapshot_interval: Duration,
    frontier: VecDeque<String>,
    batch: Vec<BatchEntry>,
    last_snapshot: Instant,
}

impl Publisher {
    pub fn new(tx: Sender<ScanEvent>, batch_size: usize, snapshot_ms: u64) -> Self {
        Self {
            tx,
            batch_size: batch_size.max(1),
            snapshot_interval: Duration::from_millis(snapshot_ms.max(50)),
            frontier: VecDeque::new(),
            batch: Vec::with_capacity(batch_size.max(1)),
            last_snapshot: Instant::now(),
        }
    }

    pub fn send_planning(&self, root: String, summary: ScanSummary) {
        let _ = self.tx.send(ScanEvent::Progress(ScanProgress {
            stage: ScanStage::Planning,
            current_path: Some(root),
            summary,
            queue_depth: 0,
        }));
    }

    pub fn on_batch_entry(&mut self, entry: BatchEntry, summary: &ScanSummary) {
        self.frontier.push_back(entry.path.clone());
        self.batch.push(entry);

        while self.frontier.len() > 32 {
            let _ = self.frontier.pop_front();
        }
        if self.batch.len() >= self.batch_size {
            let started = Instant::now();
            let size = self.batch.len();
            let _ = self
                .tx
                .send(ScanEvent::Batch(std::mem::take(&mut self.batch)));
            telemetry::record_scan_batch(size, started.elapsed().as_millis() as u64);
        }

        let _ = self.tx.send(ScanEvent::Progress(ScanProgress {
            stage: ScanStage::Enumerating,
            current_path: self.frontier.back().cloned(),
            summary: summary.clone(),
            queue_depth: self.frontier.len(),
        }));
    }

    pub fn should_emit_snapshot(&self) -> bool {
        self.last_snapshot.elapsed() >= self.snapshot_interval
    }

    pub fn send_snapshot_if_due(
        &mut self,
        delta: SnapshotDelta,
        top_files: Vec<(String, u64)>,
        top_dirs: Vec<(String, u64)>,
    ) {
        if !self.should_emit_snapshot() {
            return;
        }
        let _ = self.tx.send(ScanEvent::Snapshot {
            delta,
            top_files,
            top_dirs,
        });
        self.last_snapshot = Instant::now();
    }

    pub fn flush_batch(&mut self) {
        if !self.batch.is_empty() {
            let started = Instant::now();
            let size = self.batch.len();
            let _ = self
                .tx
                .send(ScanEvent::Batch(std::mem::take(&mut self.batch)));
            telemetry::record_scan_batch(size, started.elapsed().as_millis() as u64);
        }
    }

    pub fn send_snapshot(
        &self,
        delta: SnapshotDelta,
        top_files: Vec<(String, u64)>,
        top_dirs: Vec<(String, u64)>,
    ) {
        let _ = self.tx.send(ScanEvent::Snapshot {
            delta,
            top_files,
            top_dirs,
        });
    }

    pub fn send_finished(
        &self,
        store: dirforge_core::NodeStore,
        summary: ScanSummary,
        errors: Vec<dirforge_core::ScanErrorRecord>,
    ) {
        let _ = self.tx.send(ScanEvent::Finished {
            store,
            summary,
            errors,
        });
    }
}
