use dirforge_core::{NodeKind, NodeStore, ScanProfile};
use dirforge_dup::{detect_duplicates, DupConfig};
use dirforge_scan::{start_scan, ScanConfig, ScanEvent};
use std::time::{Duration, Instant};

const SCAN_THRESHOLD_MS: u128 = 4000;
const DUP_THRESHOLD_MS: u128 = 1200;

#[test]
fn benchmark_scan_threshold_small_tree() {
    let fixture = dirforge_testkit::FixtureTree::sample().expect("fixture");
    let start = Instant::now();

    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Ssd,
            batch_size: 128,
            snapshot_ms: 75,
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
        elapsed <= SCAN_THRESHOLD_MS,
        "scan perf regression: {}ms > {}ms",
        elapsed,
        SCAN_THRESHOLD_MS
    );
}

#[test]
fn benchmark_dup_threshold_small_dataset() {
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
        elapsed <= DUP_THRESHOLD_MS,
        "dup perf regression: {}ms > {}ms",
        elapsed,
        DUP_THRESHOLD_MS
    );
}
