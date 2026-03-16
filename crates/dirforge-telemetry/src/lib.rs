use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static SCAN_ITEMS: AtomicU64 = AtomicU64::new(0);
static SNAPSHOTS: AtomicU64 = AtomicU64::new(0);
static SCAN_ERRORS: AtomicU64 = AtomicU64::new(0);
static ACTION_ATTEMPTED: AtomicU64 = AtomicU64::new(0);
static ACTION_FAILED: AtomicU64 = AtomicU64::new(0);

static SCAN_BATCHES: AtomicU64 = AtomicU64::new(0);
static SCAN_BATCH_ITEMS: AtomicU64 = AtomicU64::new(0);
static SCAN_BATCH_ELAPSED_MS: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_COMMITS: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_COMMIT_ELAPSED_MS: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_BYTES: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_CALLS: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_ELAPSED_MS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetrySnapshot {
    pub scan_items: u64,
    pub snapshots: u64,
    pub scan_errors: u64,
    pub action_attempted: u64,
    pub action_failed: u64,
    pub scan_batches: u64,
    pub avg_scan_batch_size: u64,
    pub avg_scan_batch_elapsed_ms: u64,
    pub snapshot_commits: u64,
    pub avg_snapshot_commit_ms: u64,
    pub duplicate_hash_bytes: u64,
    pub avg_duplicate_hash_ms: u64,
}

pub fn init() {
    if INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .try_init();
        tracing::info!(event = "telemetry.init", "telemetry initialized");
    }
}

pub fn record_scan_item() {
    SCAN_ITEMS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_scan_batch(size: usize, elapsed_ms: u64) {
    SCAN_BATCHES.fetch_add(1, Ordering::Relaxed);
    SCAN_BATCH_ITEMS.fetch_add(size as u64, Ordering::Relaxed);
    SCAN_BATCH_ELAPSED_MS.fetch_add(elapsed_ms, Ordering::Relaxed);
    tracing::debug!(event = "scan.batch", size, elapsed_ms);
}

pub fn record_snapshot() {
    SNAPSHOTS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_snapshot_commit(elapsed_ms: u64) {
    SNAPSHOT_COMMITS.fetch_add(1, Ordering::Relaxed);
    SNAPSHOT_COMMIT_ELAPSED_MS.fetch_add(elapsed_ms, Ordering::Relaxed);
    tracing::debug!(event = "snapshot.commit", elapsed_ms);
}

pub fn record_duplicate_hash(bytes: u64, elapsed_ms: u64) {
    DUP_HASH_BYTES.fetch_add(bytes, Ordering::Relaxed);
    DUP_HASH_CALLS.fetch_add(1, Ordering::Relaxed);
    DUP_HASH_ELAPSED_MS.fetch_add(elapsed_ms, Ordering::Relaxed);
    tracing::debug!(event = "duplicate.hash", bytes, elapsed_ms);
}

pub fn record_scan_error() {
    SCAN_ERRORS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_action_result(success: bool) {
    ACTION_ATTEMPTED.fetch_add(1, Ordering::Relaxed);
    if !success {
        ACTION_FAILED.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn snapshot() -> TelemetrySnapshot {
    let batches = SCAN_BATCHES.load(Ordering::Relaxed);
    let batch_items = SCAN_BATCH_ITEMS.load(Ordering::Relaxed);
    let batch_elapsed = SCAN_BATCH_ELAPSED_MS.load(Ordering::Relaxed);
    let snapshot_commits = SNAPSHOT_COMMITS.load(Ordering::Relaxed);
    let snapshot_elapsed = SNAPSHOT_COMMIT_ELAPSED_MS.load(Ordering::Relaxed);

    TelemetrySnapshot {
        scan_items: SCAN_ITEMS.load(Ordering::Relaxed),
        snapshots: SNAPSHOTS.load(Ordering::Relaxed),
        scan_errors: SCAN_ERRORS.load(Ordering::Relaxed),
        action_attempted: ACTION_ATTEMPTED.load(Ordering::Relaxed),
        action_failed: ACTION_FAILED.load(Ordering::Relaxed),
        scan_batches: batches,
        avg_scan_batch_size: if batches == 0 {
            0
        } else {
            batch_items / batches
        },
        avg_scan_batch_elapsed_ms: if batches == 0 {
            0
        } else {
            batch_elapsed / batches
        },
        snapshot_commits,
        avg_snapshot_commit_ms: if snapshot_commits == 0 {
            0
        } else {
            snapshot_elapsed / snapshot_commits
        },
        duplicate_hash_bytes: DUP_HASH_BYTES.load(Ordering::Relaxed),
        avg_duplicate_hash_ms: {
            let hash_calls = DUP_HASH_CALLS.load(Ordering::Relaxed);
            if hash_calls == 0 {
                0
            } else {
                DUP_HASH_ELAPSED_MS.load(Ordering::Relaxed) / hash_calls
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_accumulate() {
        record_scan_item();
        record_snapshot();
        record_scan_error();
        record_action_result(false);
        record_scan_batch(10, 5);
        record_snapshot_commit(4);
        record_duplicate_hash(1024, 2);
        let s = snapshot();
        assert!(s.scan_items >= 1);
        assert!(s.snapshots >= 1);
        assert!(s.scan_errors >= 1);
        assert!(s.action_attempted >= 1);
        assert!(s.action_failed >= 1);
        assert!(s.scan_batches >= 1);
        assert!(s.duplicate_hash_bytes >= 1024);
    }
}
