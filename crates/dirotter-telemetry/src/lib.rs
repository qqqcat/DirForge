use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

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
static SNAPSHOT_VIEWS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_CHANGED_NODES_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_MATERIALIZED_NODES_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_RANKED_ITEMS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_TEXT_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_MAX_CHANGED_NODES: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_MAX_MATERIALIZED_NODES: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT_MAX_TEXT_BYTES: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_CALLS_TOTAL: AtomicU64 = AtomicU64::new(0);
static DUP_HASH_ELAPSED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static UI_FRAMES_TOTAL: AtomicU64 = AtomicU64::new(0);
static UI_DROPPED_BATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static UI_DROPPED_SNAPSHOT_TOTAL: AtomicU64 = AtomicU64::new(0);
static WALKER_RECV_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WALKER_RECV_BLOCKED_SAMPLES: AtomicU64 = AtomicU64::new(0);
static AGGREGATOR_PROCESSING_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static AGGREGATOR_PROCESSING_SAMPLES: AtomicU64 = AtomicU64::new(0);
static BATCH_FLUSH_ITEMS_TOTAL: AtomicU64 = AtomicU64::new(0);
static BATCH_FLUSH_EVENTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static CANCELLED_SCAN_LATENCY_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static CANCELLED_SCAN_TOTAL: AtomicU64 = AtomicU64::new(0);
static FINISHED_PAYLOAD_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
static FINISHED_NODE_COUNT_TOTAL: AtomicU64 = AtomicU64::new(0);
static FINISHED_SCAN_TOTAL: AtomicU64 = AtomicU64::new(0);

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
    pub avg_snapshot_changed_nodes: u64,
    pub max_snapshot_changed_nodes: u64,
    pub avg_snapshot_materialized_nodes: u64,
    pub max_snapshot_materialized_nodes: u64,
    pub avg_snapshot_ranked_items: u64,
    pub avg_snapshot_text_bytes: u64,
    pub max_snapshot_text_bytes: u64,
    pub duplicate_hash_bytes: u64,
    pub avg_duplicate_hash_ms: u64,
    pub ui_frames: u64,
    pub ui_dropped_batches: u64,
    pub ui_dropped_snapshots: u64,
    pub avg_walker_recv_blocked_ms: u64,
    pub avg_aggregator_processing_ms: u64,
    pub avg_batch_flush_size: u64,
    pub cancelled_scans: u64,
    pub avg_cancelled_scan_latency_ms: u64,
    pub finished_scans: u64,
    pub avg_finished_payload_bytes: u64,
    pub avg_finished_node_count: u64,
    pub collection_period_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub process_id: u32,
    pub cpu_parallelism: usize,
    pub memory_rss_bytes: Option<u64>,
    pub timestamp_unix_ms: u128,
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
            name: "df.snapshot.changed_nodes.avg",
            unit: "count",
            period_ms: COLLECTION_PERIOD_MS,
        },
        MetricDescriptor {
            name: "df.snapshot.text_bytes.avg",
            unit: "bytes",
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

pub fn record_snapshot_view(
    changed_node_count: u64,
    materialized_nodes: u64,
    ranked_items: u64,
    text_bytes: u64,
) {
    SNAPSHOT_VIEWS_TOTAL.fetch_add(1, Ordering::Relaxed);
    SNAPSHOT_CHANGED_NODES_TOTAL.fetch_add(changed_node_count, Ordering::Relaxed);
    SNAPSHOT_MATERIALIZED_NODES_TOTAL.fetch_add(materialized_nodes, Ordering::Relaxed);
    SNAPSHOT_RANKED_ITEMS_TOTAL.fetch_add(ranked_items, Ordering::Relaxed);
    SNAPSHOT_TEXT_BYTES_TOTAL.fetch_add(text_bytes, Ordering::Relaxed);
    update_max(&SNAPSHOT_MAX_CHANGED_NODES, changed_node_count);
    update_max(&SNAPSHOT_MAX_MATERIALIZED_NODES, materialized_nodes);
    update_max(&SNAPSHOT_MAX_TEXT_BYTES, text_bytes);
    tracing::debug!(
        event = "snapshot.view",
        metric = "df.snapshot.changed_nodes",
        changed_node_count,
        materialized_nodes,
        ranked_items,
        text_bytes
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

pub fn record_ui_frame() {
    UI_FRAMES_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn record_ui_backpressure(dropped_batches: u64, dropped_snapshots: u64) {
    UI_DROPPED_BATCH_TOTAL.fetch_add(dropped_batches, Ordering::Relaxed);
    UI_DROPPED_SNAPSHOT_TOTAL.fetch_add(dropped_snapshots, Ordering::Relaxed);
    if dropped_batches > 0 || dropped_snapshots > 0 {
        tracing::warn!(
            event = "ui.backpressure",
            dropped_batches,
            dropped_snapshots,
            "ui queue pressure detected"
        );
    }
}

pub fn record_walker_recv_blocked(elapsed_ms: u64) {
    WALKER_RECV_BLOCKED_MS_TOTAL.fetch_add(elapsed_ms, Ordering::Relaxed);
    WALKER_RECV_BLOCKED_SAMPLES.fetch_add(1, Ordering::Relaxed);
}

pub fn record_aggregator_processing(elapsed_ms: u64) {
    AGGREGATOR_PROCESSING_MS_TOTAL.fetch_add(elapsed_ms, Ordering::Relaxed);
    AGGREGATOR_PROCESSING_SAMPLES.fetch_add(1, Ordering::Relaxed);
}

pub fn record_batch_flush_size(size: usize) {
    BATCH_FLUSH_ITEMS_TOTAL.fetch_add(size as u64, Ordering::Relaxed);
    BATCH_FLUSH_EVENTS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn record_cancelled_scan_latency(elapsed_ms: u64) {
    CANCELLED_SCAN_TOTAL.fetch_add(1, Ordering::Relaxed);
    CANCELLED_SCAN_LATENCY_MS_TOTAL.fetch_add(elapsed_ms, Ordering::Relaxed);
}

pub fn record_scan_finished(node_count: u64, payload_bytes: u64) {
    FINISHED_SCAN_TOTAL.fetch_add(1, Ordering::Relaxed);
    FINISHED_NODE_COUNT_TOTAL.fetch_add(node_count, Ordering::Relaxed);
    FINISHED_PAYLOAD_BYTES_TOTAL.fetch_add(payload_bytes, Ordering::Relaxed);
}

fn update_max(target: &AtomicU64, candidate: u64) {
    let mut current = target.load(Ordering::Relaxed);
    while candidate > current {
        match target.compare_exchange_weak(current, candidate, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
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
    let snapshot_views = SNAPSHOT_VIEWS_TOTAL.load(Ordering::Relaxed);
    let walker_recv_samples = WALKER_RECV_BLOCKED_SAMPLES.load(Ordering::Relaxed);
    let aggregator_samples = AGGREGATOR_PROCESSING_SAMPLES.load(Ordering::Relaxed);
    let batch_flushes = BATCH_FLUSH_EVENTS_TOTAL.load(Ordering::Relaxed);
    let cancelled = CANCELLED_SCAN_TOTAL.load(Ordering::Relaxed);
    let finished = FINISHED_SCAN_TOTAL.load(Ordering::Relaxed);

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
        avg_snapshot_changed_nodes: if snapshot_views == 0 {
            0
        } else {
            SNAPSHOT_CHANGED_NODES_TOTAL.load(Ordering::Relaxed) / snapshot_views
        },
        max_snapshot_changed_nodes: SNAPSHOT_MAX_CHANGED_NODES.load(Ordering::Relaxed),
        avg_snapshot_materialized_nodes: if snapshot_views == 0 {
            0
        } else {
            SNAPSHOT_MATERIALIZED_NODES_TOTAL.load(Ordering::Relaxed) / snapshot_views
        },
        max_snapshot_materialized_nodes: SNAPSHOT_MAX_MATERIALIZED_NODES.load(Ordering::Relaxed),
        avg_snapshot_ranked_items: if snapshot_views == 0 {
            0
        } else {
            SNAPSHOT_RANKED_ITEMS_TOTAL.load(Ordering::Relaxed) / snapshot_views
        },
        avg_snapshot_text_bytes: if snapshot_views == 0 {
            0
        } else {
            SNAPSHOT_TEXT_BYTES_TOTAL.load(Ordering::Relaxed) / snapshot_views
        },
        max_snapshot_text_bytes: SNAPSHOT_MAX_TEXT_BYTES.load(Ordering::Relaxed),
        duplicate_hash_bytes: DUP_HASH_BYTES_TOTAL.load(Ordering::Relaxed),
        avg_duplicate_hash_ms: {
            let hash_calls = DUP_HASH_CALLS_TOTAL.load(Ordering::Relaxed);
            if hash_calls == 0 {
                0
            } else {
                DUP_HASH_ELAPSED_MS_TOTAL.load(Ordering::Relaxed) / hash_calls
            }
        },
        ui_frames: UI_FRAMES_TOTAL.load(Ordering::Relaxed),
        ui_dropped_batches: UI_DROPPED_BATCH_TOTAL.load(Ordering::Relaxed),
        ui_dropped_snapshots: UI_DROPPED_SNAPSHOT_TOTAL.load(Ordering::Relaxed),
        avg_walker_recv_blocked_ms: if walker_recv_samples == 0 {
            0
        } else {
            WALKER_RECV_BLOCKED_MS_TOTAL.load(Ordering::Relaxed) / walker_recv_samples
        },
        avg_aggregator_processing_ms: if aggregator_samples == 0 {
            0
        } else {
            AGGREGATOR_PROCESSING_MS_TOTAL.load(Ordering::Relaxed) / aggregator_samples
        },
        avg_batch_flush_size: if batch_flushes == 0 {
            0
        } else {
            BATCH_FLUSH_ITEMS_TOTAL.load(Ordering::Relaxed) / batch_flushes
        },
        cancelled_scans: cancelled,
        avg_cancelled_scan_latency_ms: if cancelled == 0 {
            0
        } else {
            CANCELLED_SCAN_LATENCY_MS_TOTAL.load(Ordering::Relaxed) / cancelled
        },
        finished_scans: finished,
        avg_finished_payload_bytes: if finished == 0 {
            0
        } else {
            FINISHED_PAYLOAD_BYTES_TOTAL.load(Ordering::Relaxed) / finished
        },
        avg_finished_node_count: if finished == 0 {
            0
        } else {
            FINISHED_NODE_COUNT_TOTAL.load(Ordering::Relaxed) / finished
        },
        collection_period_ms: COLLECTION_PERIOD_MS,
    }
}

pub fn system_snapshot() -> SystemSnapshot {
    SystemSnapshot {
        process_id: std::process::id(),
        cpu_parallelism: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        memory_rss_bytes: read_memory_rss_bytes(),
        timestamp_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default(),
    }
}

#[cfg(target_os = "linux")]
fn read_memory_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kb = line
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<u64>().ok())?;
    Some(kb * 1024)
}

#[cfg(not(target_os = "linux"))]
fn read_memory_rss_bytes() -> Option<u64> {
    None
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
        record_snapshot_view(32, 0, 20, 2048);
        record_duplicate_hash(1024, 2);
        record_action_audit("{}".into());
        record_ui_frame();
        record_ui_backpressure(1, 2);
        record_walker_recv_blocked(3);
        record_aggregator_processing(4);
        record_batch_flush_size(12);
        record_cancelled_scan_latency(20);
        record_scan_finished(100, 2048);
        let s = snapshot();
        assert!(s.scan_items >= 1);
        assert!(s.snapshots >= 1);
        assert!(s.scan_errors >= 1);
        assert!(s.action_attempted >= 1);
        assert!(s.action_failed >= 1);
        assert!(s.scan_batches >= 1);
        assert!(s.avg_snapshot_changed_nodes >= 1);
        assert!(s.max_snapshot_changed_nodes >= 1);
        assert!(s.avg_snapshot_ranked_items >= 1);
        assert!(s.avg_snapshot_text_bytes >= 1);
        assert!(s.duplicate_hash_bytes >= 1024);
        assert!(s.ui_frames >= 1);
        assert!(s.ui_dropped_batches >= 1);
        assert!(s.ui_dropped_snapshots >= 2);
        assert!(s.avg_walker_recv_blocked_ms >= 1);
        assert!(s.avg_aggregator_processing_ms >= 1);
        assert!(s.avg_batch_flush_size >= 1);
        assert!(s.cancelled_scans >= 1);
        assert!(s.avg_cancelled_scan_latency_ms >= 1);
        assert!(s.finished_scans >= 1);
        assert!(s.avg_finished_payload_bytes >= 1);
        assert!(s.avg_finished_node_count >= 1);
        assert_eq!(s.collection_period_ms, COLLECTION_PERIOD_MS);
        assert!(!action_audit_tail(1).is_empty());
    }

    #[test]
    fn system_snapshot_smoke() {
        let s = system_snapshot();
        assert!(s.process_id > 0);
        assert!(s.cpu_parallelism >= 1);
    }
}
