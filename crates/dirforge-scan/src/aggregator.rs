use crate::walker::EntryEvent;
use crate::BatchEntry;
use dirforge_core::{NodeId, NodeKind, NodeStore, ScanErrorRecord, ScanSummary, SnapshotDelta};

#[derive(Debug, Clone)]
pub struct Aggregator {
    pub store: NodeStore,
    pub summary: ScanSummary,
    pub errors: Vec<ScanErrorRecord>,
    changed_since_snapshot: Vec<NodeId>,
}

impl Aggregator {
    pub fn new(root_name: String, root_path: String) -> Self {
        let mut store = NodeStore::default();
        store.add_node(None, root_name, root_path, NodeKind::Dir, 0);
        Self {
            store,
            summary: ScanSummary::default(),
            errors: Vec::new(),
            changed_since_snapshot: Vec::new(),
        }
    }

    pub fn on_error(&mut self, error: ScanErrorRecord) {
        self.summary.error_count += 1;
        self.errors.push(error);
    }

    pub fn on_entry(&mut self, event: EntryEvent) -> BatchEntry {
        let parent = self.store.path_index.get(&event.parent_path).copied();
        let kind = if event.is_dir {
            self.summary.scanned_dirs += 1;
            NodeKind::Dir
        } else {
            self.summary.scanned_files += 1;
            self.summary.bytes_observed += event.size;
            NodeKind::File
        };

        let node_id = self
            .store
            .add_node(parent, event.name, event.path.clone(), kind, event.size);
        self.changed_since_snapshot.push(node_id);

        BatchEntry {
            path: event.path,
            is_dir: event.is_dir,
            size: event.size,
        }
    }

    pub fn make_snapshot_data(
        &mut self,
    ) -> (SnapshotDelta, Vec<(String, u64)>, Vec<(String, u64)>) {
        let top_files = self
            .store
            .top_n_largest_files(10)
            .into_iter()
            .map(|n| (n.path.clone(), n.size_self))
            .collect::<Vec<_>>();
        let top_dirs = self
            .store
            .largest_dirs(10)
            .into_iter()
            .map(|n| (n.path.clone(), n.size_subtree))
            .collect::<Vec<_>>();

        let top_files_delta = top_files
            .iter()
            .filter_map(|(path, _)| self.store.path_index.get(path).copied())
            .collect();
        let top_dirs_delta = top_dirs
            .iter()
            .filter_map(|(path, _)| self.store.path_index.get(path).copied())
            .collect();

        let delta = SnapshotDelta {
            changed_nodes: std::mem::take(&mut self.changed_since_snapshot),
            summary: self.summary.clone(),
            top_files_delta,
            top_dirs_delta,
        };
        (delta, top_files, top_dirs)
    }

    pub fn finalize(
        mut self,
    ) -> (
        NodeStore,
        ScanSummary,
        Vec<ScanErrorRecord>,
        SnapshotDelta,
        Vec<(String, u64)>,
        Vec<(String, u64)>,
    ) {
        self.store.rollup();
        let (delta, top_files, top_dirs) = self.make_snapshot_data();
        (
            self.store,
            self.summary,
            self.errors,
            delta,
            top_files,
            top_dirs,
        )
    }
}
