use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static SCAN_ITEMS: AtomicU64 = AtomicU64::new(0);
static SNAPSHOTS: AtomicU64 = AtomicU64::new(0);
static SCAN_ERRORS: AtomicU64 = AtomicU64::new(0);
static ACTION_ATTEMPTED: AtomicU64 = AtomicU64::new(0);
static ACTION_FAILED: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetrySnapshot {
    pub scan_items: u64,
    pub snapshots: u64,
    pub scan_errors: u64,
    pub action_attempted: u64,
    pub action_failed: u64,
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

pub fn record_snapshot() {
    SNAPSHOTS.fetch_add(1, Ordering::Relaxed);
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
    TelemetrySnapshot {
        scan_items: SCAN_ITEMS.load(Ordering::Relaxed),
        snapshots: SNAPSHOTS.load(Ordering::Relaxed),
        scan_errors: SCAN_ERRORS.load(Ordering::Relaxed),
        action_attempted: ACTION_ATTEMPTED.load(Ordering::Relaxed),
        action_failed: ACTION_FAILED.load(Ordering::Relaxed),
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
        let s = snapshot();
        assert!(s.scan_items >= 1);
        assert!(s.snapshots >= 1);
        assert!(s.scan_errors >= 1);
        assert!(s.action_attempted >= 1);
        assert!(s.action_failed >= 1);
    }
}
