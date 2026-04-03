use crate::{
    path_within_scope, SelectedTarget, MAX_BLOCKED_ITEMS_PER_CATEGORY,
    MAX_CLEANUP_ITEMS_PER_CATEGORY, MAX_CLEANUP_TOTAL_ITEMS, MIN_CACHE_DIR_BYTES,
    MIN_CLEANUP_BYTES,
};
use dirotter_actions::ExecutionMode;
use dirotter_core::{NodeKind, NodeStore, RiskLevel};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CleanupCategory {
    Cache,
    Downloads,
    Video,
    Archive,
    Installer,
    Image,
    System,
    Other,
}

#[derive(Clone)]
pub(crate) struct CleanupCandidate {
    pub(crate) target: SelectedTarget,
    pub(crate) category: CleanupCategory,
    pub(crate) risk: RiskLevel,
    pub(crate) cleanup_score: f32,
    pub(crate) unused_days: Option<u64>,
}

#[derive(Clone)]
pub(crate) struct CleanupCategorySummary {
    pub(crate) category: CleanupCategory,
    pub(crate) total_bytes: u64,
    pub(crate) reclaimable_bytes: u64,
    pub(crate) blocked_bytes: u64,
    pub(crate) item_count: usize,
}

#[derive(Clone, Default)]
pub(crate) struct CleanupAnalysis {
    pub(crate) reclaimable_bytes: u64,
    pub(crate) quick_clean_bytes: u64,
    pub(crate) categories: Vec<CleanupCategorySummary>,
    pub(crate) items: Vec<CleanupCandidate>,
}

pub(crate) fn cleanup_category_for_path(path: &str, kind: NodeKind) -> CleanupCategory {
    let lower = path.to_ascii_lowercase();
    let extension = PathBuf::from(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext.to_ascii_lowercase()))
        .unwrap_or_default();

    if is_system_path(&lower) {
        return CleanupCategory::System;
    }
    if is_cache_path(&lower, kind) {
        return CleanupCategory::Cache;
    }
    if lower.contains("\\downloads\\") || lower.ends_with("\\downloads") {
        return CleanupCategory::Downloads;
    }
    if matches!(
        extension.as_str(),
        ".mp4" | ".mkv" | ".avi" | ".mov" | ".wmv" | ".flv" | ".webm"
    ) {
        return CleanupCategory::Video;
    }
    if matches!(extension.as_str(), ".zip" | ".rar" | ".7z" | ".tar" | ".gz") {
        return CleanupCategory::Archive;
    }
    if matches!(extension.as_str(), ".exe" | ".msi" | ".pkg" | ".dmg") {
        return CleanupCategory::Installer;
    }
    if matches!(
        extension.as_str(),
        ".jpg" | ".jpeg" | ".png" | ".gif" | ".bmp" | ".webp" | ".heic"
    ) {
        return CleanupCategory::Image;
    }
    CleanupCategory::Other
}

