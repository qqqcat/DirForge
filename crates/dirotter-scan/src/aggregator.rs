use crate::walker::EntryEvent;
use crate::{BatchEntry, SelectionState, SnapshotView};
use dirotter_core::{NodeId, NodeKind, NodeStore, ScanErrorRecord, ScanSummary, SnapshotDelta};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Aggregator {
    pub store: NodeStore,
    pub summary: ScanSummary,
    pub errors: Vec<ScanErrorRecord>,
    changed_since_snapshot: Vec<NodeId>,
    root_path: Arc<str>,
    pending_by_parent: HashMap<Arc<str>, Vec<EntryEvent>>,
}

impl Aggregator {
    pub fn new(root_name: String, root_path: String) -> Self {
        let mut store = NodeStore::default();
        store.add_node(None, root_name, root_path.clone(), NodeKind::Dir, 0);
        Self {
            store,
            summary: ScanSummary::default(),
            errors: Vec::new(),
            changed_since_snapshot: Vec::new(),
            root_path: Arc::from(root_path),
            pending_by_parent: HashMap::new(),
        }
    }

    pub fn on_error(&mut self, error: ScanErrorRecord) {
        self.summary.error_count += 1;
        self.errors.push(error);
    }

    pub fn on_entry(&mut self, event: EntryEvent) -> Vec<BatchEntry> {
        if self.store.path_index.contains_key(event.path.as_ref()) {
            return Vec::new();
        }

        if !self
            .store
            .path_index
            .contains_key(event.parent_path.as_ref())
            && event.parent_path != self.root_path
        {
            self.pending_by_parent
                .entry(event.parent_path.clone())
                .or_default()
                .push(event);
            return Vec::new();
        }

        let mut emitted = Vec::new();
        let mut queue = vec![event];
        while let Some(event) = queue.pop() {
            if self.store.path_index.contains_key(event.path.as_ref()) {
                continue;
            }

            let parent = self
                .store
                .path_index
                .get(event.parent_path.as_ref())
                .copied();
            let kind = if event.is_dir {
                self.summary.scanned_dirs += 1;
                NodeKind::Dir
            } else {
                self.summary.scanned_files += 1;
                self.summary.bytes_observed += event.size;
                NodeKind::File
            };

            let node_id = self.store.add_node(
                parent,
                event.name.as_ref().to_owned(),
                event.path.as_ref().to_owned(),
                kind,
                event.size,
            );
            self.changed_since_snapshot.push(node_id);

            emitted.push(BatchEntry {
                path: event.path.clone(),
                is_dir: event.is_dir,
                size: event.size,
            });

            if let Some(children) = self.pending_by_parent.remove(event.path.as_ref()) {
                queue.extend(children);
            }
        }

        emitted
    }

    pub fn make_snapshot_data(&mut self, include_full_tree: bool) -> (SnapshotDelta, SnapshotView) {
        // The scan path is append-only, so ancestor aggregates stay current as nodes arrive.
        // Snapshots only need to clear the "changed since last snapshot" markers.
        self.store.clear_dirty();

        let top_file_nodes = self.store.top_n_largest_files(10);
        let top_dir_nodes = self.store.largest_dirs(10);
        let top_files = top_file_nodes
            .iter()
            .map(|node| (node.path.clone(), node.size_self))
            .collect::<Vec<_>>();
        let top_dirs = top_dir_nodes
            .iter()
            .map(|node| (node.path.clone(), node.size_subtree))
            .collect::<Vec<_>>();
        let top_files_delta = top_file_nodes.iter().map(|node| node.id).collect();
        let top_dirs_delta = top_dir_nodes.iter().map(|node| node.id).collect();

        let changed_nodes = std::mem::take(&mut self.changed_since_snapshot);
        let view_nodes = if include_full_tree {
            self.store
                .nodes
                .iter()
                .map(|node| self.store.resolved_node(node))
                .collect()
        } else {
            changed_nodes
                .iter()
                .filter_map(|id| self.store.nodes.get(id.0))
                .map(|node| self.store.resolved_node(node))
                .collect()
        };

        let delta = SnapshotDelta {
            changed_nodes,
            summary: self.summary.clone(),
            top_files_delta,
            top_dirs_delta,
        };
        let view = SnapshotView {
            nodes: view_nodes,
            top_files,
            top_dirs,
            selection: SelectionState {
                focused: None,
                expanded: Vec::new(),
            },
        };
        (delta, view)
    }

    pub fn finalize(
        mut self,
    ) -> (
        ScanSummary,
        NodeStore,
        Vec<ScanErrorRecord>,
        SnapshotDelta,
        SnapshotView,
    ) {
        let (delta, view) = self.make_snapshot_data(false);
        (self.summary, self.store, self.errors, delta, view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_children_until_parent_arrives() {
        let root = "/tmp/root".to_string();
        let mut aggr = Aggregator::new("root".to_string(), root.clone());

        let child = aggr.on_entry(EntryEvent {
            path: format!("{root}/a/b.txt").into(),
            parent_path: format!("{root}/a").into(),
            name: "b.txt".into(),
            is_dir: false,
            size: 7,
            metadata_backlog: 0,
        });
        assert!(child.is_empty());

        let parent_batch = aggr.on_entry(EntryEvent {
            path: format!("{root}/a").into(),
            parent_path: root.clone().into(),
            name: "a".into(),
            is_dir: true,
            size: 0,
            metadata_backlog: 0,
        });

        assert_eq!(parent_batch.len(), 2);
        assert_eq!(aggr.summary.scanned_dirs, 1);
        assert_eq!(aggr.summary.scanned_files, 1);
        assert_eq!(aggr.summary.bytes_observed, 7);
        assert!(aggr
            .store
            .path_index
            .contains_key(format!("{root}/a/b.txt").as_str()));
    }

    #[test]
    fn snapshot_uses_incremental_aggregates_without_extra_rollup() {
        let root = "/tmp/root".to_string();
        let mut aggr = Aggregator::new("root".to_string(), root.clone());

        aggr.on_entry(EntryEvent {
            path: format!("{root}/a").into(),
            parent_path: root.clone().into(),
            name: "a".into(),
            is_dir: true,
            size: 0,
            metadata_backlog: 0,
        });
        aggr.on_entry(EntryEvent {
            path: format!("{root}/a/huge.bin").into(),
            parent_path: format!("{root}/a").into(),
            name: "huge.bin".into(),
            is_dir: false,
            size: 42,
            metadata_backlog: 0,
        });

        let (_, view) = aggr.make_snapshot_data(false);

        assert_eq!(view.top_files.first().map(|entry| entry.1), Some(42));
        assert_eq!(view.top_dirs.first().map(|entry| entry.1), Some(42));
        assert!(aggr.store.nodes.iter().all(|node| !node.dirty));
    }
}
