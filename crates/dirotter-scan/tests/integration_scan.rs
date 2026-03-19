use dirotter_core::ScanProfile;
use dirotter_scan::{start_scan, ScanConfig, ScanEvent, ScanMode};

#[test]
fn scan_fixture_tree() {
    let fixture = dirotter_testkit::FixtureTree::sample().expect("fixture");
    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Ssd,
            batch_size: 4,
            snapshot_ms: 50,
            metadata_parallelism: 4,
            deep_tasks_throttle: 64,
        },
    );

    let mut finished = false;
    let mut saw_batch = false;
    let mut snapshot_nodes = 0usize;
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
                ScanEvent::Snapshot { view, .. } => {
                    snapshot_nodes += view.nodes.len();
                }
                ScanEvent::Finished { summary, .. } => {
                    assert!(summary.scanned_files >= 2);
                    finished = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(saw_batch);
    assert!(
        snapshot_nodes > 0,
        "snapshot view should carry incremental nodes"
    );
    assert!(finished, "scan should finish in time");
}

#[test]
fn scan_can_cancel() {
    let fixture = dirotter_testkit::FixtureTree::sample().expect("fixture");
    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Network,
            batch_size: 1,
            snapshot_ms: 50,
            metadata_parallelism: 4,
            deep_tasks_throttle: 64,
        },
    );

    handle.cancel();
    let mut saw_finished = false;
    for _ in 0..200 {
        if let Ok(ScanEvent::Finished { .. }) = handle
            .events
            .recv_timeout(std::time::Duration::from_millis(20))
        {
            saw_finished = true;
            break;
        }
    }

    assert!(saw_finished, "cancelled scan should still produce Finished");
}

#[test]
fn scan_reports_errors_on_restricted_dir() {
    let fixture = dirotter_testkit::FixtureTree::restricted_dir().expect("fixture");
    let handle = start_scan(fixture.root.clone(), ScanConfig::default());
    #[cfg(unix)]
    let mut got_errors = false;

    for _ in 0..2000 {
        if let Ok(ScanEvent::Finished {
            summary, errors, ..
        }) = handle
            .events
            .recv_timeout(std::time::Duration::from_millis(10))
        {
            #[cfg(unix)]
            {
                got_errors = summary.error_count > 0 || !errors.is_empty();
            }
            #[cfg(not(unix))]
            {
                let _ = (summary, errors);
            }
            break;
        }
    }

    #[cfg(unix)]
    {
        if nix::unistd::Uid::effective().is_root() {
            assert!(true, "root may bypass restricted permissions");
        } else {
            assert!(
                got_errors,
                "expected permission errors on restricted fixture"
            );
        }
    }
}

#[test]
fn scan_skips_following_symlink_loops() {
    let fixture = dirotter_testkit::FixtureTree::with_symlink().expect("fixture");
    let handle = start_scan(fixture.root.clone(), ScanConfig::default());

    let mut done = false;
    for _ in 0..2000 {
        if let Ok(ScanEvent::Finished { summary, .. }) = handle
            .events
            .recv_timeout(std::time::Duration::from_millis(10))
        {
            assert!(summary.scanned_files >= 1);
            done = true;
            break;
        }
    }

    assert!(done, "scan with symlink should finish");
}

#[test]
fn scan_deep_and_wide_fixtures() {
    let deep = dirotter_testkit::FixtureTree::deep_tree(30).expect("deep");
    let wide = dirotter_testkit::FixtureTree::wide_tree(300).expect("wide");

    for fixture in [deep, wide] {
        let handle = start_scan(fixture.root.clone(), ScanConfig::default());
        let mut done = false;
        for _ in 0..2500 {
            if let Ok(ScanEvent::Finished { summary, .. }) = handle
                .events
                .recv_timeout(std::time::Duration::from_millis(10))
            {
                assert!(summary.scanned_files > 0);
                done = true;
                break;
            }
        }
        assert!(done, "scan should finish for fixture");
    }
}

#[test]
fn scan_modes_finish_sample_fixture() {
    let fixture = dirotter_testkit::FixtureTree::sample().expect("fixture");

    for mode in [ScanMode::Quick, ScanMode::Deep, ScanMode::LargeDisk] {
        let handle = start_scan(fixture.root.clone(), ScanConfig::for_mode(mode));
        let mut done = false;

        for _ in 0..2500 {
            if let Ok(ScanEvent::Finished { summary, .. }) = handle
                .events
                .recv_timeout(std::time::Duration::from_millis(10))
            {
                assert!(summary.scanned_files >= 2);
                done = true;
                break;
            }
        }

        assert!(
            done,
            "scan mode {mode:?} should finish on the sample fixture"
        );
    }
}

#[test]
fn scan_finishes_when_directory_backlog_exceeds_throttle() {
    let fixture = dirotter_testkit::FixtureTree::massive_tree(2, 5).expect("massive");
    let handle = start_scan(
        fixture.root.clone(),
        ScanConfig {
            profile: ScanProfile::Ssd,
            batch_size: 8,
            snapshot_ms: 50,
            metadata_parallelism: 4,
            deep_tasks_throttle: 4,
        },
    );

    let mut done = false;
    for _ in 0..3000 {
        if let Ok(ScanEvent::Finished { summary, .. }) = handle
            .events
            .recv_timeout(std::time::Duration::from_millis(10))
        {
            assert!(summary.scanned_dirs >= 10);
            assert!(summary.scanned_files >= 10);
            done = true;
            break;
        }
    }

    assert!(
        done,
        "scan should finish even when discovered directories exceed the throttle budget"
    );
}
