use dirotter_core::{NodeKind, NodeStore, RiskLevel};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateLocation {
    Documents,
    Downloads,
    Desktop,
    Temp,
    Cache,
    ProgramFiles,
    Windows,
    AppData,
    UserData,
    Other,
}

#[derive(Debug, Clone)]
pub struct DuplicateFileEntry {
    pub path: String,
    pub size: u64,
    pub modified_unix_secs: Option<u64>,
    pub location: DuplicateLocation,
    pub hidden: bool,
    pub system: bool,
    pub keep_score: i32,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub id: u64,
    pub size: u64,
    pub files: Vec<DuplicateFileEntry>,
    pub total_waste: u64,
    pub risk: RiskLevel,
    pub recommended_keep_index: usize,
}

#[derive(Debug, Clone)]
pub struct DuplicateSizeCandidate {
    pub size: u64,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DuplicateProgress {
    pub candidate_groups_total: usize,
    pub candidate_groups_processed: usize,
    pub groups_found: usize,
    pub latest_groups: Vec<DuplicateGroup>,
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

pub fn collect_size_candidates(store: &NodeStore) -> Vec<DuplicateSizeCandidate> {
    let mut by_size: HashMap<u64, Vec<String>> = HashMap::new();
    for node in &store.nodes {
        if matches!(node.kind, NodeKind::File) && node.size_self > 0 {
            by_size
                .entry(node.size_self)
                .or_default()
                .push(store.node_path(node).to_string());
        }
    }

    let mut candidates: Vec<_> = by_size
        .into_iter()
        .filter_map(|(size, paths)| {
            (paths.len() >= 2).then_some(DuplicateSizeCandidate { size, paths })
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.paths.len().cmp(&b.paths.len()))
    });
    candidates
}

pub fn detect_duplicates(store: &NodeStore, cfg: DupConfig) -> Vec<DuplicateGroup> {
    resolve_duplicates_with_progress(collect_size_candidates(store), cfg, |_| {})
}

pub fn resolve_duplicates_with_progress<F>(
    candidates: Vec<DuplicateSizeCandidate>,
    cfg: DupConfig,
    mut on_progress: F,
) -> Vec<DuplicateGroup>
where
    F: FnMut(DuplicateProgress),
{
    let total = candidates.len();
    let mut groups = Vec::new();
    let mut next_group_id = 1u64;

    for (candidate_index, candidate) in candidates.into_iter().enumerate() {
        let new_groups = resolve_candidate_group(candidate, cfg, &mut next_group_id);
        let new_group_count = new_groups.len();
        groups.extend(new_groups.clone());
        on_progress(DuplicateProgress {
            candidate_groups_total: total,
            candidate_groups_processed: candidate_index + 1,
            groups_found: groups.len(),
            latest_groups: new_groups,
        });

        if total == 0 && new_group_count == 0 {
            on_progress(DuplicateProgress {
                candidate_groups_total: 0,
                candidate_groups_processed: 0,
                groups_found: 0,
                latest_groups: Vec::new(),
            });
        }
    }

    groups.sort_by(|a, b| {
        b.total_waste
            .cmp(&a.total_waste)
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.id.cmp(&b.id))
    });
    groups
}

