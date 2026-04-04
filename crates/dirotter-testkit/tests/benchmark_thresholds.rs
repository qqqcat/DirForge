use dirotter_core::{NodeKind, NodeStore, ScanProfile};
use dirotter_dup::{detect_duplicates, DupConfig};
use dirotter_scan::{start_scan, ScanConfig, ScanEvent};
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct PerfBaseline {
    scan_small_tree_ms: u128,
    scan_massive_tree_ms: u128,
    dup_small_dataset_ms: u128,
    snapshot_massive_tree_payload_bytes: usize,
}

fn load_baseline() -> PerfBaseline {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/perf/baseline.json");
    let json = std::fs::read_to_string(path).expect("read baseline");
    serde_json::from_str(&json).expect("parse baseline")
}

#[test]
fn benchmark_scan_threshold_small_tree() {
    let baseline = load_baseline();
    let fixture = dirotter_testkit::FixtureTree::sample().expect("fixture");
    let start = Instant::now();

    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Ssd,
            batch_size: 128,
            snapshot_ms: 75,
            metadata_parallelism: 4,
            deep_tasks_throttle: 64,
        },
    );

    loop {
        let event = handle
            .events
            .recv_timeout(Duration::from_millis(25))
            .expect("event");
        if let ScanEvent::Finished { .. } = event {
            break;
        }
    }

    let elapsed = start.elapsed().as_millis();
    assert!(
        elapsed <= baseline.scan_small_tree_ms,
        "scan perf regression: {}ms > {}ms",
        elapsed,
        baseline.scan_small_tree_ms
    );
}

#[test]
fn benchmark_scan_threshold_massive_tree() {
    let baseline = load_baseline();
    let fixture = dirotter_testkit::FixtureTree::massive_tree(3, 8).expect("fixture");
    let start = Instant::now();

    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Ssd,
            batch_size: 256,
            snapshot_ms: 100,
            metadata_parallelism: 4,
            deep_tasks_throttle: 128,
        },
    );

    loop {
        let event = handle
            .events
            .recv_timeout(Duration::from_millis(50))
            .expect("event");
        if let ScanEvent::Finished { .. } = event {
            break;
        }
    }

    let elapsed = start.elapsed().as_millis();
    assert!(
        elapsed <= baseline.scan_massive_tree_ms,
        "massive scan perf regression: {}ms > {}ms",
        elapsed,
        baseline.scan_massive_tree_ms
    );
}

#[test]
fn benchmark_dup_threshold_small_dataset() {
    let baseline = load_baseline();
    let mut store = NodeStore::default();
    let root = store.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
    for i in 0..600 {
        let size = if i % 3 == 0 {
            2048
        } else {
            1024 + (i as u64 % 10)
        };
        store.add_node(
            Some(root),
            format!("f{i}"),
            format!("/root/f{i}"),
            NodeKind::File,
            size,
        );
    }

    let start = Instant::now();
    let _groups = detect_duplicates(&store, DupConfig::default());
    let elapsed = start.elapsed().as_millis();

    assert!(
        elapsed <= baseline.dup_small_dataset_ms,
        "dup perf regression: {}ms > {}ms",
        elapsed,
        baseline.dup_small_dataset_ms
    );
}

#[test]
fn benchmark_snapshot_payload_threshold_massive_tree() {
    let baseline = load_baseline();
    let fixture = dirotter_testkit::FixtureTree::massive_tree(3, 8).expect("fixture");
    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Ssd,
            batch_size: 128,
            snapshot_ms: 50,
            metadata_parallelism: 4,
            deep_tasks_throttle: 128,
        },
    );

    let mut max_payload_bytes = 0usize;
    let mut saw_snapshot = false;
    loop {
        let event = handle
            .events
            .recv_timeout(Duration::from_millis(25))
            .expect("event");
        match event {
            ScanEvent::Snapshot { view, .. } => {
                saw_snapshot = true;
                let payload = serde_json::to_vec(&view).expect("serialize snapshot view");
                max_payload_bytes = max_payload_bytes.max(payload.len());
            }
            ScanEvent::Finished { .. } => break,
            _ => {}
        }
    }

    assert!(saw_snapshot, "expected at least one snapshot event");
    assert!(
        max_payload_bytes <= baseline.snapshot_massive_tree_payload_bytes,
        "snapshot payload regression: {} bytes > {} bytes",
        max_payload_bytes,
        baseline.snapshot_massive_tree_payload_bytes
    );
}
