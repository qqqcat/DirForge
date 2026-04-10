use crate::{cleanup, CleanupAnalysis, DeleteRequestScope, SelectedTarget};
use dirotter_actions::{
    execute_plan_with_progress, DeletionPlan, ExecutionMode, ExecutionProgress, ExecutionReport,
};
use dirotter_core::{NodeId, NodeStore, ScanErrorRecord, ScanSummary};
use dirotter_scan::RankedPath;
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) struct DeleteSession {
    pub(crate) relay: Arc<Mutex<DeleteRelayState>>,
}

pub(crate) struct DeleteFinalizeSession {
    pub(crate) relay: Arc<Mutex<DeleteFinalizeRelayState>>,
}

pub(crate) struct MemoryReleaseSession {
    pub(crate) relay: Arc<Mutex<MemoryReleaseRelayState>>,
}

pub(crate) struct ResultStoreLoadSession {
    pub(crate) relay: Arc<Mutex<ResultStoreLoadRelayState>>,
}

pub(crate) struct DuplicateScanSession {
    pub(crate) relay: Arc<Mutex<DuplicateScanRelayState>>,
}

pub(crate) struct DeleteRelayState {
    pub(crate) started_at: Instant,
    pub(crate) label: String,
    pub(crate) target_count: usize,
    pub(crate) mode: ExecutionMode,
    pub(crate) completed_count: usize,
    pub(crate) succeeded_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) current_path: Option<String>,
    pub(crate) finished: Option<DeleteFinishedPayload>,
}

pub(crate) struct DeleteFinalizeState {
    pub(crate) started_at: Instant,
    pub(crate) label: String,
    pub(crate) target_count: usize,
    pub(crate) mode: ExecutionMode,
    pub(crate) succeeded_count: usize,
    pub(crate) failed_count: usize,
}

#[derive(Clone)]
pub(crate) struct MemoryReleaseRelayState {
    pub(crate) finished: Option<
        Result<dirotter_platform::SystemMemoryReleaseReport, dirotter_platform::PlatformError>,
    >,
}

pub(crate) struct DeleteFinishedPayload {
    pub(crate) request: DeleteRequestScope,
    pub(crate) report: ExecutionReport,
}

pub(crate) struct DeleteFinalizePayload {
    pub(crate) report: ExecutionReport,
    pub(crate) store: Option<NodeStore>,
    pub(crate) summary: ScanSummary,
    pub(crate) cleanup_analysis: Option<CleanupAnalysis>,
    pub(crate) live_files: Vec<RankedPath>,
    pub(crate) live_top_files: Vec<RankedPath>,
    pub(crate) live_top_dirs: Vec<RankedPath>,
    pub(crate) completed_top_files: Vec<RankedPath>,
    pub(crate) completed_top_dirs: Vec<RankedPath>,
    pub(crate) errors: Vec<ScanErrorRecord>,
}

#[derive(Default)]
pub(crate) struct DeleteFinalizeRelayState {
    pub(crate) finished: Option<DeleteFinalizePayload>,
    pub(crate) snapshot: Option<DeleteFinalizeState>,
}

pub(crate) struct ResultStoreLoadPayload {
    pub(crate) root: String,
    pub(crate) store: Option<NodeStore>,
    pub(crate) summary: Option<ScanSummary>,
    pub(crate) cleanup_analysis: Option<CleanupAnalysis>,
    pub(crate) top_files: Vec<RankedPath>,
    pub(crate) top_dirs: Vec<RankedPath>,
}

#[derive(Default)]
pub(crate) struct ResultStoreLoadRelayState {
    pub(crate) finished: Option<ResultStoreLoadPayload>,
}

pub(crate) struct DuplicateScanPayload {
    pub(crate) groups: Vec<dirotter_dup::DuplicateGroup>,
}

#[derive(Clone, Default)]
pub(crate) struct DuplicateScanState {
    pub(crate) candidate_groups_total: usize,
    pub(crate) candidate_groups_processed: usize,
    pub(crate) groups_found: usize,
    pub(crate) duplicate_files_found: usize,
    pub(crate) reclaimable_bytes_found: u64,
}

#[derive(Default)]
pub(crate) struct DuplicateScanRelayState {
    pub(crate) finished: Option<DuplicateScanPayload>,
    pub(crate) snapshot: DuplicateScanState,
}

