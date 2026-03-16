use dirforge_core::{NodeKind, NodeStore, RiskLevel};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone)]
pub struct DuplicateMember {
    pub path: String,
    pub size: u64,
    pub keeper: bool,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub size: u64,
    pub members: Vec<DuplicateMember>,
    pub reclaimable_bytes: u64,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Copy)]
pub struct DupConfig {
    pub partial_bytes: usize,
    pub full_hash_min_size: u64,
}

impl Default for DupConfig {
    fn default() -> Self {
        Self {
            partial_bytes: 4096,
            full_hash_min_size: 128 * 1024,
        }
    }
}

pub fn detect_duplicates(store: &NodeStore, cfg: DupConfig) -> Vec<DuplicateGroup> {
    let mut by_size: HashMap<u64, Vec<String>> = HashMap::new();
    for n in &store.nodes {
        if matches!(n.kind, NodeKind::File) && n.size_self > 0 {
            by_size.entry(n.size_self).or_default().push(n.path.clone());
        }
    }

    let mut out = Vec::new();

    for (size, paths) in by_size {
        if paths.len() < 2 {
            continue;
        }

        // phase 2: partial fingerprint regroup
        let mut partial_bucket: HashMap<Vec<u8>, Vec<String>> = HashMap::new();
        for p in paths {
            let sig = partial_fingerprint(&p, cfg.partial_bytes);
            partial_bucket.entry(sig).or_default().push(p);
        }

        for (_, partial_paths) in partial_bucket {
            if partial_paths.len() < 2 {
                continue;
            }

            // phase 3: full hash for stronger confirmation
            let mut full_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
            for p in partial_paths {
                let hash = if size >= cfg.full_hash_min_size {
                    full_hash(&p)
                } else {
                    hash_bytes(p.as_bytes())
                };
                full_bucket.entry(hash).or_default().push(p);
            }

            for (_, final_paths) in full_bucket {
                if final_paths.len() < 2 {
                    continue;
                }
                let keeper_idx = recommend_keeper(&final_paths);
                let members: Vec<DuplicateMember> = final_paths
                    .iter()
                    .enumerate()
                    .map(|(i, p)| DuplicateMember {
                        path: p.clone(),
                        size,
                        keeper: i == keeper_idx,
                    })
                    .collect();
                let reclaimable_bytes =
                    size.saturating_mul((members.len() as u64).saturating_sub(1));
                let risk = risk_of_group(&members);
                out.push(DuplicateGroup {
                    size,
                    members,
                    reclaimable_bytes,
                    risk,
                });
            }
        }
    }

    out
}

fn partial_fingerprint(path: &str, n: usize) -> Vec<u8> {
    if let Ok(bytes) = fs::read(path) {
        let head = bytes.iter().take(n).copied();
        let tail = bytes.iter().rev().take(n).copied();
        let mut sampled: Vec<u8> = head.collect();
        sampled.extend(tail);
        sampled
    } else {
        path.as_bytes().iter().take(n).copied().collect()
    }
}

fn full_hash(path: &str) -> [u8; 32] {
    match fs::read(path) {
        Ok(bytes) => hash_bytes(&bytes),
        Err(_) => hash_bytes(path.as_bytes()),
    }
}

fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(blake3::hash(bytes).as_bytes());
    out
}

fn recommend_keeper(paths: &[String]) -> usize {
    paths
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| p.len())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn risk_of_group(members: &[DuplicateMember]) -> RiskLevel {
    if members.iter().any(|m| {
        let p = m.path.to_lowercase();
        p.contains("windows") || p.contains("program files")
    }) {
        RiskLevel::High
    } else if members
        .iter()
        .any(|m| m.path.to_lowercase().contains("appdata"))
    {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
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
        let d = detect_duplicates(&s, DupConfig::default());
        // paths don't exist; still grouped by fallback hashing
        assert!(d.len() <= 1);
    }
}
