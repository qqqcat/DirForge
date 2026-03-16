use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub name: String,
    pub path: String,
    pub kind: NodeKind,
    pub size_self: u64,
    pub size_subtree: u64,
    pub file_count: u64,
    pub dir_count: u64,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NodeStore {
    pub nodes: Vec<Node>,
    pub children: HashMap<NodeId, Vec<NodeId>>,
    pub path_index: HashMap<String, NodeId>,
}

impl NodeStore {
    pub fn add_node(
        &mut self,
        parent: Option<NodeId>,
        name: String,
        path: String,
        kind: NodeKind,
        size_self: u64,
    ) -> NodeId {
        if let Some(id) = self.path_index.get(&path) {
            return *id;
        }
        let id = NodeId(self.nodes.len());
        let node = Node {
            id,
            parent,
            name,
            path: path.clone(),
            kind,
            size_self,
            size_subtree: size_self,
            file_count: u64::from(matches!(kind, NodeKind::File)),
            dir_count: u64::from(matches!(kind, NodeKind::Dir)),
        };
        self.nodes.push(node);
        self.path_index.insert(path, id);
        if let Some(pid) = parent {
            self.children.entry(pid).or_default().push(id);
        }
        id
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
    }

    pub fn top_n_largest_files(&self, n: usize) -> Vec<&Node> {
        let mut heap: BinaryHeap<(u64, usize)> = BinaryHeap::new();
        for node in &self.nodes {
            if matches!(node.kind, NodeKind::File) {
                heap.push((node.size_self, node.id.0));
            }
        }
        (0..n)
            .filter_map(|_| heap.pop())
            .map(|(_, idx)| &self.nodes[idx])
            .collect()
    }

    pub fn largest_dirs(&self, n: usize) -> Vec<&Node> {
        let mut heap: BinaryHeap<(u64, usize)> = BinaryHeap::new();
        for node in &self.nodes {
            if matches!(node.kind, NodeKind::Dir) {
                heap.push((node.size_subtree, node.id.0));
            }
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanErrorRecord {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanProfile {
    Ssd,
    Hdd,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDelta {
    pub changed_nodes: usize,
    pub scanned_files: u64,
    pub scanned_dirs: u64,
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

    #[test]
    fn rollup_works() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "root".into(), "/root".into(), NodeKind::Dir, 0);
        s.add_node(Some(root), "a".into(), "/root/a".into(), NodeKind::File, 4);
        s.add_node(Some(root), "b".into(), "/root/b".into(), NodeKind::File, 6);
        s.rollup();
        assert_eq!(s.nodes[root.0].size_subtree, 10);
        assert_eq!(s.nodes[root.0].file_count, 2);
    }
}