fn resolve_candidate_group(
    candidate: DuplicateSizeCandidate,
    cfg: DupConfig,
    next_group_id: &mut u64,
) -> Vec<DuplicateGroup> {
    let partial_entries: Vec<([u8; 32], String)> = candidate
        .paths
        .into_par_iter()
        .map(|path| {
            let sig = partial_fingerprint(Path::new(&path), cfg.partial_bytes)
                .unwrap_or_else(|_| hash_bytes(path.as_bytes()));
            (sig, path)
        })
        .collect();

    let mut partial_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
    for (sig, path) in partial_entries {
        partial_bucket.entry(sig).or_default().push(path);
    }

    let mut groups = Vec::new();
    for (_, partial_paths) in partial_bucket {
        if partial_paths.len() < 2 {
            continue;
        }

        let full_entries: Vec<([u8; 32], String)> = partial_paths
            .into_par_iter()
            .map(|path| {
                let hash = if candidate.size < cfg.full_hash_min_size {
                    partial_fingerprint(Path::new(&path), cfg.partial_bytes)
                        .unwrap_or_else(|_| hash_bytes(path.as_bytes()))
                } else {
                    full_hash(Path::new(&path)).unwrap_or_else(|_| hash_bytes(path.as_bytes()))
                };
                (hash, path)
            })
            .collect();

        let mut full_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
        for (hash, path) in full_entries {
            full_bucket.entry(hash).or_default().push(path);
        }

        for (_, final_paths) in full_bucket {
            if final_paths.len() < 2 {
                continue;
            }

            let mut files: Vec<DuplicateFileEntry> = final_paths
                .iter()
                .map(|path| build_file_entry(path, candidate.size))
                .collect();
            if files.len() < 2 {
                continue;
            }

            let recommended_keep_index = recommend_keep_index(&files);
            let total_waste = candidate
                .size
                .saturating_mul((files.len() as u64).saturating_sub(1));
            let risk = risk_of_group(&files);
            files.sort_by(|a, b| a.path.cmp(&b.path));
            let recommended_keep_path = files
                .get(recommended_keep_index)
                .map(|file| file.path.clone())
                .unwrap_or_default();
            let recommended_keep_index = files
                .iter()
                .position(|file| file.path == recommended_keep_path)
                .unwrap_or(0);

            groups.push(DuplicateGroup {
                id: *next_group_id,
                size: candidate.size,
                files,
                total_waste,
                risk,
                recommended_keep_index,
            });
            *next_group_id = next_group_id.saturating_add(1);
        }
    }

    groups.sort_by(|a, b| {
        b.total_waste
            .cmp(&a.total_waste)
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.id.cmp(&b.id))
    });
    groups
}

fn build_file_entry(path: &str, size: u64) -> DuplicateFileEntry {
    let metadata = std::fs::metadata(path).ok();
    let modified_unix_secs = metadata
        .as_ref()
        .and_then(|meta| meta.modified().ok())
        .and_then(system_time_to_unix);
    let (hidden, system) = file_attribute_flags(path, metadata.as_ref());
    let location = classify_location(path);

    let mut entry = DuplicateFileEntry {
        path: path.to_string(),
        size,
        modified_unix_secs,
        location,
        hidden,
        system,
        keep_score: 0,
    };
    entry.keep_score = keep_score(&entry);
    entry
}

fn system_time_to_unix(stamp: SystemTime) -> Option<u64> {
    stamp
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn file_attribute_flags(_path: &str, metadata: Option<&std::fs::Metadata>) -> (bool, bool) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;

        let attributes = metadata.map(|meta| meta.file_attributes()).unwrap_or(0);
        let hidden = (attributes & 0x2) != 0;
        let system = (attributes & 0x4) != 0;
        (hidden, system)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let hidden = Path::new(_path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false);
        let _ = metadata;
        (hidden, false)
    }
}

fn classify_location(path: &str) -> DuplicateLocation {
    let lower = path.to_ascii_lowercase();
    if lower.contains("\\windows") {
        DuplicateLocation::Windows
    } else if lower.contains("\\program files") {
        DuplicateLocation::ProgramFiles
    } else if lower.contains("\\users\\") && lower.contains("\\documents\\") {
        DuplicateLocation::Documents
    } else if lower.contains("\\users\\") && lower.contains("\\downloads\\") {
        DuplicateLocation::Downloads
    } else if lower.contains("\\users\\") && lower.contains("\\desktop\\") {
        DuplicateLocation::Desktop
    } else if lower.contains("\\appdata\\local\\temp")
        || lower.contains("\\temp\\")
        || lower.ends_with("\\temp")
    {
        DuplicateLocation::Temp
    } else if lower.contains("\\cache\\") || lower.ends_with("\\cache") {
        DuplicateLocation::Cache
    } else if lower.contains("\\appdata\\") {
        DuplicateLocation::AppData
    } else if lower.contains("\\users\\") {
        DuplicateLocation::UserData
    } else {
        DuplicateLocation::Other
    }
}

