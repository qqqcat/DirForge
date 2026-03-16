use dirforge_core::ScanProfile;
use dirforge_scan::{start_scan, ScanConfig, ScanEvent};

#[test]
fn scan_fixture_tree() {
    let fixture = dirforge_testkit::FixtureTree::sample().expect("fixture");
    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Ssd,
            batch_size: 4,
            snapshot_ms: 50,
        },
    );

    let mut finished = false;
    let mut saw_batch = false;
    for _ in 0..2000 {
        if let Ok(event) = handle
            .events
            .recv_timeout(std::time::Duration::from_millis(10))
        {
            match event {
                ScanEvent::Batch(b) => {
                    if !b.is_empty() {
                        saw_batch = true;
                    }
                }
                ScanEvent::Finished { store, summary, .. } => {
                    assert!(summary.scanned_files >= 2);
                    assert!(!store.nodes.is_empty());
                    finished = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(saw_batch);
    assert!(finished, "scan should finish in time");
}
