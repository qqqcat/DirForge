use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StringId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub name_id: StringId,
    pub path: Arc<str>,
    pub kind: NodeKind,
    pub size_self: u64,
    pub size_subtree: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub name: Arc<str>,
    pub path: Arc<str>,
    pub kind: NodeKind,
    pub size_self: u64,
    pub size_subtree: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub dirty: bool,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NodeStore {
    pub nodes: Vec<Node>,
    pub children: HashMap<NodeId, Vec<NodeId>>,
    pub path_index: HashMap<Arc<str>, NodeId>,
    pub string_pool: Vec<Arc<str>>,
    pub string_index: HashMap<Arc<str>, StringId>,
}

impl NodeStore {
    fn intern(&mut self, value: &str) -> StringId {
        if let Some(id) = self.string_index.get(value) {
            return *id;
        }
        let id = StringId(self.string_pool.len());
        let owned: Arc<str> = Arc::from(value);
        self.string_pool.push(owned.clone());
        self.string_index.insert(owned, id);
        id
    }

    pub fn resolve_string(&self, id: StringId) -> Option<&str> {
        self.string_pool.get(id.0).map(|s| s.as_ref())
    }

    pub fn resolve_string_arc(&self, id: StringId) -> Option<Arc<str>> {
        self.string_pool.get(id.0).cloned()
    }

    pub fn node_name(&self, node: &Node) -> &str {
        self.resolve_string(node.name_id).unwrap_or("")
    }