fn keep_score(file: &DuplicateFileEntry) -> i32 {
    let mut score = match file.location {
        DuplicateLocation::Documents => 180,
        DuplicateLocation::Desktop => 160,
        DuplicateLocation::Downloads => 130,
        DuplicateLocation::UserData => 110,
        DuplicateLocation::Other => 70,
        DuplicateLocation::AppData => 10,
        DuplicateLocation::Cache => -120,
        DuplicateLocation::Temp => -160,
        DuplicateLocation::ProgramFiles => -220,
        DuplicateLocation::Windows => -260,
    };

    if file.hidden {
        score -= 40;
    } else {
        score += 20;
    }

    if file.system {
        score -= 120;
    } else {
        score += 20;
    }

    if let Some(modified) = file.modified_unix_secs {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let age_days = ((now.saturating_sub(modified)) / 86_400).min(365);
        score += (365u64.saturating_sub(age_days)) as i32;
    }

    let path_bonus = (80 - file.path.len().min(120) as i32).max(-40);
    score += path_bonus;
    score
}

fn recommend_keep_index(files: &[DuplicateFileEntry]) -> usize {
    files
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            left.keep_score
                .cmp(&right.keep_score)
                .then_with(|| left.modified_unix_secs.cmp(&right.modified_unix_secs))
                .then_with(|| right.path.len().cmp(&left.path.len()))
                .then_with(|| right.path.cmp(&left.path))
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn risk_of_group(files: &[DuplicateFileEntry]) -> RiskLevel {
    if files.iter().any(|file| {
        file.system
            || matches!(
                file.location,
                DuplicateLocation::Windows | DuplicateLocation::ProgramFiles
            )
    }) {
        RiskLevel::High
    } else if files.iter().any(|file| {
        matches!(
            file.location,
            DuplicateLocation::Downloads | DuplicateLocation::AppData
        )
    }) {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use dirotter_core::NodeStore;

    #[test]
    fn duplicate_by_size_collects_candidates() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "r".into(), "/r".into(), NodeKind::Dir, 0);
        store.add_node(Some(root), "a".into(), "/r/a".into(), NodeKind::File, 7);
        store.add_node(Some(root), "b".into(), "/r/b".into(), NodeKind::File, 7);
        let candidates = collect_size_candidates(&store);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].size, 7);
        assert_eq!(candidates[0].paths.len(), 2);
    }

    #[test]
    fn duplicate_detection_with_real_files_returns_product_groups() {
        let fixture = dirotter_testkit::FixtureTree::duplicate_file_set().expect("fixture");
        let mut store = NodeStore::default();
        let root = store.add_node(
            None,
            "r".into(),
            fixture.root.display().to_string(),
            NodeKind::Dir,
            0,
        );

        for entry in std::fs::read_dir(fixture.root.join("set")).expect("readdir") {
            let entry = entry.expect("entry");
            let meta = entry.metadata().expect("meta");
            if meta.is_file() {
                store.add_node(
                    Some(root),
                    entry.file_name().to_string_lossy().to_string(),
                    entry.path().display().to_string(),
                    NodeKind::File,
                    meta.len(),
                );
            }
        }

        let groups = detect_duplicates(&store, DupConfig::default());
        assert!(groups.len() >= 2, "expected at least two duplicate groups");
        assert!(groups.iter().all(|group| group.files.len() >= 2));
        assert!(groups.iter().all(|group| group.total_waste >= group.size));
    }

    #[test]
    fn recommendation_prefers_documents_over_temp() {
        let documents = DuplicateFileEntry {
            path: "c:\\Users\\alice\\Documents\\report.docx".into(),
            size: 42,
            modified_unix_secs: Some(1_700_000_000),
            location: DuplicateLocation::Documents,
            hidden: false,
            system: false,
            keep_score: 0,
        };
        let temp = DuplicateFileEntry {
            path: "c:\\Users\\alice\\AppData\\Local\\Temp\\report.docx".into(),
            size: 42,
            modified_unix_secs: Some(1_700_000_100),
            location: DuplicateLocation::Temp,
            hidden: false,
            system: false,
            keep_score: 0,
        };
        let mut files = vec![documents, temp];
        for file in &mut files {
            file.keep_score = keep_score(file);
        }

        assert_eq!(recommend_keep_index(&files), 0);
    }
}
