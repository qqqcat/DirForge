use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

const COLLECTION_PERIOD_MS: u64 = 500;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static LAST_COLLECTION_TICK_MS: AtomicU64 = AtomicU64::new(0);
static SCAN_ITEMS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SCAN_ERRORS_TOTAL: AtomicU64 = AtomicU64::new(0);
static ACTION_ATTEMPTED_TOTAL: AtomicU64 = AtomicU64::new(0);
static ACTION_FAILED_TOTAL: AtomicU64 = AtomicU64::new(0);

static SCAN_BATCHES_TOTAL: AtomicU64 = AtomicU64::new(0);
static SCAN_BATCH_ITEMS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SCAN_BATCH_ELAPSED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_COMMITS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_COMMIT_ELAPSED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_CALLS_TOTAL: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_ELAPSED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);

static ACTION_AUDIT_RING: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    pub collection_period_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricDescriptor {
    pub name: &'static str,
    pub unit: &'static str,
    pub period_ms: u64,
}

pub fn metric_descriptors() -> Vec<MetricDescriptor> {
    vec![
        MetricDescriptor {
            name: "df.scan.items.total",
            unit: "count",
            period_ms: COLLECTION_PERIOD_MS,
        },
        MetricDescriptor {
            name: "df.scan.errors.total",
            unit: "count",
            period_ms: COLLECTION_PERIOD_MS,
        },
        MetricDescriptor {
            name: "df.scan.batch.elapsed_ms.avg",
            unit: "ms",
            period_ms: COLLECTION_PERIOD_MS,
        },
        MetricDescriptor {
            name: "df.snapshot.commit.elapsed_ms.avg",
            unit: "ms",
            period_ms: COLLECTION_PERIOD_MS,
        },
        MetricDescriptor {
            name: "df.action.failed.total",
            unit: "count",
            period_ms: COLLECTION_PERIOD_MS,
        },
    ]
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
        tracing::info!(
            event = "telemetry.init",
            period_ms = COLLECTION_PERIOD_MS,
            "telemetry initialized"
        );
    }
}

pub fn record_scan_item() {
    SCAN_ITEMS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn record_scan_batch(size: usize, elapsed_ms: u64) {
    SCAN_BATCHES_TOTAL.fetch_add(1, Ordering::Relaxed);
    SCAN_BATCH_ITEMS_TOTAL.fetch_add(size as u64, Ordering::Relaxed);
    SCAN_BATCH_ELAPSED_MS_TOTAL.fetch_add(elapsed_ms, Ordering::Relaxed);
    tracing::debug!(
        event = "scan.batch",
        metric = "df.scan.batch.elapsed_ms",
        size,
        elapsed_ms
    );
}

pub fn record_snapshot() {
    SNAPSHOTS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn record_snapshot_commit(elapsed_ms: u64) {
    SNAPSHOT_COMMITS_TOTAL.fetch_add(1, Ordering::Relaxed);
    SNAPSHOT_COMMIT_ELAPSED_MS_TOTAL.fetch_add(elapsed_ms, Ordering::Relaxed);
    tracing::debug!(
        event = "snapshot.commit",
        metric = "df.snapshot.commit.elapsed_ms",
        elapsed_ms
    );
}

pub fn record_duplicate_hash(bytes: u64, elapsed_ms: u64) {
    DUP_HASH_BYTES_TOTAL.fetch_add(bytes, Ordering::Relaxed);
    DUP_HASH_CALLS_TOTAL.fetch_add(1, Ordering::Relaxed);
    DUP_HASH_ELAPSED_MS_TOTAL.fetch_add(elapsed_ms, Ordering::Relaxed);
    tracing::debug!(
        event = "duplicate.hash",
        metric = "df.duplicate.hash.elapsed_ms",
        bytes,
        elapsed_ms
    );
}

pub fn record_scan_error() {
    SCAN_ERRORS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn record_action_result(success: bool) {
    ACTION_ATTEMPTED_TOTAL.fetch_add(1, Ordering::Relaxed);
    if !success {
        ACTION_FAILED_TOTAL.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_action_audit(payload: String) {
    let ring = ACTION_AUDIT_RING.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut r) = ring.lock() {
        r.push(payload);
        if r.len() > 256 {
            let drop_n = r.len() - 256;
            r.drain(0..drop_n);
        }
    }
}

pub fn action_audit_tail(limit: usize) -> Vec<String> {
    let ring = ACTION_AUDIT_RING.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(r) = ring.lock() {
        let start = r.len().saturating_sub(limit);
        return r[start..].to_vec();
    }
    Vec::new()
}

pub fn snapshot() -> TelemetrySnapshot {
    LAST_COLLECTION_TICK_MS.fetch_add(COLLECTION_PERIOD_MS, Ordering::Relaxed);

    let batches = SCAN_BATCHES_TOTAL.load(Ordering::Relaxed);
    let batch_items = SCAN_BATCH_ITEMS_TOTAL.load(Ordering::Relaxed);
    let batch_elapsed = SCAN_BATCH_ELAPSED_MS_TOTAL.load(Ordering::Relaxed);
    let snapshot_commits = SNAPSHOT_COMMITS_TOTAL.load(Ordering::Relaxed);
    let snapshot_elapsed = SNAPSHOT_COMMIT_ELAPSED_MS_TOTAL.load(Ordering::Relaxed);

    TelemetrySnapshot {
        scan_items: SCAN_ITEMS_TOTAL.load(Ordering::Relaxed),
        snapshots: SNAPSHOTS_TOTAL.load(Ordering::Relaxed),
        scan_errors: SCAN_ERRORS_TOTAL.load(Ordering::Relaxed),
        action_attempted: ACTION_ATTEMPTED_TOTAL.load(Ordering::Relaxed),
        action_failed: ACTION_FAILED_TOTAL.load(Ordering::Relaxed),
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
        duplicate_hash_bytes: DUP_HASH_BYTES_TOTAL.load(Ordering::Relaxed),
        avg_duplicate_hash_ms: {
            let hash_calls = DUP_HASH_CALLS_TOTAL.load(Ordering::Relaxed);
            if hash_calls == 0 {
                0
            } else {
                DUP_HASH_ELAPSED_MS_TOTAL.load(Ordering::Relaxed) / hash_calls
            }
        },
        collection_period_ms: COLLECTION_PERIOD_MS,
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
        record_action_audit("{}".into());
        let s = snapshot();
        assert!(s.scan_items >= 1);
        assert!(s.snapshots >= 1);
        assert!(s.scan_errors >= 1);
        assert!(s.action_attempted >= 1);
        assert!(s.action_failed >= 1);
        assert!(s.scan_batches >= 1);
        assert!(s.duplicate_hash_bytes >= 1024);
        assert_eq!(s.collection_period_ms, COLLECTION_PERIOD_MS);
        assert!(!action_audit_tail(1).is_empty());
    }
}