pub(crate) fn cleanup_risk_for_path(path: &str, category: CleanupCategory) -> RiskLevel {
    let lower = path.to_ascii_lowercase();
    if is_system_path(&lower)
        || lower.ends_with("\\hiberfil.sys")
        || lower.ends_with("\\pagefile.sys")
        || lower.ends_with("\\swapfile.sys")
    {
        RiskLevel::High
    } else if category == CleanupCategory::Cache {
        RiskLevel::Low
    } else if lower.contains("\\appdata\\") {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

pub(crate) fn build_cleanup_analysis(store: &NodeStore) -> CleanupAnalysis {
    let mut cache_dirs: Vec<&dirotter_core::Node> = store
        .nodes
        .iter()
        .filter(|node| {
            let path = store.node_path(node);
            node.kind == NodeKind::Dir
                && is_cache_path(&path.to_ascii_lowercase(), node.kind)
                && node.size_subtree.max(node.size_self) >= MIN_CACHE_DIR_BYTES
        })
        .collect();
    cache_dirs.sort_by(|a, b| {
        store
            .node_path(a)
            .len()
            .cmp(&store.node_path(b).len())
            .then_with(|| b.size_subtree.cmp(&a.size_subtree))
    });

    let mut cache_scope_paths: Vec<String> = Vec::new();
    let mut category_candidates: HashMap<CleanupCategory, Vec<CleanupCandidate>> = HashMap::new();

    for node in cache_dirs {
        let node_path = store.node_path(node);
        let node_name = store.node_name(node);
        if cache_scope_paths
            .iter()
            .any(|scope| path_within_scope(node_path, scope))
        {
            continue;
        }

        cache_scope_paths.push(node_path.to_string());
        let target = SelectedTarget {
            name: node_name.to_string(),
            path: node_path.to_string(),
            size_bytes: node.size_subtree.max(node.size_self),
            kind: node.kind,
            file_count: node.file_count,
            dir_count: node.dir_count,
        };
        let unused_days = cleanup_unused_days(&target.path);
        push_ranked_cleanup_candidate(
            &mut category_candidates,
            CleanupCandidate {
                cleanup_score: cleanup_score(
                    target.size_bytes,
                    unused_days,
                    CleanupCategory::Cache,
                    RiskLevel::Low,
                ),
                target,
                category: CleanupCategory::Cache,
                risk: RiskLevel::Low,
                unused_days,
            },
        );
    }

    for node in store
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
    {
        let node_path = store.node_path(node);
        let node_name = store.node_name(node);
        if cache_scope_paths
            .iter()
            .any(|scope| path_within_scope(node_path, scope))
        {
            continue;
        }

        let category = cleanup_category_for_path(node_path, node.kind);
        let risk = cleanup_risk_for_path(node_path, category);
        if category == CleanupCategory::Other && node.size_self < MIN_CLEANUP_BYTES * 4 {
            continue;
        }
        if category != CleanupCategory::System && node.size_self < MIN_CLEANUP_BYTES {
            continue;
        }

        let unused_days = if risk == RiskLevel::High {
            None
        } else {
            cleanup_unused_days(node_path)
        };
        let score = cleanup_score(node.size_self, unused_days, category, risk);
        if risk != RiskLevel::High && score < 1.0 {
            continue;
        }

        push_ranked_cleanup_candidate(
            &mut category_candidates,
            CleanupCandidate {
                target: SelectedTarget {
                    name: node_name.to_string(),
                    path: node_path.to_string(),
                    size_bytes: node.size_self,
                    kind: node.kind,
                    file_count: node.file_count,
                    dir_count: node.dir_count,
                },
                category,
                risk,
                cleanup_score: score,
                unused_days,
            },
        );
    }

    let mut items: Vec<CleanupCandidate> = category_candidates.into_values().flatten().collect();
    items.sort_by(|a, b| {
        rank_cleanup_candidate(b)
            .cmp(&rank_cleanup_candidate(a))
            .then_with(|| cleanup_sort_priority(a.category).cmp(&cleanup_sort_priority(b.category)))
            .then_with(|| a.target.path.cmp(&b.target.path))
    });
    if items.len() > MAX_CLEANUP_TOTAL_ITEMS {
        items.truncate(MAX_CLEANUP_TOTAL_ITEMS);
    }

    let mut category_map: HashMap<CleanupCategory, CleanupCategorySummary> = HashMap::new();
    let mut reclaimable_bytes = 0u64;
    let mut quick_clean_bytes = 0u64;
    for item in &items {
        let summary = category_map
            .entry(item.category)
            .or_insert_with(|| CleanupCategorySummary {
                category: item.category,
                total_bytes: 0,
                reclaimable_bytes: 0,
                blocked_bytes: 0,
                item_count: 0,
            });
        summary.total_bytes = summary.total_bytes.saturating_add(item.target.size_bytes);
        summary.item_count += 1;
        if item.risk == RiskLevel::High {
            summary.blocked_bytes = summary.blocked_bytes.saturating_add(item.target.size_bytes);
        } else {
            summary.reclaimable_bytes = summary
                .reclaimable_bytes
                .saturating_add(item.target.size_bytes);
            reclaimable_bytes = reclaimable_bytes.saturating_add(item.target.size_bytes);
            if item.category == CleanupCategory::Cache && item.risk == RiskLevel::Low {
                quick_clean_bytes = quick_clean_bytes.saturating_add(item.target.size_bytes);
            }
        }
    }

    let mut categories: Vec<_> = category_map.into_values().collect();
    categories.sort_by(|a, b| {
        b.reclaimable_bytes
            .cmp(&a.reclaimable_bytes)
            .then_with(|| b.total_bytes.cmp(&a.total_bytes))
            .then_with(|| cleanup_sort_priority(a.category).cmp(&cleanup_sort_priority(b.category)))
    });

    CleanupAnalysis {
        reclaimable_bytes,
        quick_clean_bytes,
        categories,
        items,
    }
}

pub(crate) fn cleanup_delete_mode_for_category(category: CleanupCategory) -> ExecutionMode {
    if category == CleanupCategory::Cache {
        ExecutionMode::FastPurge
    } else {
        ExecutionMode::RecycleBin
    }
}

pub(crate) fn can_fast_purge_path(path: &str) -> bool {
    let kind = fs::metadata(path)
        .ok()
        .map(|meta| {
            if meta.is_dir() {
                NodeKind::Dir
            } else {
                NodeKind::File
            }
        })
        .unwrap_or(NodeKind::File);
    let category = cleanup_category_for_path(path, kind);
    let risk = cleanup_risk_for_path(path, category);
    category == CleanupCategory::Cache && risk == RiskLevel::Low
}

fn cleanup_sort_priority(category: CleanupCategory) -> usize {
    match category {
        CleanupCategory::Cache => 0,
        CleanupCategory::Downloads => 1,
        CleanupCategory::Installer => 2,
        CleanupCategory::Archive => 3,
        CleanupCategory::Video => 4,
        CleanupCategory::Image => 5,
        CleanupCategory::Other => 6,
        CleanupCategory::System => 7,
    }
}

fn is_system_path(lower_path: &str) -> bool {
    lower_path.contains("\\windows")
        || lower_path.contains("\\program files")
        || lower_path.contains("\\programdata")
        || lower_path.contains("\\system volume information")
        || lower_path.contains("\\$recycle.bin")
}

fn is_cache_path(lower_path: &str, kind: NodeKind) -> bool {
    lower_path.contains("\\appdata\\local\\temp")
        || lower_path.contains("\\temp\\")
        || lower_path.ends_with("\\temp")
        || lower_path.contains("\\cache\\")
        || lower_path.ends_with("\\cache")
        || lower_path.contains("\\tmp\\")
        || lower_path.ends_with("\\tmp")
        || (matches!(kind, NodeKind::Dir)
            && (lower_path.ends_with("\\gpucache")
                || lower_path.ends_with("\\shadercache")
                || lower_path.ends_with("\\code cache")
                || lower_path.ends_with("\\cached data")))
}

fn cleanup_unused_days(path: &str) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let now = std::time::SystemTime::now();
    let stamp = metadata
        .accessed()
        .ok()
        .or_else(|| metadata.modified().ok())?;
    now.duration_since(stamp)
        .ok()
        .map(|duration| duration.as_secs() / 86_400)
}

fn cleanup_score(
    size_bytes: u64,
    unused_days: Option<u64>,
    category: CleanupCategory,
    risk: RiskLevel,
) -> f32 {
    if risk == RiskLevel::High {
        return -100.0;
    }
    let size_gb = size_bytes as f32 / 1024.0 / 1024.0 / 1024.0;
    let mut score = size_gb * 0.7 + unused_days.unwrap_or(0) as f32 * 0.3;
    match category {
        CleanupCategory::Cache => score += 0.5,
        CleanupCategory::Installer => score += 0.3,
        CleanupCategory::System => score -= 100.0,
        _ => {}
    }
    score
}

fn cleanup_candidate_limit(risk: RiskLevel) -> usize {
    if risk == RiskLevel::High {
        MAX_BLOCKED_ITEMS_PER_CATEGORY
    } else {
        MAX_CLEANUP_ITEMS_PER_CATEGORY
    }
}

fn rank_cleanup_candidate(candidate: &CleanupCandidate) -> (i64, i64) {
    let score_key = (candidate.cleanup_score * 10.0).round() as i64;
    (score_key, candidate.target.size_bytes as i64)
}

fn push_ranked_cleanup_candidate(
    category_candidates: &mut HashMap<CleanupCategory, Vec<CleanupCandidate>>,
    candidate: CleanupCandidate,
) {
    let limit = cleanup_candidate_limit(candidate.risk);
    let bucket = category_candidates.entry(candidate.category).or_default();
    bucket.push(candidate);
    bucket.sort_by(|a, b| {
        rank_cleanup_candidate(b)
            .cmp(&rank_cleanup_candidate(a))
            .then_with(|| a.target.path.cmp(&b.target.path))
    });
    if bucket.len() > limit {
        bucket.truncate(limit);
    }
}
