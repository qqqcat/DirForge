use dirotter_core::{NodeKind, NodeStore, RiskLevel};
use dirotter_platform::stable_file_identity;
use rayon::{prelude::*, ThreadPool, ThreadPoolBuilder};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

thread_local! {
    static SMALL_IO_BUFFER: RefCell<Vec<u8>> = RefCell::new(vec![0u8; 64 * 1024]);
    static LARGE_IO_BUFFER: RefCell<Vec<u8>> = RefCell::new(vec![0u8; 256 * 1024]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateLocation {
    Documents,
    Downloads,
    Desktop,
    Pictures,
    Videos,
    Music,
    Temp,
    Cache,
    ProgramFiles,
    Windows,
    AppData,
    UserData,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateSafetyClass {
    NeverAutoDelete,
    ManualReview,
    CautiousAuto,
    SafeAuto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyReasonTag {
    SystemPath,
    InstalledApp,
    RuntimeDependency,
    UserContent,
    Downloads,
    TempOrCache,
    HiddenOrSystem,
    InstallerPackage,
    ArchivePackage,
    ExecutableBinary,
    DatabaseOrDiskImage,
    ProjectSource,
    DuplicateName,
    SyncFolder,
}

#[derive(Debug, Clone)]
pub struct DuplicateSafetyDecision {
    pub class: DuplicateSafetyClass,
    pub suggested_keep_allowed: bool,
    pub auto_select_allowed: bool,
    pub delete_allowed_by_default: bool,
    pub reason_tags: Vec<SafetyReasonTag>,
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
    pub safety: DuplicateSafetyDecision,
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
    pub latest_groups_found: usize,
    pub latest_duplicate_files_found: usize,
    pub latest_reclaimable_bytes_found: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct DupConfig {
    pub partial_bytes: usize,
    pub sample_bytes: usize,
    pub min_candidate_size: u64,
    pub min_candidate_total_waste: u64,
    pub quick_actionable_only: bool,
    pub small_file_full_hash_max: u64,
    pub large_file_sample_threshold: u64,
    pub large_file_sample_points: usize,
    pub full_hash_min_size: u64,
}

impl Default for DupConfig {
    fn default() -> Self {
        Self {
            partial_bytes: 32 * 1024,
            sample_bytes: 64 * 1024,
            min_candidate_size: 256 * 1024,
            min_candidate_total_waste: 8 * 1024 * 1024,
            quick_actionable_only: false,
            small_file_full_hash_max: 1024 * 1024,
            large_file_sample_threshold: 128 * 1024 * 1024,
            large_file_sample_points: 5,
            full_hash_min_size: 512 * 1024,
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
            let paths = prune_same_identity_paths(paths);
            (paths.len() >= 2).then_some(DuplicateSizeCandidate { size, paths })
        })
        .collect();
    sort_size_candidates(&mut candidates);
    candidates
}

pub fn collect_review_candidates(store: &NodeStore, cfg: DupConfig) -> Vec<DuplicateSizeCandidate> {
    let mut by_size: HashMap<u64, Vec<String>> = HashMap::new();
    for node in &store.nodes {
        if matches!(node.kind, NodeKind::File)
            && node.size_self >= cfg.min_candidate_size
            && (!cfg.quick_actionable_only
                || allow_quick_duplicate_candidate_path(store.node_path(node)))
        {
            by_size
                .entry(node.size_self)
                .or_default()
                .push(store.node_path(node).to_string());
        }
    }

    let mut candidates: Vec<_> = by_size
        .into_iter()
        .filter_map(|(size, paths)| {
            let paths = prune_same_identity_paths(paths);
            let total_waste = size.saturating_mul(paths.len().saturating_sub(1) as u64);
            (paths.len() >= 2 && total_waste >= cfg.min_candidate_total_waste)
                .then_some(DuplicateSizeCandidate { size, paths })
        })
        .collect();
    sort_size_candidates(&mut candidates);
    candidates
}

pub fn sort_size_candidates(candidates: &mut [DuplicateSizeCandidate]) {
    candidates.sort_by(|a, b| {
        a.paths
            .len()
            .cmp(&b.paths.len())
            .then_with(|| candidate_work_hint(a).cmp(&candidate_work_hint(b)))
            .then_with(|| a.size.cmp(&b.size))
    });
}

pub fn detect_duplicates(store: &NodeStore, cfg: DupConfig) -> Vec<DuplicateGroup> {
    resolve_duplicates_with_progress(collect_review_candidates(store, cfg), cfg, |_| {})
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
    let hash_pool = duplicate_hash_pool();

    for (candidate_index, candidate) in candidates.into_iter().enumerate() {
        let new_groups = resolve_candidate_group(candidate, cfg, &mut next_group_id, hash_pool);
        let latest_groups_found = new_groups.len();
        let latest_duplicate_files_found = new_groups.iter().map(|group| group.files.len()).sum();
        let latest_reclaimable_bytes_found = new_groups.iter().map(|group| group.total_waste).sum();
        groups.extend(new_groups);
        on_progress(DuplicateProgress {
            candidate_groups_total: total,
            candidate_groups_processed: candidate_index + 1,
            groups_found: groups.len(),
            latest_groups_found,
            latest_duplicate_files_found,
            latest_reclaimable_bytes_found,
        });

        if total == 0 && latest_groups_found == 0 {
            on_progress(DuplicateProgress {
                candidate_groups_total: 0,
                candidate_groups_processed: 0,
                groups_found: 0,
                latest_groups_found: 0,
                latest_duplicate_files_found: 0,
                latest_reclaimable_bytes_found: 0,
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
    hash_pool: &ThreadPool,
) -> Vec<DuplicateGroup> {
    if candidate.size <= cfg.small_file_full_hash_max {
        return resolve_exact_group(candidate, next_group_id, hash_pool);
    }

    let partial_entries: Vec<([u8; 32], String)> = hash_pool.install(|| {
        candidate
            .paths
            .into_par_iter()
            .map(|path| {
                let sig = partial_fingerprint(Path::new(&path), cfg.partial_bytes)
                    .unwrap_or_else(|_| hash_bytes(path.as_bytes()));
                (sig, path)
            })
            .collect()
    });

    let mut partial_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
    for (sig, path) in partial_entries {
        partial_bucket.entry(sig).or_default().push(path);
    }

    let mut groups = Vec::new();
    for (_, partial_paths) in partial_bucket {
        if partial_paths.len() < 2 {
            continue;
        }

        let sample_buckets: Vec<Vec<String>> = if candidate.size >= cfg.full_hash_min_size {
            let sample_entries: Vec<([u8; 32], String)> = hash_pool.install(|| {
                partial_paths
                    .into_par_iter()
                    .map(|path| {
                        let sig = sample_fingerprint(
                            Path::new(&path),
                            cfg.sample_bytes,
                            sample_point_count(candidate.size, cfg),
                        )
                        .unwrap_or_else(|_| hash_bytes(path.as_bytes()));
                        (sig, path)
                    })
                    .collect()
            });
            let mut sample_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
            for (sig, path) in sample_entries {
                sample_bucket.entry(sig).or_default().push(path);
            }
            sample_bucket
                .into_values()
                .filter(|paths| paths.len() >= 2)
                .collect()
        } else {
            vec![partial_paths]
        };

        for final_paths in sample_buckets {
            if final_paths.len() < 2 {
                continue;
            }

            if final_paths.len() == 2 && candidate.size > cfg.partial_bytes as u64 {
                let left = Path::new(&final_paths[0]);
                let right = Path::new(&final_paths[1]);
                let identical = hash_pool
                    .install(|| files_are_identical(left, right))
                    .unwrap_or(false);
                if !identical {
                    continue;
                }
                push_duplicate_group(candidate.size, final_paths, next_group_id, &mut groups);
                continue;
            }

            let full_entries: Vec<([u8; 32], String)> = hash_pool.install(|| {
                final_paths
                    .into_par_iter()
                    .map(|path| {
                        let hash = if candidate.size <= cfg.partial_bytes as u64 {
                            partial_fingerprint(Path::new(&path), cfg.partial_bytes)
                                .unwrap_or_else(|_| hash_bytes(path.as_bytes()))
                        } else {
                            full_hash(Path::new(&path))
                                .unwrap_or_else(|_| hash_bytes(path.as_bytes()))
                        };
                        (hash, path)
                    })
                    .collect()
            });

            let mut full_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
            for (hash, path) in full_entries {
                full_bucket.entry(hash).or_default().push(path);
            }

            for (_, exact_paths) in full_bucket {
                if exact_paths.len() < 2 {
                    continue;
                }
                push_duplicate_group(candidate.size, exact_paths, next_group_id, &mut groups);
            }
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

fn resolve_exact_group(
    candidate: DuplicateSizeCandidate,
    next_group_id: &mut u64,
    hash_pool: &ThreadPool,
) -> Vec<DuplicateGroup> {
    let full_entries: Vec<([u8; 32], String)> = hash_pool.install(|| {
        candidate
            .paths
            .into_par_iter()
            .map(|path| {
                let hash =
                    full_hash(Path::new(&path)).unwrap_or_else(|_| hash_bytes(path.as_bytes()));
                (hash, path)
            })
            .collect()
    });

    let mut full_bucket: HashMap<[u8; 32], Vec<String>> = HashMap::new();
    for (hash, path) in full_entries {
        full_bucket.entry(hash).or_default().push(path);
    }

    let mut groups = Vec::new();
    for exact_paths in full_bucket.into_values() {
        if exact_paths.len() < 2 {
            continue;
        }
        push_duplicate_group(candidate.size, exact_paths, next_group_id, &mut groups);
    }

    groups.sort_by(|a, b| {
        b.total_waste
            .cmp(&a.total_waste)
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.id.cmp(&b.id))
    });
    groups
}

fn duplicate_hash_pool() -> &'static ThreadPool {
    static POOL: OnceLock<ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().min(4).max(2))
            .unwrap_or(2);
        ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|index| format!("dirotter-dup-{index}"))
            .build()
            .expect("duplicate hash thread pool")
    })
}

fn candidate_work_hint(candidate: &DuplicateSizeCandidate) -> u64 {
    candidate
        .size
        .saturating_mul(candidate.paths.len().saturating_sub(1) as u64)
}

fn prune_same_identity_paths(paths: Vec<String>) -> Vec<String> {
    if paths.len() < 2 {
        return paths;
    }

    let mut kept = Vec::with_capacity(paths.len());
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        if let Ok(identity) = stable_file_identity(&path) {
            if !seen.insert((identity.dev, identity.inode)) {
                continue;
            }
        }
        kept.push(path);
    }
    kept
}

fn sample_point_count(size: u64, cfg: DupConfig) -> usize {
    if size >= cfg.large_file_sample_threshold {
        cfg.large_file_sample_points.max(3)
    } else {
        3
    }
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
    } else if lower.contains("\\users\\") && lower.contains("\\pictures\\") {
        DuplicateLocation::Pictures
    } else if lower.contains("\\users\\") && lower.contains("\\videos\\") {
        DuplicateLocation::Videos
    } else if lower.contains("\\users\\") && lower.contains("\\music\\") {
        DuplicateLocation::Music
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
        DuplicateLocation::Pictures => 170,
        DuplicateLocation::Videos => 170,
        DuplicateLocation::Music => 150,
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

pub fn allow_quick_duplicate_candidate_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let ext = file_extension(path);
    if is_never_auto_path(&lower) || is_runtime_extension(ext.as_deref()) {
        return false;
    }

    if matches!(
        ext.as_deref(),
        Some("db" | "sqlite" | "sqlite3" | "pst" | "ost" | "vhd" | "vhdx" | "vmdk" | "qcow2")
    ) {
        return false;
    }

    if matches!(
        ext.as_deref(),
        Some("psd" | "ai" | "aep" | "prproj" | "blend" | "max" | "skp" | "dwg" | "cad")
    ) {
        return false;
    }

    lower.contains("\\downloads\\")
        || lower.contains("\\temp\\")
        || lower.contains("\\cache\\")
        || looks_like_duplicate_copy_name(path)
}

fn safety_decision_for_group(files: &[DuplicateFileEntry]) -> DuplicateSafetyDecision {
    let mut reason_tags = Vec::new();
    let mut has_never = false;
    let mut has_manual = false;
    let mut has_cautious = false;
    let mut has_safe = false;

    for file in files {
        let lower = file.path.to_ascii_lowercase();
        let ext = file_extension(&file.path);

        if file.hidden || file.system {
            push_reason(&mut reason_tags, SafetyReasonTag::HiddenOrSystem);
            has_never = true;
        }
        if is_never_auto_path(&lower) {
            push_reason(
                &mut reason_tags,
                if lower.contains("\\windows") {
                    SafetyReasonTag::SystemPath
                } else {
                    SafetyReasonTag::InstalledApp
                },
            );
            has_never = true;
        }
        if is_runtime_extension(ext.as_deref()) {
            push_reason(
                &mut reason_tags,
                if matches!(ext.as_deref(), Some("exe")) {
                    SafetyReasonTag::ExecutableBinary
                } else {
                    SafetyReasonTag::RuntimeDependency
                },
            );
            has_never = true;
        }
        if matches!(ext.as_deref(), Some("msi" | "msp" | "cab")) && !lower.contains("\\downloads\\")
        {
            push_reason(&mut reason_tags, SafetyReasonTag::InstallerPackage);
            has_never = true;
        }

        if matches!(
            file.location,
            DuplicateLocation::Documents
                | DuplicateLocation::Desktop
                | DuplicateLocation::Pictures
                | DuplicateLocation::Videos
                | DuplicateLocation::Music
        ) {
            push_reason(&mut reason_tags, SafetyReasonTag::UserContent);
            has_manual = true;
        }
        if lower.contains("\\onedrive\\")
            || lower.contains("\\dropbox\\")
            || lower.contains("\\google drive\\")
            || lower.contains("\\icloud drive\\")
            || lower.contains("\\syncthing\\")
        {
            push_reason(&mut reason_tags, SafetyReasonTag::SyncFolder);
            has_manual = true;
        }
        if matches!(
            ext.as_deref(),
            Some("db" | "sqlite" | "sqlite3" | "pst" | "ost" | "vhd" | "vhdx" | "vmdk" | "qcow2")
        ) {
            push_reason(&mut reason_tags, SafetyReasonTag::DatabaseOrDiskImage);
            has_manual = true;
        }
        if matches!(
            ext.as_deref(),
            Some("psd" | "ai" | "aep" | "prproj" | "blend" | "max" | "skp" | "dwg" | "cad")
        ) {
            push_reason(&mut reason_tags, SafetyReasonTag::ProjectSource);
            has_manual = true;
        }

        if file.location == DuplicateLocation::Downloads {
            push_reason(&mut reason_tags, SafetyReasonTag::Downloads);
            has_cautious = true;
        }
        if matches!(ext.as_deref(), Some("zip" | "rar" | "7z")) {
            push_reason(&mut reason_tags, SafetyReasonTag::ArchivePackage);
            has_cautious = true;
        }
        if matches!(ext.as_deref(), Some("msi" | "msp" | "cab")) && lower.contains("\\downloads\\")
        {
            push_reason(&mut reason_tags, SafetyReasonTag::InstallerPackage);
            has_cautious = true;
        }

        if matches!(
            file.location,
            DuplicateLocation::Temp | DuplicateLocation::Cache
        ) {
            push_reason(&mut reason_tags, SafetyReasonTag::TempOrCache);
            has_safe = true;
        }
        if looks_like_duplicate_copy_name(&file.path) {
            push_reason(&mut reason_tags, SafetyReasonTag::DuplicateName);
            has_safe = true;
        }
    }

    let class = if has_never {
        DuplicateSafetyClass::NeverAutoDelete
    } else if has_manual {
        DuplicateSafetyClass::ManualReview
    } else if has_cautious {
        DuplicateSafetyClass::CautiousAuto
    } else if has_safe {
        DuplicateSafetyClass::SafeAuto
    } else {
        DuplicateSafetyClass::ManualReview
    };

    DuplicateSafetyDecision {
        class,
        suggested_keep_allowed: !matches!(class, DuplicateSafetyClass::NeverAutoDelete),
        auto_select_allowed: matches!(
            class,
            DuplicateSafetyClass::CautiousAuto | DuplicateSafetyClass::SafeAuto
        ),
        delete_allowed_by_default: matches!(class, DuplicateSafetyClass::SafeAuto),
        reason_tags,
    }
}

fn push_reason(tags: &mut Vec<SafetyReasonTag>, tag: SafetyReasonTag) {
    if !tags.contains(&tag) {
        tags.push(tag);
    }
}

fn is_never_auto_path(lower: &str) -> bool {
    lower.contains("\\windows\\")
        || lower.contains("\\program files\\")
        || lower.contains("\\program files (x86)\\")
        || lower.contains("\\programdata\\package cache\\")
        || lower.contains("\\windows\\installer\\")
        || lower.contains("\\windows\\winsxs\\")
}

fn is_runtime_extension(ext: Option<&str>) -> bool {
    matches!(
        ext,
        Some("exe" | "dll" | "sys" | "drv" | "com" | "ocx" | "cpl")
    )
}

fn file_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn looks_like_duplicate_copy_name(path: &str) -> bool {
    let lower = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    lower.contains(" copy")
        || lower.contains(" - copy")
        || lower.contains("副本")
        || lower.contains("(1)")
        || lower.contains("(2)")
}

fn partial_fingerprint(path: &Path, n: usize) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    with_small_io_buffer(n.max(1), |buf| -> io::Result<()> {
        let usable = n.max(1);
        let head_n = file.read(&mut buf[..usable])?;
        hasher.update(&buf[..head_n]);

        if len > n as u64 {
            let tail_n = (n as u64).min(len) as usize;
            file.seek(SeekFrom::End(-(tail_n as i64)))?;
            file.read_exact(&mut buf[..tail_n])?;
            hasher.update(&buf[..tail_n]);
        }
        Ok(())
    })?;

    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Ok(out)
}

fn sample_fingerprint(path: &Path, n: usize, sample_points: usize) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&len.to_le_bytes());

    let chunk_len = (n.max(1024) as u64).min(len.max(1));
    let mut offsets = sample_offsets(len, chunk_len, sample_points);
    offsets.sort_unstable();
    offsets.dedup();

    with_small_io_buffer(chunk_len as usize, |buf| -> io::Result<()> {
        for offset in offsets {
            file.seek(SeekFrom::Start(offset))?;
            let read_n = file.read(&mut buf[..chunk_len as usize])?;
            hasher.update(&buf[..read_n]);
        }
        Ok(())
    })?;

    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Ok(out)
}

fn sample_offsets(len: u64, chunk_len: u64, sample_points: usize) -> Vec<u64> {
    if len <= chunk_len {
        return vec![0];
    }

    let points = sample_points.max(2);
    let span = len.saturating_sub(chunk_len);
    (0..points)
        .map(|index| {
            if index + 1 == points {
                span
            } else {
                span.saturating_mul(index as u64) / (points.saturating_sub(1) as u64)
            }
        })
        .collect()
}

fn full_hash(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    with_large_io_buffer(256 * 1024, |buf| -> io::Result<()> {
        loop {
            let read_n = file.read(&mut buf[..256 * 1024])?;
            if read_n == 0 {
                break;
            }
            hasher.update(&buf[..read_n]);
        }
        Ok(())
    })?;
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Ok(out)
}

fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(blake3::hash(bytes).as_bytes());
    out
}

fn files_are_identical(left: &Path, right: &Path) -> io::Result<bool> {
    let left_meta = std::fs::metadata(left)?;
    let right_meta = std::fs::metadata(right)?;
    if left_meta.len() != right_meta.len() {
        return Ok(false);
    }

    let mut left_file = File::open(left)?;
    let mut right_file = File::open(right)?;
    let mut right_buf = [0u8; 256 * 1024];
    with_large_io_buffer(256 * 1024, |left_buf| -> io::Result<bool> {
        loop {
            let left_n = left_file.read(&mut left_buf[..256 * 1024])?;
            let right_n = right_file.read(&mut right_buf)?;
            if left_n != right_n {
                return Ok(false);
            }
            if left_n == 0 {
                return Ok(true);
            }
            if left_buf[..left_n] != right_buf[..right_n] {
                return Ok(false);
            }
        }
    })
}

fn with_small_io_buffer<T>(size: usize, f: impl FnOnce(&mut Vec<u8>) -> T) -> T {
    SMALL_IO_BUFFER.with(|cell| {
        let mut buf = cell.borrow_mut();
        if buf.len() < size {
            buf.resize(size, 0);
        }
        f(&mut buf)
    })
}

fn with_large_io_buffer<T>(size: usize, f: impl FnOnce(&mut Vec<u8>) -> T) -> T {
    LARGE_IO_BUFFER.with(|cell| {
        let mut buf = cell.borrow_mut();
        if buf.len() < size {
            buf.resize(size, 0);
        }
        f(&mut buf)
    })
}

fn push_duplicate_group(
    size: u64,
    exact_paths: Vec<String>,
    next_group_id: &mut u64,
    groups: &mut Vec<DuplicateGroup>,
) {
    let mut files: Vec<DuplicateFileEntry> = exact_paths
        .iter()
        .map(|path| build_file_entry(path, size))
        .collect();
    if files.len() < 2 {
        return;
    }

    let recommended_keep_index = recommend_keep_index(&files);
    let total_waste = size.saturating_mul((files.len() as u64).saturating_sub(1));
    let safety = safety_decision_for_group(&files);
    let risk = match safety.class {
        DuplicateSafetyClass::NeverAutoDelete => RiskLevel::High,
        DuplicateSafetyClass::ManualReview => RiskLevel::Medium,
        DuplicateSafetyClass::CautiousAuto => RiskLevel::Medium,
        DuplicateSafetyClass::SafeAuto => RiskLevel::Low,
    };
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
        size,
        files,
        total_waste,
        risk,
        safety,
        recommended_keep_index,
    });
    *next_group_id = next_group_id.saturating_add(1);
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

        let groups = detect_duplicates(
            &store,
            DupConfig {
                min_candidate_size: 0,
                min_candidate_total_waste: 0,
                ..DupConfig::default()
            },
        );
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

    #[test]
    fn pairwise_compare_detects_non_identical_files_without_hashing_groups() {
        let fixture = dirotter_testkit::FixtureTree::sample().expect("fixture");
        let left = fixture.root.join("left.bin");
        let right = fixture.root.join("right.bin");
        std::fs::write(&left, vec![1u8; 256 * 1024]).expect("write left");
        let mut right_bytes = vec![1u8; 256 * 1024];
        right_bytes[4096] = 7;
        std::fs::write(&right, right_bytes).expect("write right");

        assert!(
            !files_are_identical(&left, &right).expect("compare files"),
            "different files should not compare as identical"
        );
    }

    #[test]
    fn size_candidates_collapse_same_identity_hardlinks() {
        let fixture = dirotter_testkit::FixtureTree::sample().expect("fixture");
        let left = fixture.root.join("left.txt");
        let right = fixture.root.join("right.txt");
        std::fs::write(&left, b"same-data").expect("write left");
        std::fs::hard_link(&left, &right).expect("hard link");

        let mut store = NodeStore::default();
        let root = store.add_node(
            None,
            "r".into(),
            fixture.root.display().to_string(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(root),
            "left.txt".into(),
            left.display().to_string(),
            NodeKind::File,
            9,
        );
        store.add_node(
            Some(root),
            "right.txt".into(),
            right.display().to_string(),
            NodeKind::File,
            9,
        );

        let candidates = collect_size_candidates(&store);
        assert!(
            candidates.is_empty(),
            "hardlinks to the same file should be collapsed before duplicate hashing"
        );
    }
}
