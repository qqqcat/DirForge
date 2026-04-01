use rayon::prelude::*;
use serde::{Deserialize, Serialize};
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
    pub name: String,
    pub path: String,
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
            name: self.node_name(node).to_string(),
            path: self.node_path(node).to_string(),
            kind: node.kind,
            size_self: node.size_self,
            size_subtree: node.size_subtree,
            file_count: node.file_count,
            dir_count: node.dir_count,
            dirty: node.dirty,
        }
    }

    pub fn mark_dirty(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.get_mut(id.0) {
            node.dirty = true;
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
            self.mark_dirty(pid);
        }
        id
    }

    pub fn upsert_resolved_node(&mut self, node: ResolvedNode) {
        let name_id = self.intern(&node.name);
        let path: Arc<str> = Arc::from(node.path);
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
        for idx in (0..self.nodes.len()).rev() {
            let id = NodeId(idx);
            if let Some(kids) = self.children.get(&id) {
                let mut subtree = self.nodes[idx].size_self;
                let mut files = u64::from(matches!(self.nodes[idx].kind, NodeKind::File));
                let mut dirs = u64::from(matches!(self.nodes[idx].kind, NodeKind::Dir));
                for kid in kids {
                    let n = &self.nodes[kid.0];
                    subtree += n.size_subtree;
                    files += n.file_count;
                    dirs += n.dir_count;
                }
                self.nodes[idx].size_subtree = subtree;
                self.nodes[idx].file_count = files;
                self.nodes[idx].dir_count = dirs;
            }
        }
        self.clear_dirty();
    }

    pub fn top_n_largest_files(&self, n: usize) -> Vec<&Node> {
        let mut heap: BinaryHeap<(u64, usize)> = self
            .nodes
            .par_iter()
            .filter(|node| matches!(node.kind, NodeKind::File))
            .map(|node| (node.size_self, node.id.0))
            .collect();

        (0..n)
            .filter_map(|_| heap.pop())
            .map(|(_, idx)| &self.nodes[idx])
            .collect()
    }

    pub fn largest_dirs(&self, n: usize) -> Vec<&Node> {
        let mut heap: BinaryHeap<(u64, usize)> = self
            .nodes
            .par_iter()
            .filter(|node| matches!(node.kind, NodeKind::Dir))
            .map(|node| (node.size_subtree, node.id.0))
            .collect();

        (0..n)
            .filter_map(|_| heap.pop())
            .map(|(_, idx)| &self.nodes[idx])
            .collect()
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