pub(crate) struct QueuedDeleteRequest {
    pub(crate) request: DeleteRequestScope,
    pub(crate) mode: ExecutionMode,
}

impl DeleteRelayState {
    pub(crate) fn new(request: &DeleteRequestScope, mode: ExecutionMode) -> Self {
        Self {
            started_at: Instant::now(),
            label: request.label.clone(),
            target_count: request.targets.len(),
            mode,
            completed_count: 0,
            succeeded_count: 0,
            failed_count: 0,
            current_path: None,
            finished: None,
        }
    }
}

impl DeleteSession {
    pub(crate) fn snapshot(&self) -> DeleteRelayState {
        let relay = self.relay.lock().expect("delete relay lock");
        DeleteRelayState {
            started_at: relay.started_at,
            label: relay.label.clone(),
            target_count: relay.target_count,
            mode: relay.mode,
            completed_count: relay.completed_count,
            succeeded_count: relay.succeeded_count,
            failed_count: relay.failed_count,
            current_path: relay.current_path.clone(),
            finished: None,
        }
    }
}

impl DeleteFinalizeSession {
    pub(crate) fn snapshot(&self) -> Option<DeleteFinalizeState> {
        let relay = self.relay.lock().expect("delete finalize relay lock");
        relay.snapshot.as_ref().map(|snapshot| DeleteFinalizeState {
            started_at: snapshot.started_at,
            label: snapshot.label.clone(),
            target_count: snapshot.target_count,
            mode: snapshot.mode,
            succeeded_count: snapshot.succeeded_count,
            failed_count: snapshot.failed_count,
        })
    }
}

impl DuplicateScanSession {
    pub(crate) fn snapshot(&self) -> DuplicateScanState {
        let relay = self.relay.lock().expect("duplicate scan relay lock");
        relay.snapshot.clone()
    }
}

pub(crate) fn start_memory_release_session(ctx: egui::Context) -> MemoryReleaseSession {
    let relay = Arc::new(Mutex::new(MemoryReleaseRelayState { finished: None }));
    let relay_state = Arc::clone(&relay);
    std::thread::spawn(move || {
        let result = dirotter_platform::release_system_memory();
        let mut state = relay_state.lock().expect("memory release relay lock");
        state.finished = Some(result);
        drop(state);
        ctx.request_repaint();
    });
    MemoryReleaseSession { relay }
}

pub(crate) fn take_finished_memory_release(
    session: &MemoryReleaseSession,
) -> Option<Result<dirotter_platform::SystemMemoryReleaseReport, dirotter_platform::PlatformError>>
{
    let mut relay = session.relay.lock().expect("memory release relay lock");
    relay.finished.take()
}

pub(crate) fn start_delete_session(
    ctx: egui::Context,
    request: DeleteRequestScope,
    plan: DeletionPlan,
    mode: ExecutionMode,
) -> DeleteSession {
    let relay = Arc::new(Mutex::new(DeleteRelayState::new(&request, mode)));
    let relay_state = Arc::clone(&relay);
    let progress_relay = Arc::clone(&relay);
    let progress_ctx = ctx.clone();
    std::thread::spawn(move || {
        let report = execute_plan_with_progress(&plan, mode, |progress: ExecutionProgress| {
            let mut state = progress_relay.lock().expect("delete relay lock");
            state.completed_count = progress.completed;
            state.succeeded_count = progress.succeeded;
            state.failed_count = progress.failed;
            state.current_path = Some(progress.item.path);
            drop(state);
            progress_ctx.request_repaint();
        });
        let mut state = relay_state.lock().expect("delete relay lock");
        state.finished = Some(DeleteFinishedPayload { request, report });
        drop(state);
        ctx.request_repaint();
    });
    DeleteSession { relay }
}

pub(crate) fn take_finished_delete(session: &DeleteSession) -> Option<DeleteFinishedPayload> {
    let mut relay = session.relay.lock().expect("delete relay lock");
    relay.finished.take()
}