    pub fn node_path<'a>(&self, node: &'a Node) -> &'a str {
        node.path.as_ref()
    }

    pub fn resolved_node(&self, node: &Node) -> ResolvedNode {
        ResolvedNode {
            id: node.id,
            parent: node.parent,
            name: self
                .resolve_string_arc(node.name_id)
                .unwrap_or_else(|| Arc::from("")),
            path: node.path.clone(),
            kind: node.kind,
            size_self: node.size_self,
            size_subtree: node.size_subtree,
            file_count: node.file_count,
            dir_count: node.dir_count,
            dirty: node.dirty,
        }
    }

    pub fn mark_dirty(&mut self, id: NodeId) {
        let mut current = Some(id);
        while let Some(node_id) = current {
            let Some(node) = self.nodes.get_mut(node_id.0) else {
                break;
            };
            let parent = node.parent;
            node.dirty = true;
            current = parent;
        }
    }

    pub fn clear_dirty(&mut self) {
        for node in &mut self.nodes {
            node.dirty = false;
        }
    }

    pub fn add_node(
        &mut self,
        parent: Option<NodeId>,
        name: String,
        path: String,
        kind: NodeKind,
        size_self: u64,
    ) -> NodeId {
        if let Some(id) = self.path_index.get(path.as_str()) {
            return *id;
        }
        let id = NodeId(self.nodes.len());
        let name_id = self.intern(&name);
        let path: Arc<str> = Arc::from(path);
        let node = Node {
            id,
            parent,
            name_id,
            path: path.clone(),
            kind,
            size_self,
            size_subtree: size_self,
            file_count: u64::from(matches!(kind, NodeKind::File)),
            dir_count: u64::from(matches!(kind, NodeKind::Dir)),
            dirty: true,
        };
        self.nodes.push(node);
        self.path_index.insert(path, id);
        if let Some(pid) = parent {
            self.children.entry(pid).or_default().push(id);
            self.propagate_addition(pid, kind, size_self);
            self.mark_dirty(pid);
        }
        id
    }

    pub fn upsert_resolved_node(&mut self, node: ResolvedNode) {
        let name_id = self.intern(node.name.as_ref());
        let path = node.path.clone();
        let compact = Node {
            id: node.id,
            parent: node.parent,
            name_id,
            path: path.clone(),
            kind: node.kind,
            size_self: node.size_self,
            size_subtree: node.size_subtree,
            file_count: node.file_count,
            dir_count: node.dir_count,
            dirty: node.dirty,
        };
        if compact.id.0 >= self.nodes.len() {
            self.nodes.push(compact.clone());
        } else {
            self.nodes[compact.id.0] = compact.clone();
        }
        self.path_index.insert(path, compact.id);
        if let Some(parent) = compact.parent {
            let children = self.children.entry(parent).or_default();
            if !children.contains(&compact.id) {
                children.push(compact.id);
            }
        }
    }

    pub fn rollup(&mut self) {
        let dirty_nodes: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, node)| node.dirty.then_some(idx))
            .collect();

        for idx in dirty_nodes.into_iter().rev() {
            let id = NodeId(idx);
            let (subtree, files, dirs) = {
                let node = &self.nodes[idx];
                let mut subtree = node.size_self;
                let mut files = u64::from(matches!(node.kind, NodeKind::File));
                let mut dirs = u64::from(matches!(node.kind, NodeKind::Dir));

                if let Some(kids) = self.children.get(&id) {
                    for kid in kids {
                        let child = &self.nodes[kid.0];
                        subtree += child.size_subtree;
                        files += child.file_count;
                        dirs += child.dir_count;
                    }
                }

                (subtree, files, dirs)
            };

            let node = &mut self.nodes[idx];
            node.size_subtree = subtree;
            node.file_count = files;
            node.dir_count = dirs;
            node.dirty = false;
        }
    }

    pub fn top_n_largest_files(&self, n: usize) -> Vec<&Node> {
        self.top_n_nodes_by(n, |node| {
            matches!(node.kind, NodeKind::File).then_some(node.size_self)
        })
    }

    pub fn largest_dirs(&self, n: usize) -> Vec<&Node> {
        self.top_n_nodes_by(n, |node| {
            matches!(node.kind, NodeKind::Dir).then_some(node.size_subtree)
        })
    }

    fn top_n_nodes_by<F>(&self, n: usize, mut score: F) -> Vec<&Node>
    where
        F: FnMut(&Node) -> Option<u64>,
    {
        if n == 0 {
            return Vec::new();
        }

        let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::with_capacity(n);
        for node in &self.nodes {
            let Some(value) = score(node) else {
                continue;
            };
            let entry = (value, node.id.0);
            if heap.len() < n {
                heap.push(Reverse(entry));
                continue;
            }
            if heap.peek().is_some_and(|smallest| entry > smallest.0) {
                heap.pop();
                heap.push(Reverse(entry));
            }
        }

        let mut entries: Vec<_> = heap.into_iter().map(|entry| entry.0).collect();
        entries.sort_unstable_by(|left, right| right.cmp(left));
        entries
            .into_iter()
            .map(|(_, idx)| &self.nodes[idx])
            .collect()
    }

    fn propagate_addition(&mut self, parent: NodeId, kind: NodeKind, size_self: u64) {
        let file_delta = u64::from(matches!(kind, NodeKind::File));
        let dir_delta = u64::from(matches!(kind, NodeKind::Dir));
        let mut current = Some(parent);

        while let Some(node_id) = current {
            let Some(node) = self.nodes.get_mut(node_id.0) else {
                break;
            };
            node.size_subtree += size_self;
            node.file_count += file_delta;
            node.dir_count += dir_delta;
            current = node.parent;
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub scanned_files: u64,
    pub scanned_dirs: u64,
    pub bytes_observed: u64,
    pub error_count: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorKind {
    User,
    Transient,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanErrorRecord {
    pub path: String,
    pub reason: String,
    pub kind: ErrorKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanProfile {
    Ssd,
    Hdd,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDelta {
    pub changed_nodes: Vec<NodeId>,
    pub summary: ScanSummary,
    pub top_files_delta: Vec<NodeId>,
    pub top_dirs_delta: Vec<NodeId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn rollup_works() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        s.add_node(Some(root), "a".into(), "/root/a".into(), NodeKind::File, 4);
        s.add_node(Some(root), "b".into(), "/root/b".into(), NodeKind::File, 6);
        s.rollup();
        assert_eq!(s.nodes[root.0].size_subtree, 10);
        assert_eq!(s.nodes[root.0].file_count, 2);
        assert!(!s.nodes[root.0].dirty);
    }

    #[test]
    fn duplicate_path_returns_existing_node() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        let first = s.add_node(Some(root), "a".into(), "/root/a".into(), NodeKind::File, 1);
        let second = s.add_node(Some(root), "a".into(), "/root/a".into(), NodeKind::File, 99);
        assert_eq!(first, second);
        assert_eq!(s.nodes.len(), 2);
    }

    #[test]
    fn top_n_largest_files_respects_limit() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        s.add_node(Some(root), "a".into(), "/root/a".into(), NodeKind::File, 1);
        s.add_node(Some(root), "b".into(), "/root/b".into(), NodeKind::File, 20);
        s.add_node(Some(root), "c".into(), "/root/c".into(), NodeKind::File, 10);
        let top = s.top_n_largest_files(2);
        assert_eq!(top.len(), 2);
        assert!(top[0].size_self >= top[1].size_self);
    }

    #[test]
    fn mark_dirty_propagates_to_ancestors() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        let child = s.add_node(
            Some(root),
            "child".into(),
            "/root/child".into(),
            NodeKind::Dir,
            0,
        );
        s.rollup();

        s.mark_dirty(child);

        assert!(s.nodes[child.0].dirty);
        assert!(s.nodes[root.0].dirty);
    }

    #[test]
    fn rollup_updates_new_leaf_without_recomputing_everything() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        let child = s.add_node(
            Some(root),
            "child".into(),
            "/root/child".into(),
            NodeKind::Dir,
            0,
        );
        s.add_node(
            Some(child),
            "old.bin".into(),
            "/root/child/old.bin".into(),
            NodeKind::File,
            4,
        );
        s.rollup();

        s.add_node(
            Some(child),
            "new.bin".into(),
            "/root/child/new.bin".into(),
            NodeKind::File,
            6,
        );
        s.rollup();

        assert_eq!(s.nodes[child.0].size_subtree, 10);
        assert_eq!(s.nodes[root.0].size_subtree, 10);
        assert!(s.nodes.iter().all(|node| !node.dirty));
    }

    #[test]
    fn add_node_updates_ancestor_aggregates_immediately() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        let child = s.add_node(
            Some(root),
            "child".into(),
            "/root/child".into(),
            NodeKind::Dir,
            0,
        );

        s.add_node(
            Some(child),
            "new.bin".into(),
            "/root/child/new.bin".into(),
            NodeKind::File,
            6,
        );

        assert_eq!(s.nodes[child.0].size_subtree, 6);
        assert_eq!(s.nodes[root.0].size_subtree, 6);
        assert_eq!(s.nodes[child.0].file_count, 1);
        assert_eq!(s.nodes[root.0].file_count, 1);
    }

    #[test]
    fn string_pool_reuses_values() {
        let mut s = NodeStore::default();
        s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        s.add_node(None, "root".into(), "/root-2".into(), NodeKind::Dir, 0);
        assert_eq!(s.string_pool.len(), 1);
        let name_id = s.nodes[0].name_id;
        assert_eq!(s.resolve_string(name_id), Some("root"));
    }

    #[test]
    fn compact_node_layout_is_smaller_than_legacy_string_heavy_layout() {
        #[allow(dead_code)]
        struct LegacyLikeNode {
            id: NodeId,
            parent: Option<NodeId>,
            name: String,
            path: String,
            name_id: StringId,
            kind: NodeKind,
            size_self: u64,
            size_subtree: u64,
            file_count: u64,
            dir_count: u64,
            dirty: bool,
        }

        let compact = size_of::<Node>();
        let legacy = size_of::<LegacyLikeNode>();
        println!("compact_node_bytes={compact} legacy_like_node_bytes={legacy}");
        assert!(compact < legacy);
    }
}
