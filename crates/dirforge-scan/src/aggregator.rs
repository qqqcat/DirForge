use crate::walker::EntryEvent;
use crate::{BatchEntry, SelectionState, SnapshotView};
use dirforge_core::{NodeId, NodeKind, NodeStore, ScanErrorRecord, ScanSummary, SnapshotDelta};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Aggregator {
    pub store: NodeStore,
    pub summary: ScanSummary,
    pub errors: Vec<ScanErrorRecord>,
    changed_since_snapshot: Vec<NodeId>,
    root_path: String,
    pending_by_parent: HashMap<String, Vec<EntryEvent>>,
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
            root_path,
            pending_by_parent: HashMap::new(),
        }
    }

    pub fn on_error(&mut self, error: ScanErrorRecord) {
        self.summary.error_count += 1;
        self.errors.push(error);
    }

    pub fn on_entry(&mut self, event: EntryEvent) -> Vec<BatchEntry> {
        if self.store.path_index.contains_key(&event.path) {
            return Vec::new();
        }

        if !self.store.path_index.contains_key(&event.parent_path)
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
            if self.store.path_index.contains_key(&event.path) {
                continue;
            }

            let parent = self.store.path_index.get(&event.parent_path).copied();
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
                event.name.clone(),
                event.path.clone(),
                kind,
                event.size,
            );
            self.changed_since_snapshot.push(node_id);

            emitted.push(BatchEntry {
                path: event.path.clone(),
                is_dir: event.is_dir,
                size: event.size,
            });

            if let Some(children) = self.pending_by_parent.remove(&event.path) {
                queue.extend(children);
            }
        }

        emitted
    }

    pub fn make_snapshot_data(&mut self, include_full_tree: bool) -> (SnapshotDelta, SnapshotView) {
        self.store.rollup();

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

        let changed_nodes = std::mem::take(&mut self.changed_since_snapshot);
        let view_nodes = if include_full_tree {
            self.store.nodes.clone()
        } else {
            changed_nodes
                .iter()
                .filter_map(|id| self.store.nodes.get(id.0).cloned())
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
        Vec<ScanErrorRecord>,
        SnapshotDelta,
        SnapshotView,
    ) {
        let (delta, view) = self.make_snapshot_data(true);
        (self.summary, self.errors, delta, view)
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
            path: format!("{root}/a/b.txt"),
            parent_path: format!("{root}/a"),
            name: "b.txt".to_string(),
            is_dir: false,
            size: 7,
            metadata_backlog: 0,
            recv_blocked_ms: 0,
        });
        assert!(child.is_empty());

        let parent_batch = aggr.on_entry(EntryEvent {
            path: format!("{root}/a"),
            parent_path: root.clone(),
            name: "a".to_string(),
            is_dir: true,
            size: 0,
            metadata_backlog: 0,
            recv_blocked_ms: 0,
        });

        assert_eq!(parent_batch.len(), 2);
        assert_eq!(aggr.summary.scanned_dirs, 1);
        assert_eq!(aggr.summary.scanned_files, 1);
        assert_eq!(aggr.summary.bytes_observed, 7);
        assert!(aggr
            .store
            .path_index
            .contains_key(&format!("{root}/a/b.txt")));
    }
}