pub(crate) struct DeleteFinalizeInput {
    pub(crate) started_at: Instant,
    pub(crate) label: String,
    pub(crate) target_count: usize,
    pub(crate) mode: ExecutionMode,
    pub(crate) succeeded_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) report: ExecutionReport,
    pub(crate) succeeded_targets: Vec<SelectedTarget>,
    pub(crate) summary: ScanSummary,
    pub(crate) store: Option<NodeStore>,
    pub(crate) cleanup_analysis: Option<CleanupAnalysis>,
    pub(crate) live_files: Vec<RankedPath>,
    pub(crate) live_top_files: Vec<RankedPath>,
    pub(crate) live_top_dirs: Vec<RankedPath>,
    pub(crate) completed_top_files: Vec<RankedPath>,
    pub(crate) completed_top_dirs: Vec<RankedPath>,
    pub(crate) errors: Vec<ScanErrorRecord>,
}

pub(crate) fn start_delete_finalize_session(
    ctx: egui::Context,
    input: DeleteFinalizeInput,
) -> DeleteFinalizeSession {
    let relay = Arc::new(Mutex::new(DeleteFinalizeRelayState {
        finished: None,
        snapshot: Some(DeleteFinalizeState {
            started_at: input.started_at,
            label: input.label.clone(),
            target_count: input.target_count,
            mode: input.mode,
            succeeded_count: input.succeeded_count,
            failed_count: input.failed_count,
        }),
    }));
    let relay_state = Arc::clone(&relay);
    std::thread::spawn(move || {
        let summary_before = input.summary.clone();
        let targets = input.succeeded_targets;
        let (
            store,
            summary,
            cleanup_analysis,
            live_top_files,
            live_top_dirs,
            completed_top_files,
            completed_top_dirs,
        ) = if let Some(store) = input.store {
            let store = rebuild_store_without_targets(&store, &targets);
            let summary = summarize_store(store.as_ref())
                .unwrap_or_else(|| subtract_summary(summary_before.clone(), &targets));
            let cleanup_analysis = store.as_ref().map(cleanup::build_cleanup_analysis);
            let (live_top_files, live_top_dirs, completed_top_files, completed_top_dirs) =
                if let Some(store) = store.as_ref() {
                    let top_files: Vec<RankedPath> = store
                        .top_n_largest_files(32)
                        .into_iter()
                        .map(|node| (node.path.clone(), node.size_self))
                        .collect();
                    let top_dirs: Vec<RankedPath> = store
                        .largest_dirs(32)
                        .into_iter()
                        .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
                        .collect();
                    (top_files.clone(), top_dirs.clone(), top_files, top_dirs)
                } else {
                    (Vec::new(), Vec::new(), Vec::new(), Vec::new())
                };
            (
                store,
                summary,
                cleanup_analysis,
                live_top_files,
                live_top_dirs,
                completed_top_files,
                completed_top_dirs,
            )
        } else {
            (
                None,
                subtract_summary(summary_before, &targets),
                prune_cleanup_analysis(input.cleanup_analysis, &targets),
                filter_ranked_paths(input.live_top_files, &targets),
                filter_ranked_paths(input.live_top_dirs, &targets),
                filter_ranked_paths(input.completed_top_files, &targets),
                filter_ranked_paths(input.completed_top_dirs, &targets),
            )
        };

        let payload = DeleteFinalizePayload {
            report: input.report,
            store,
            summary,
            cleanup_analysis,
            live_files: filter_ranked_paths(input.live_files, &targets),
            live_top_files,
            live_top_dirs,
            completed_top_files,
            completed_top_dirs,
            errors: input
                .errors
                .into_iter()
                .filter(|error| !path_matches_any_target(&error.path, &targets))
                .collect(),
        };
        let mut state = relay_state.lock().expect("delete finalize relay lock");
        state.finished = Some(payload);
        drop(state);
        ctx.request_repaint();
    });
    DeleteFinalizeSession { relay }
}

pub(crate) fn take_finished_delete_finalize(
    session: &DeleteFinalizeSession,
) -> Option<DeleteFinalizePayload> {
    let mut relay = session.relay.lock().expect("delete finalize relay lock");
    relay.finished.take()
}

