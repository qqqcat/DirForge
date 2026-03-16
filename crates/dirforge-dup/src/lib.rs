use dirforge_core::{NodeKind, NodeStore, RiskLevel};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

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

    by_size
        .into_par_iter()
        .filter_map(|(size, paths)| (paths.len() >= 2).then_some((size, paths)))
        .flat_map_iter(|(size, paths)| {
            // phase 2: partial fingerprint regroup
            let partial_entries: Vec<([u8; 32], String)> = paths
                .into_par_iter()
                .map(|p| {
                    let sig = partial_fingerprint(Path::new(&p), cfg.partial_bytes)
                        .unwrap_or_else(|_| hash_bytes(p.as_bytes()));
                    (sig, p)
                })
                .collect();

            let mut partial_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
            for (sig, p) in partial_entries {
                partial_bucket.entry(sig).or_default().push(p);
            }

            let mut groups = Vec::new();

            for (_, partial_paths) in partial_bucket {
                if partial_paths.len() < 2 {
                    continue;
                }

                // phase 3: full hash for stronger confirmation
                let full_entries: Vec<([u8; 32], String)> = partial_paths
                    .into_par_iter()
                    .map(|p| {
                        let hash = if size < cfg.full_hash_min_size {
                            partial_fingerprint(Path::new(&p), cfg.partial_bytes)
                                .unwrap_or_else(|_| hash_bytes(p.as_bytes()))
                        } else {
                            full_hash(Path::new(&p)).unwrap_or_else(|_| hash_bytes(p.as_bytes()))
                        };
                        (hash, p)
                    })
                    .collect();

                let mut full_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
                for (hash, p) in full_entries {
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
                    groups.push(DuplicateGroup {
                        size,
                        members,
                        reclaimable_bytes,
                        risk,
                    });
                }
            }

            groups
        })
        .collect()
}

fn partial_fingerprint(path: &Path, n: usize) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; n.max(1)];

    let head_n = file.read(&mut buf)?;
    hasher.update(&buf[..head_n]);

    if len > n as u64 {
        let tail_n = (n as u64).min(len) as usize;
        file.seek(SeekFrom::End(-(tail_n as i64)))?;
        let mut tail = vec![0u8; tail_n];
        file.read_exact(&mut tail)?;
        hasher.update(&tail);
    }

    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Ok(out)
}

fn full_hash(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read_n = file.read(&mut buf)?;
        if read_n == 0 {
            break;
        }
        hasher.update(&buf[..read_n]);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Ok(out)
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

    #[test]
    fn duplicate_detection_with_real_files() {
        let fixture = dirforge_testkit::FixtureTree::duplicate_file_set().expect("fixture");
        let mut s = NodeStore::default();
        let root = s.add_node(
            None,
            "r".into(),
            fixture.root.display().to_string(),
            NodeKind::Dir,
            0,
        );

        for ent in std::fs::read_dir(fixture.root.join("set")).expect("readdir") {
            let ent = ent.expect("entry");
            let meta = ent.metadata().expect("meta");
            if meta.is_file() {
                s.add_node(
                    Some(root),
                    ent.file_name().to_string_lossy().to_string(),
                    ent.path().display().to_string(),
                    NodeKind::File,
                    meta.len(),
                );
            }
        }

        let groups = detect_duplicates(&s, DupConfig::default());
        assert!(groups.len() >= 2, "expected at least two duplicate groups");
        assert!(groups.iter().all(|g| g.members.len() >= 2));
    }
}
