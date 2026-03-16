use dirforge_core::{NodeKind, NodeStore};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub size: u64,
    pub members: Vec<String>,
    pub reclaimable_bytes: u64,
}

pub fn detect_duplicates(store: &NodeStore) -> Vec<DuplicateGroup> {
    let mut by_size: HashMap<u64, Vec<String>> = HashMap::new();
    for n in &store.nodes {
        if matches!(n.kind, NodeKind::File) && n.size_self > 0 {
            by_size.entry(n.size_self).or_default().push(n.path.clone());
        }
    }
    by_size
        .into_iter()
        .filter_map(|(size, members)| {
            if members.len() < 2 {
                None
            } else {
                Some(DuplicateGroup {
                    size,
                    reclaimable_bytes: size
                        .saturating_mul((members.len() as u64).saturating_sub(1)),
                    members,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dirforge_core::{NodeKind, NodeStore};

    #[test]
    fn duplicate_by_size() {
        let mut s = NodeStore::default();
        let root = s.add_node(None, "r".into(), "/r".into(), NodeKind::Dir, 0);
        s.add_node(Some(root), "a".into(), "/r/a".into(), NodeKind::File, 7);
        s.add_node(Some(root), "b".into(), "/r/b".into(), NodeKind::File, 7);
        let d = detect_duplicates(&s);
        assert_eq!(d.len(), 1);
    }
}