pub(crate) fn start_result_store_load_session(
    ctx: egui::Context,
    session_root: PathBuf,
    root: String,
) -> ResultStoreLoadSession {
    let relay = Arc::new(Mutex::new(ResultStoreLoadRelayState::default()));
    let relay_state = Arc::clone(&relay);
    std::thread::spawn(move || {
        let loaded =
            dirotter_cache::CacheStore::load_snapshot_from_session_root(&session_root, &root);
        let payload = match loaded {
            Ok(Some(store)) => {
                let summary = summarize_store(Some(&store));
                let top_files: Vec<RankedPath> = store
                    .top_n_largest_files(32)
                    .into_iter()
                    .map(|node| (node.path.clone(), node.size_self))
                    .collect();
                let top_dirs: Vec<RankedPath> = store
                    .largest_dirs(32)
                    .into_iter()
                    .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
                    .collect();
                ResultStoreLoadPayload {
                    root,
                    cleanup_analysis: Some(cleanup::build_cleanup_analysis(&store)),
                    store: Some(store),
                    summary,
                    top_files,
                    top_dirs,
                }
            }
            _ => ResultStoreLoadPayload {
                root,
                store: None,
                summary: None,
                cleanup_analysis: None,
                top_files: Vec::new(),
                top_dirs: Vec::new(),
            },
        };
        let mut state = relay_state.lock().expect("result store load relay lock");
        state.finished = Some(payload);
        drop(state);
        ctx.request_repaint();
    });
    ResultStoreLoadSession { relay }
}

pub(crate) fn take_finished_result_store_load(
    session: &ResultStoreLoadSession,
) -> Option<ResultStoreLoadPayload> {
    let mut relay = session.relay.lock().expect("result store load relay lock");
    relay.finished.take()
}

pub(crate) fn start_duplicate_scan_session(
    ctx: egui::Context,
    store: NodeStore,
    candidates: Option<Vec<dirotter_dup::DuplicateSizeCandidate>>,
    cfg: dirotter_dup::DupConfig,
) -> DuplicateScanSession {
    let relay = Arc::new(Mutex::new(DuplicateScanRelayState::default()));
    let relay_state = Arc::clone(&relay);
    std::thread::spawn(move || {
        let mut last_emit = Instant::now();
        let mut last_processed = 0usize;
        let mut duplicate_files_found = 0usize;
        let mut reclaimable_bytes_found = 0u64;
        let candidates =
            candidates.unwrap_or_else(|| dirotter_dup::collect_review_candidates(&store, cfg));
        let groups = dirotter_dup::resolve_duplicates_with_progress(candidates, cfg, |progress| {
            duplicate_files_found += progress.latest_duplicate_files_found;
            reclaimable_bytes_found += progress.latest_reclaimable_bytes_found;

            let processed = progress.candidate_groups_processed;
            let should_emit = processed == progress.candidate_groups_total
                || processed.saturating_sub(last_processed) >= 128
                || last_emit.elapsed() >= Duration::from_millis(120);
            if !should_emit {
                return;
            }

            let mut state = relay_state.lock().expect("duplicate scan relay lock");
            state.snapshot = DuplicateScanState {
                candidate_groups_total: progress.candidate_groups_total,
                candidate_groups_processed: progress.candidate_groups_processed,
                groups_found: progress.groups_found,
                duplicate_files_found,
                reclaimable_bytes_found,
            };
            drop(state);
            last_emit = Instant::now();
            last_processed = processed;
            ctx.request_repaint();
        });

        let groups_found = groups.len();
        let duplicate_files_found = groups.iter().map(|group| group.files.len()).sum();
        let reclaimable_bytes_found = groups.iter().map(|group| group.total_waste).sum();

        let mut state = relay_state.lock().expect("duplicate scan relay lock");
        state.finished = Some(DuplicateScanPayload { groups });
        state.snapshot.groups_found = groups_found;
        state.snapshot.duplicate_files_found = duplicate_files_found;
        state.snapshot.reclaimable_bytes_found = reclaimable_bytes_found;
        if state.snapshot.candidate_groups_total > 0 {
            state.snapshot.candidate_groups_processed = state.snapshot.candidate_groups_total;
        }
        drop(state);
        ctx.request_repaint();
    });
    DuplicateScanSession { relay }
}

pub(crate) fn take_finished_duplicate_scan(
    session: &DuplicateScanSession,
) -> Option<DuplicateScanPayload> {
    let mut relay = session.relay.lock().expect("duplicate scan relay lock");
    relay.finished.take()
}

fn filter_ranked_paths(items: Vec<RankedPath>, targets: &[SelectedTarget]) -> Vec<RankedPath> {
    items
        .into_iter()
        .filter(|(path, _)| !path_matches_any_target(path.as_ref(), targets))
        .collect()
}

