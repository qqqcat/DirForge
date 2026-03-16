use dirforge_scan::{start_scan, ScanEvent};

#[test]
fn scan_fixture_tree() {
    let fixture = dirforge_testkit::FixtureTree::sample().expect("fixture");
    let handle = start_scan(fixture.root.clone());

    let mut finished = false;
    for _ in 0..2000 {
        if let Ok(event) = handle
            .events
            .recv_timeout(std::time::Duration::from_millis(10))
        {
            if let ScanEvent::Finished { store, summary, .. } = event {
                assert!(summary.scanned_files >= 2);
                assert!(!store.nodes.is_empty());
                finished = true;
                break;
            }
        }
    }

    assert!(finished, "scan should finish in time");
}