fn path_matches_target(path: &str, target: &SelectedTarget) -> bool {
    if path == target.path.as_ref() {
        return true;
    }
    if !matches!(target.kind, dirotter_core::NodeKind::Dir) {
        return false;
    }
    let Some(rest) = path.strip_prefix(target.path.as_ref()) else {
        return false;
    };
    rest.starts_with('\\') || rest.starts_with('/')
}

fn path_matches_any_target(path: &str, targets: &[SelectedTarget]) -> bool {
    targets
        .iter()
        .any(|target| path_matches_target(path, target))
}

fn rebuild_store_without_targets(
    store: &NodeStore,
    targets: &[SelectedTarget],
) -> Option<NodeStore> {
    let mut next = NodeStore::default();
    let mut id_map: std::collections::HashMap<NodeId, NodeId> = std::collections::HashMap::new();

    for node in &store.nodes {
        let node_path = store.node_path(node);
        if path_matches_any_target(node_path, targets) {
            continue;
        }
        let parent = node.parent.and_then(|old_id| id_map.get(&old_id).copied());
        let new_id = next.add_node(
            parent,
            store.node_name(node).to_string(),
            node_path.to_string(),
            node.kind,
            node.size_self,
        );
        id_map.insert(node.id, new_id);
    }

    if next.nodes.is_empty() {
        return None;
    }

    next.rollup();
    Some(next)
}

fn summarize_store(store: Option<&NodeStore>) -> Option<ScanSummary> {
    let store = store?;
    let root = store.nodes.iter().find(|node| node.parent.is_none())?;
    Some(ScanSummary {
        scanned_files: root.file_count,
        scanned_dirs: root.dir_count,
        bytes_observed: root.size_subtree.max(root.size_self),
        error_count: 0,
    })
}

fn subtract_summary(summary: ScanSummary, targets: &[SelectedTarget]) -> ScanSummary {
    let released_bytes = targets.iter().map(|target| target.size_bytes).sum();
    let released_files = targets.iter().map(|target| target.file_count).sum();
    let released_dirs = targets.iter().map(|target| target.dir_count).sum();
    ScanSummary {
        scanned_files: summary.scanned_files.saturating_sub(released_files),
        scanned_dirs: summary.scanned_dirs.saturating_sub(released_dirs),
        bytes_observed: summary.bytes_observed.saturating_sub(released_bytes),
        error_count: summary.error_count,
    }
}

fn prune_cleanup_analysis(
    analysis: Option<CleanupAnalysis>,
    targets: &[SelectedTarget],
) -> Option<CleanupAnalysis> {
    let mut analysis = analysis?;
    analysis
        .items
        .retain(|item| !path_matches_any_target(item.target.path.as_ref(), targets));
    let mut category_map: std::collections::HashMap<
        crate::cleanup::CleanupCategory,
        crate::cleanup::CleanupCategorySummary,
    > = std::collections::HashMap::new();
    let mut reclaimable_bytes = 0u64;
    let mut quick_clean_bytes = 0u64;
    for item in &analysis.items {
        let summary = category_map.entry(item.category).or_insert_with(|| {
            crate::cleanup::CleanupCategorySummary {
                category: item.category,
                total_bytes: 0,
                reclaimable_bytes: 0,
                blocked_bytes: 0,
                item_count: 0,
            }
        });
        summary.total_bytes = summary.total_bytes.saturating_add(item.target.size_bytes);
        summary.item_count += 1;
        if item.risk == dirotter_core::RiskLevel::High {
            summary.blocked_bytes = summary.blocked_bytes.saturating_add(item.target.size_bytes);
        } else {
            summary.reclaimable_bytes = summary
                .reclaimable_bytes
                .saturating_add(item.target.size_bytes);
            reclaimable_bytes = reclaimable_bytes.saturating_add(item.target.size_bytes);
            if item.category == crate::cleanup::CleanupCategory::Cache
                && item.risk == dirotter_core::RiskLevel::Low
            {
                quick_clean_bytes = quick_clean_bytes.saturating_add(item.target.size_bytes);
            }
        }
    }
    let mut categories: Vec<_> = category_map.into_values().collect();
    categories.sort_by(|a, b| {
        b.reclaimable_bytes
            .cmp(&a.reclaimable_bytes)
            .then_with(|| b.total_bytes.cmp(&a.total_bytes))
    });
    analysis.categories = categories;
    analysis.reclaimable_bytes = reclaimable_bytes;
    analysis.quick_clean_bytes = quick_clean_bytes;
    Some(analysis)
}
