use dirotter_actions::{
    build_deletion_plan_with_origin, execute_plan, ActionFailureKind, ExecutionMode,
    ExecutionReport, SelectionOrigin,
};
use dirotter_cache::{CacheStore, HistoryRecord};
use dirotter_core::{
    ErrorKind, Node, NodeId, NodeKind, NodeStore, RiskLevel, ScanErrorRecord, ScanProfile,
    ScanSummary, SnapshotDelta,
};
use dirotter_report::{
    default_manifest, export_diagnostics_archive, export_diagnostics_bundle, export_errors_csv,
};
use dirotter_scan::{start_scan, BatchEntry, ScanConfig, ScanEvent};
use dirotter_telemetry as telemetry;
use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

const MAX_PENDING_BATCH_EVENTS: usize = 32;
const MAX_PENDING_SNAPSHOTS: usize = 8;
const MAX_LIVE_FILES: usize = 20_000;
const NAV_WIDTH: f32 = 188.0;
const INSPECTOR_WIDTH: f32 = 300.0;
const TOOLBAR_HEIGHT: f32 = 56.0;
const STATUSBAR_HEIGHT: f32 = 26.0;
const SHELL_RADIUS: u8 = 0;
const CARD_RADIUS: u8 = 14;
const CONTROL_RADIUS: u8 = 10;
const CARD_PADDING: f32 = 14.0;
const CARD_STROKE_WIDTH: f32 = 1.0;
const CONTROL_HEIGHT: f32 = 34.0;
const PRIMARY_BUTTON_HEIGHT: f32 = 40.0;
const NAV_ITEM_HEIGHT: f32 = 36.0;
const STATUS_BADGE_HEIGHT: f32 = 32.0;
const CONTROL_MIN_WIDTH: f32 = 56.0;
const PAGE_MAX_WIDTH: f32 = 1360.0;
const MIN_TREEMAP_TILE_EDGE: f32 = 16.0;
const MIN_TREEMAP_LABEL_WIDTH: f32 = 84.0;
const MIN_TREEMAP_LABEL_HEIGHT: f32 = 30.0;
const TREEMAP_TILE_LIMIT: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Dashboard,
    CurrentScan,
    Treemap,
    History,
    Errors,
    Diagnostics,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    En,
    Zh,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppStatus {
    Idle,
    Scanning,
    Completed,
    Deleting,
    DeleteExecuted,
    DeleteFailed,
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectionSource {
    Treemap,
    Table,
    History,
    Error,
}

#[derive(Default, Clone)]
struct SelectionState {
    selected_node: Option<NodeId>,
    selected_path: Option<String>,
    source: Option<SelectionSource>,
}

#[derive(Default)]
struct PerfMetrics {
    frame_ms: f32,
    snapshot_queue_depth: usize,
    avg_snapshot_commit_ms: u64,
    avg_scan_batch_size: u64,
    last_update: Option<Instant>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ErrorFilter {
    All,
    User,
    Transient,
    System,
}

#[derive(Clone)]
struct TreemapTile {
    node_id: NodeId,
    rect: egui::Rect,
    label: String,
    size_bytes: u64,
    path: String,
}

#[derive(Default, Clone)]
struct TreemapViewportCache {
    key: Option<(u32, u32, usize)>,
    tiles: Vec<TreemapTile>,
}

struct ScanSession {
    cancel: Arc<AtomicBool>,
    relay: Arc<Mutex<ScanRelayState>>,
}

struct ScanRelayState {
    latest_progress: Option<dirotter_scan::ScanProgress>,
    pending_batches: VecDeque<Vec<BatchEntry>>,
    latest_snapshot: Option<(SnapshotDelta, dirotter_scan::SnapshotView)>,
    finished: Option<FinishedPayload>,
    last_event_at: Instant,
    dropped_batches: u64,
    dropped_snapshots: u64,
    dropped_progress: u64,
}

struct FinishedPayload {
    summary: ScanSummary,
    errors: Vec<ScanErrorRecord>,
    top_files: Vec<(String, u64)>,
    top_dirs: Vec<(String, u64)>,
}

#[derive(Clone)]
struct SelectedTarget {
    name: String,
    path: String,
    size_bytes: u64,
    kind: NodeKind,
    file_count: u64,
    dir_count: u64,
}

#[derive(Clone)]
struct PendingDeleteConfirmation {
    target: SelectedTarget,
    risk: RiskLevel,
}

struct DeleteSession {
    relay: Arc<Mutex<DeleteRelayState>>,
}

struct DeleteRelayState {
    started_at: Instant,
    target_path: String,
    mode: ExecutionMode,
    finished: Option<DeleteFinishedPayload>,
}

struct DeleteFinishedPayload {
    target: SelectedTarget,
    report: ExecutionReport,
}

struct QueuedDeleteRequest {
    target: SelectedTarget,
    mode: ExecutionMode,
}

impl DeleteRelayState {
    fn new(target: &SelectedTarget, mode: ExecutionMode) -> Self {
        Self {
            started_at: Instant::now(),
            target_path: target.path.clone(),
            mode,
            finished: None,
        }
    }
}

impl DeleteSession {
    fn snapshot(&self) -> DeleteRelayState {
        let relay = self.relay.lock().expect("delete relay lock");
        DeleteRelayState {
            started_at: relay.started_at,
            target_path: relay.target_path.clone(),
            mode: relay.mode,
            finished: None,
        }
    }
}

impl Default for ScanRelayState {
    fn default() -> Self {
        Self {
            latest_progress: None,
            pending_batches: VecDeque::new(),
            latest_snapshot: None,
            finished: None,
            last_event_at: Instant::now(),
            dropped_batches: 0,
            dropped_snapshots: 0,
            dropped_progress: 0,
        }
    }
}

pub struct DirOtterNativeApp {
    egui_ctx: egui::Context,
    page: Page,
    available_volumes: Vec<dirotter_platform::VolumeInfo>,
    root_input: String,
    status: AppStatus,
    summary: ScanSummary,
    store: Option<NodeStore>,
    scan_session: Option<ScanSession>,
    delete_session: Option<DeleteSession>,
    scan_profile: ScanProfile,
    snapshot_interval_ms: u64,
    event_batch_size: usize,
    scan_current_path: Option<String>,
    scan_last_event_at: Option<Instant>,
    scan_dropped_batches: u64,
    scan_dropped_snapshots: u64,
    scan_dropped_progress: u64,

    pending_batch_events: VecDeque<Vec<BatchEntry>>,
    pending_snapshots: VecDeque<SnapshotDelta>,
    live_files: Vec<(String, u64)>,
    live_top_files: Vec<(String, u64)>,
    live_top_dirs: Vec<(String, u64)>,
    completed_top_files: Vec<(String, u64)>,
    completed_top_dirs: Vec<(String, u64)>,
    last_coalesce_commit: Instant,

    execution_report: Option<ExecutionReport>,
    pending_delete_confirmation: Option<PendingDeleteConfirmation>,
    queued_delete: Option<QueuedDeleteRequest>,
    explorer_feedback: Option<(String, bool)>,

    history: Vec<HistoryRecord>,
    errors: Vec<ScanErrorRecord>,
    selected_history_id: Option<i64>,

    language: Lang,
    theme_dark: bool,
    cache: CacheStore,

    perf: PerfMetrics,
    diagnostics_json: String,
    selection: SelectionState,
    error_filter: ErrorFilter,
    treemap_cache: TreemapViewportCache,
}

impl DirOtterNativeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        let cache = CacheStore::new("dirotter.db").expect("open sqlite cache");
        let language = cache
            .get_setting("language")
            .ok()
            .flatten()
            .map(|v| if v == "zh" { Lang::Zh } else { Lang::En })
            .unwrap_or_else(detect_lang);
        let theme_dark = cache
            .get_setting("theme")
            .ok()
            .flatten()
            .map(|v| v != "light")
            .unwrap_or(true);
        let available_volumes = dirotter_platform::list_volumes().unwrap_or_default();
        let initial_root = preferred_root_from_volumes(&available_volumes);

        let mut app = Self {
            egui_ctx: cc.egui_ctx.clone(),
            page: Page::Dashboard,
            available_volumes,
            root_input: initial_root,
            status: AppStatus::Idle,
            summary: ScanSummary::default(),
            store: None,
            scan_session: None,
            delete_session: None,
            scan_profile: ScanProfile::Ssd,
            snapshot_interval_ms: 75,
            event_batch_size: 256,
            scan_current_path: None,
            scan_last_event_at: None,
            scan_dropped_batches: 0,
            scan_dropped_snapshots: 0,
            scan_dropped_progress: 0,
            pending_batch_events: VecDeque::new(),
            pending_snapshots: VecDeque::new(),
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            execution_report: None,
            pending_delete_confirmation: None,
            queued_delete: None,
            explorer_feedback: None,
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language,
            theme_dark,
            cache,
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
            treemap_cache: TreemapViewportCache::default(),
        };

        let _ = app.reload_history();
        if let Ok(Some(snapshot)) = app.cache.load_latest_snapshot(&app.root_input) {
            app.store = Some(snapshot);
        }
        app.apply_theme(&cc.egui_ctx);
        app.refresh_diagnostics();
        app
    }

    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        match self.language {
            Lang::Zh => zh,
            Lang::En => en,
        }
    }

    fn status_text(&self) -> &'static str {
        match self.status {
            AppStatus::Idle => self.t("空闲", "Idle"),
            AppStatus::Scanning => self.t("扫描中", "Scanning"),
            AppStatus::Completed => self.t("完成", "Completed"),
            AppStatus::Deleting => self.t("删除中", "Deleting"),
            AppStatus::DeleteExecuted => self.t("删除已执行", "Delete executed"),
            AppStatus::DeleteFailed => self.t("删除失败", "Delete failed"),
            AppStatus::Cancelled => self.t("已取消", "Cancelled"),
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(16.0);
        style.spacing.menu_margin = egui::Margin::same(10.0);
        style.spacing.indent = 18.0;
        style.spacing.combo_width = 120.0;
        style.visuals = if self.theme_dark {
            build_dark_visuals()
        } else {
            build_light_visuals()
        };
        style.visuals.widgets.noninteractive.rounding =
            egui::Rounding::same(CONTROL_RADIUS as f32);
        style.visuals.widgets.inactive.rounding = egui::Rounding::same(CONTROL_RADIUS as f32);
        style.visuals.widgets.hovered.rounding = egui::Rounding::same(CONTROL_RADIUS as f32);
        style.visuals.widgets.active.rounding = egui::Rounding::same(CONTROL_RADIUS as f32);
        style.visuals.widgets.open.rounding = egui::Rounding::same(CONTROL_RADIUS as f32);
        style.visuals.widgets.noninteractive.expansion = 0.0;
        style.visuals.widgets.inactive.expansion = 0.0;
        style.visuals.widgets.hovered.expansion = 0.0;
        style.visuals.widgets.active.expansion = 0.0;
        style.visuals.widgets.open.expansion = 0.0;
        style.text_styles = [
            (
                egui::TextStyle::Heading,
                egui::FontId::new(24.0, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Name("title".into()),
                egui::FontId::new(18.0, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Body,
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Button,
                egui::FontId::new(13.5, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Monospace,
                egui::FontId::new(13.0, egui::FontFamily::Monospace),
            ),
            (
                egui::TextStyle::Small,
                egui::FontId::new(12.0, egui::FontFamily::Proportional),
            ),
        ]
        .into();
        ctx.set_style(style);
    }

    fn summary_cards(&self) -> Vec<(String, String, String)> {
        let mut cards = vec![
            (
                self.t("文件", "Files").to_string(),
                format_count(self.summary.scanned_files),
                self.t("已发现文件数", "Discovered files").to_string(),
            ),
            (
                self.t("目录", "Directories").to_string(),
                format_count(self.summary.scanned_dirs),
                self.t("已遍历目录数", "Traversed directories").to_string(),
            ),
            (
                self.t("扫描体积", "Scanned Size").to_string(),
                format_bytes(self.summary.bytes_observed),
                self.t(
                    "仅统计已扫描到的文件体积",
                    "Only the file bytes actually scanned",
                )
                .to_string(),
            ),
        ];

        if let Some(volume) = self.current_volume_info() {
            let used = volume.total_bytes.saturating_sub(volume.available_bytes);
            cards.push((
                self.t("磁盘已用", "Volume Used").to_string(),
                format_bytes(used),
                format!(
                    "{} {}  |  {} {}",
                    format_bytes(volume.total_bytes),
                    self.t("总容量", "total"),
                    format_bytes(volume.available_bytes),
                    self.t("可用", "free")
                ),
            ));
        }

        cards.push((
            self.t("错误", "Errors").to_string(),
            format_count(self.summary.error_count),
            self.t("需要关注的问题项", "Items needing attention")
                .to_string(),
        ));

        cards
    }

    fn selected_node(&self) -> Option<&Node> {
        let store = self.store.as_ref()?;
        let node_id = self.selection.selected_node?;
        store.nodes.get(node_id.0)
    }

    fn selected_target(&self) -> Option<SelectedTarget> {
        if let Some(node) = self.selected_node() {
            return Some(SelectedTarget {
                name: node.name.clone(),
                path: node.path.clone(),
                size_bytes: node.size_subtree.max(node.size_self),
                kind: node.kind,
                file_count: node.file_count,
                dir_count: node.dir_count,
            });
        }

        let path = self.selection.selected_path.clone()?;
        let metadata = fs::metadata(&path).ok();
        let kind = if metadata.as_ref().is_some_and(|meta| meta.is_dir()) {
            NodeKind::Dir
        } else {
            NodeKind::File
        };
        let size_bytes = metadata
            .as_ref()
            .map(|meta| if meta.is_file() { meta.len() } else { 0 })
            .unwrap_or(0);
        let name = PathBuf::from(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.clone());
        Some(SelectedTarget {
            name,
            path,
            size_bytes,
            kind,
            file_count: if kind == NodeKind::File { 1 } else { 0 },
            dir_count: if kind == NodeKind::Dir { 1 } else { 0 },
        })
    }

    fn selection_origin(&self) -> SelectionOrigin {
        match self.selection.source {
            Some(SelectionSource::Table | SelectionSource::Treemap) => SelectionOrigin::TopFiles,
            Some(SelectionSource::History | SelectionSource::Error) | None => {
                SelectionOrigin::Manual
            }
        }
    }

    fn risk_for_path(&self, path: &str) -> RiskLevel {
        let lower = path.to_ascii_lowercase();
        if lower.contains("\\windows")
            || lower.contains("\\program files")
            || lower.contains("\\programdata")
            || lower.contains("\\system volume information")
            || lower.contains("\\$recycle.bin")
        {
            RiskLevel::High
        } else if lower.contains("\\appdata") || lower.ends_with(":\\") {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }

    fn start_scan_for_root(&mut self, root: String) {
        self.root_input = root;
        self.page = Page::CurrentScan;
        self.start_scan();
    }

    fn delete_feedback_message(&self) -> Option<(String, String, bool)> {
        let report = self.execution_report.as_ref()?;
        let item = report.items.first()?;
        if item.success {
            return Some(match report.mode {
                ExecutionMode::RecycleBin => (
                    self.t("已移到回收站", "Moved to Recycle Bin").to_string(),
                    self.t(
                        "可在系统回收站中恢复该项目。",
                        "You can restore this item from the system recycle bin.",
                    )
                    .to_string(),
                    true,
                ),
                ExecutionMode::Permanent => (
                    self.t("已永久删除", "Deleted Permanently").to_string(),
                    self.t(
                        "该操作已执行，当前版本不提供撤销。",
                        "This action has been executed and cannot be undone in the current build.",
                    )
                    .to_string(),
                    true,
                ),
            });
        }

        let hint = match item.failure_kind {
            Some(ActionFailureKind::PermissionDenied) => self.t(
                "权限不足。请检查目标是否为系统目录，或使用更高权限重试。",
                "Permission denied. Check whether the target is protected or retry with higher privileges.",
            ),
            Some(ActionFailureKind::Protected) => self.t(
                "该目标被风险策略拦截，建议优先使用回收站删除或重新评估路径。",
                "This target was blocked by risk protection. Prefer recycle-bin deletion or review the path.",
            ),
            Some(ActionFailureKind::Io) => self.t(
                "文件或目录可能正被占用。关闭相关程序后重试。",
                "The file or directory may be in use. Close related programs and try again.",
            ),
            Some(ActionFailureKind::Missing) => self.t(
                "目标已不存在，界面会在下一次刷新后自动同步。",
                "The target no longer exists. The UI will synchronize on the next refresh.",
            ),
            Some(ActionFailureKind::PlatformUnavailable | ActionFailureKind::NotSupported) => {
                self.t(
                    "当前平台不支持该操作，建议改用回收站删除或系统文件管理器。",
                    "This operation is not supported on the current platform. Try recycle-bin deletion or the system file manager.",
                )
            }
            Some(ActionFailureKind::PrecheckMismatch) => self.t(
                "预检查与执行前状态不一致，建议重新选择该对象后重试。",
                "Precheck no longer matches current state. Re-select the item and try again.",
            ),
            Some(ActionFailureKind::UnsupportedType) => self.t(
                "当前只支持文件和目录，特殊对象请改用系统工具处理。",
                "Only files and directories are supported. Use system tools for special objects.",
            ),
            None => self.t(
                "删除执行失败，请查看下方消息并重试。",
                "Delete action failed. Review the message below and try again.",
            ),
        };

        Some((
            self.t("删除失败", "Delete Failed").to_string(),
            format!("{} {}", item.message, hint),
            false,
        ))
    }

    fn path_matches_target(path: &str, target: &SelectedTarget) -> bool {
        if path == target.path {
            return true;
        }
        if target.kind != NodeKind::Dir {
            return false;
        }
        let Some(rest) = path.strip_prefix(&target.path) else {
            return false;
        };
        rest.starts_with('\\') || rest.starts_with('/')
    }

    fn retain_existing_ranked_items(
        items: &[(String, u64)],
        limit: usize,
        include_dirs: bool,
    ) -> Vec<(String, u64)> {
        items
            .iter()
            .filter(|(path, _)| {
                fs::metadata(path)
                    .map(|meta| {
                        if include_dirs {
                            meta.is_dir()
                        } else {
                            meta.is_file()
                        }
                    })
                    .unwrap_or(false)
            })
            .take(limit)
            .cloned()
            .collect()
    }

    fn rebuild_store_without_target(
        store: &NodeStore,
        target: &SelectedTarget,
    ) -> Option<NodeStore> {
        let mut next = NodeStore::default();
        let mut id_map: HashMap<NodeId, NodeId> = HashMap::new();

        for node in &store.nodes {
            if Self::path_matches_target(&node.path, target) {
                continue;
            }
            let parent = node.parent.and_then(|old_id| id_map.get(&old_id).copied());
            let new_id = next.add_node(
                parent,
                node.name.clone(),
                node.path.clone(),
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

    fn sync_summary_from_store(&mut self) {
        if let Some(store) = self.store.as_ref() {
            if let Some(root) = store.nodes.iter().find(|node| node.parent.is_none()) {
                self.summary.scanned_files = root.file_count;
                self.summary.scanned_dirs = root.dir_count;
                self.summary.bytes_observed = root.size_subtree.max(root.size_self);
            }
        } else {
            self.summary.scanned_files = 0;
            self.summary.scanned_dirs = 0;
            self.summary.bytes_observed = 0;
        }
    }

    fn sync_rankings_from_store(&mut self) {
        let Some(store) = self.store.as_ref() else {
            self.live_top_files.clear();
            self.live_top_dirs.clear();
            self.completed_top_files.clear();
            self.completed_top_dirs.clear();
            return;
        };

        let top_files: Vec<(String, u64)> = store
            .top_n_largest_files(32)
            .into_iter()
            .map(|node| (node.path.clone(), node.size_self))
            .collect();
        let top_dirs: Vec<(String, u64)> = store
            .largest_dirs(32)
            .into_iter()
            .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
            .collect();

        self.live_top_files = top_files.clone();
        self.live_top_dirs = top_dirs.clone();
        self.completed_top_files = top_files;
        self.completed_top_dirs = top_dirs;
    }

    fn prune_deleted_target(&mut self, target: &SelectedTarget) {
        let matches_target = |path: &str| -> bool { Self::path_matches_target(path, target) };

        self.live_files.retain(|(path, _)| !matches_target(path));
        self.live_top_files
            .retain(|(path, _)| !matches_target(path));
        self.live_top_dirs.retain(|(path, _)| !matches_target(path));
        self.completed_top_files
            .retain(|(path, _)| !matches_target(path));
        self.completed_top_dirs
            .retain(|(path, _)| !matches_target(path));
        self.errors.retain(|error| !matches_target(&error.path));
        if let Some(store) = self.store.clone() {
            self.store = Self::rebuild_store_without_target(&store, target);
            self.sync_summary_from_store();
            self.sync_rankings_from_store();
            self.treemap_cache = TreemapViewportCache::default();
        } else {
            self.summary.bytes_observed = self
                .summary
                .bytes_observed
                .saturating_sub(target.size_bytes);
            self.summary.scanned_files =
                self.summary.scanned_files.saturating_sub(target.file_count);
            self.summary.scanned_dirs = self.summary.scanned_dirs.saturating_sub(target.dir_count);
        }
        self.selection = SelectionState::default();
    }

    fn execute_selected_delete(&mut self, mode: ExecutionMode) {
        let Some(target) = self.selected_target() else {
            return;
        };
        self.queue_delete_for_target(target, mode);
    }

    fn queue_delete_for_target(&mut self, target: SelectedTarget, mode: ExecutionMode) {
        self.pending_delete_confirmation = None;
        self.execution_report = None;
        self.queued_delete = Some(QueuedDeleteRequest { target, mode });
        self.egui_ctx.request_repaint();
    }

    fn process_queued_delete(&mut self) {
        if self.delete_session.is_some() {
            return;
        }
        let Some(request) = self.queued_delete.take() else {
            return;
        };
        self.execute_delete_for_target(request.target, request.mode);
    }

    fn execute_delete_for_target(&mut self, target: SelectedTarget, mode: ExecutionMode) {
        let plan = build_deletion_plan_with_origin(
            vec![(
                target.path.clone(),
                target.size_bytes,
                self.risk_for_path(&target.path),
            )],
            self.selection_origin(),
        );
        let relay = Arc::new(Mutex::new(DeleteRelayState::new(&target, mode)));
        let relay_state = Arc::clone(&relay);
        let ctx = self.egui_ctx.clone();
        self.pending_delete_confirmation = None;
        self.execution_report = None;
        self.explorer_feedback = None;
        self.status = AppStatus::Deleting;
        self.delete_session = Some(DeleteSession { relay });
        self.egui_ctx.request_repaint();

        std::thread::spawn(move || {
            let report = execute_plan(&plan, mode);
            let mut state = relay_state.lock().expect("delete relay lock");
            state.finished = Some(DeleteFinishedPayload { target, report });
            drop(state);
            ctx.request_repaint();
        });
    }

    fn delete_active(&self) -> bool {
        self.delete_session.is_some()
    }

    fn process_delete_events(&mut self) {
        let Some(session) = &self.delete_session else {
            return;
        };

        let finished = {
            let mut relay = session.relay.lock().expect("delete relay lock");
            relay.finished.take()
        };

        let Some(payload) = finished else {
            return;
        };

        let report = payload.report;
        let audit_payload = serde_json::json!({
            "path": payload.target.path,
            "mode": format!("{:?}", report.mode),
            "attempted": report.attempted,
            "succeeded": report.succeeded,
            "failed": report.failed,
        })
        .to_string();
        let _ = self.cache.add_audit_event("delete_execute", &audit_payload);
        if report.succeeded > 0 {
            self.prune_deleted_target(&payload.target);
            self.status = AppStatus::DeleteExecuted;
        } else {
            self.status = AppStatus::DeleteFailed;
        }
        self.execution_report = Some(report);
        self.delete_session = None;
        self.refresh_diagnostics();
    }

    fn source_label(&self, source: SelectionSource) -> &'static str {
        match source {
            SelectionSource::Treemap => self.t("矩形树图", "Treemap"),
            SelectionSource::Table => self.t("列表", "Table"),
            SelectionSource::History => self.t("历史", "History"),
            SelectionSource::Error => self.t("错误", "Error"),
        }
    }

    fn select_path(&mut self, path: &str, source: SelectionSource) {
        self.selection.selected_path = Some(path.to_string());
        self.selection.source = Some(source);
        self.selection.selected_node = self
            .store
            .as_ref()
            .and_then(|store| store.path_index.get(path).copied());
        self.execution_report = None;
        self.pending_delete_confirmation = None;
        self.explorer_feedback = None;
    }

    fn current_volume_info(&self) -> Option<dirotter_platform::VolumeInfo> {
        dirotter_platform::volume_info(&self.root_input).ok()
    }

    fn volume_numbers(&self) -> Option<(u64, u64, u64)> {
        let volume = self.current_volume_info()?;
        let used = volume.total_bytes.saturating_sub(volume.available_bytes);
        Some((used, volume.available_bytes, volume.total_bytes))
    }

    fn scanned_coverage_ratio(&self) -> Option<f32> {
        let (used, _, _) = self.volume_numbers()?;
        if used == 0 {
            return None;
        }
        Some(self.summary.bytes_observed as f32 / used as f32)
    }

    fn scan_active(&self) -> bool {
        self.scan_session.is_some()
    }

    fn scan_health_summary(&self) -> String {
        let age = self
            .scan_last_event_at
            .map(|instant| instant.elapsed().as_secs_f32())
            .unwrap_or_default();
        format!(
            "{} {:.1}s  |  {} {}  |  {} {}  |  {} {}",
            self.t("最近事件", "Last event"),
            age,
            self.t("丢弃进度", "Dropped progress"),
            format_count(self.scan_dropped_progress),
            self.t("丢弃批次", "Dropped batches"),
            format_count(self.scan_dropped_batches),
            self.t("丢弃快照", "Dropped snapshots"),
            format_count(self.scan_dropped_snapshots),
        )
    }

    fn scan_health_short(&self) -> String {
        let age = self
            .scan_last_event_at
            .map(|instant| instant.elapsed().as_secs_f32())
            .unwrap_or_default();
        let path = self
            .scan_current_path
            .as_deref()
            .map(|path| truncate_middle(path, 46))
            .unwrap_or_else(|| self.t("准备中", "Preparing").to_string());
        format!(
            "{} {:.1}s  |  {}",
            self.t("最近事件", "Last event"),
            age,
            path
        )
    }

    fn current_ranked_dirs(&self, limit: usize) -> Vec<(String, u64)> {
        if self.scan_active() && !self.live_top_dirs.is_empty() {
            return Self::retain_existing_ranked_items(&self.live_top_dirs, limit, true);
        }
        if !self.scan_active() && !self.completed_top_dirs.is_empty() {
            return Self::retain_existing_ranked_items(&self.completed_top_dirs, limit, true);
        }

        self.store
            .as_ref()
            .map(|store| {
                store
                    .largest_dirs(limit)
                    .into_iter()
                    .filter(|node| {
                        fs::metadata(&node.path)
                            .map(|meta| meta.is_dir())
                            .unwrap_or(false)
                    })
                    .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
                    .take(limit)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn current_ranked_files(&self, limit: usize) -> Vec<(String, u64)> {
        if self.scan_active() && !self.live_top_files.is_empty() {
            return Self::retain_existing_ranked_items(&self.live_top_files, limit, false);
        }
        if !self.scan_active() && !self.completed_top_files.is_empty() {
            return Self::retain_existing_ranked_items(&self.completed_top_files, limit, false);
        }

        self.store
            .as_ref()
            .map(|store| {
                store
                    .top_n_largest_files(limit)
                    .into_iter()
                    .filter(|node| {
                        fs::metadata(&node.path)
                            .map(|meta| meta.is_file())
                            .unwrap_or(false)
                    })
                    .map(|node| (node.path.clone(), node.size_self))
                    .take(limit)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn ranked_files_in_scope(&self, scope_path: &str, limit: usize) -> Vec<(String, u64)> {
        let Some(store) = self.store.as_ref() else {
            return Vec::new();
        };
        let mut matches: Vec<(String, u64)> = store
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::File))
            .filter(|node| node.path != scope_path)
            .filter(|node| path_within_scope(&node.path, scope_path))
            .map(|node| (node.path.clone(), node.size_self))
            .collect();
        matches.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        matches.truncate(limit);
        matches
    }

    fn contextual_ranked_files_panel(&self, limit: usize) -> (String, String, Vec<(String, u64)>) {
        if let Some(target) = self.selected_target() {
            let scope_path = match target.kind {
                NodeKind::Dir => Some(target.path.clone()),
                NodeKind::File => PathBuf::from(&target.path)
                    .parent()
                    .map(|parent| parent.display().to_string()),
            };

            if let Some(scope_path) = scope_path {
                let scoped_files = self.ranked_files_in_scope(&scope_path, limit);
                if !scoped_files.is_empty() {
                    let scope_name = PathBuf::from(&scope_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| scope_path.clone());
                    return (
                        self.t("所选位置中的最大文件", "Largest Files In Selection")
                            .to_string(),
                        format!(
                            "{}: {}",
                            self.t("当前范围", "Current scope"),
                            truncate_middle(&scope_name, 40)
                        ),
                        scoped_files,
                    );
                }
            }
        }

        (
            self.t("当前最大的文件", "Largest Files Found So Far")
                .to_string(),
            self.t(
                "先发现的结果不代表最终排序。",
                "Early findings are not yet the final ordering.",
            )
            .to_string(),
            self.current_ranked_files(limit),
        )
    }

    fn refresh_diagnostics(&mut self) {
        let cache_payload = self
            .cache
            .export_diagnostics_json()
            .unwrap_or_else(|_| "{}".to_string());
        let telemetry_snapshot = telemetry::snapshot();
        let system_snapshot = telemetry::system_snapshot();
        let metrics = telemetry::metric_descriptors();
        let audit = telemetry::action_audit_tail(32);
        let path_access = dirotter_platform::assess_path_access(&self.root_input)
            .map(|a| {
                serde_json::json!({
                    "normalized_path": a.normalized_path,
                    "is_dir": a.is_dir,
                    "is_reparse_point": a.is_reparse_point,
                    "boundary": format!("{:?}", a.boundary),
                })
            })
            .unwrap_or_else(
                |e| serde_json::json!({"error": format!("{:?}: {}", e.kind, e.message)}),
            );

        let cache_json = serde_json::from_str::<serde_json::Value>(&cache_payload)
            .unwrap_or_else(|_| serde_json::json!({"raw": cache_payload}));

        self.diagnostics_json = serde_json::to_string_pretty(&serde_json::json!({
            "bundle_structure_version": 2,
            "cache": cache_json,
            "telemetry_snapshot": telemetry_snapshot,
            "system_snapshot": system_snapshot,
            "metrics": metrics,
            "action_audit_tail": audit,
            "path_access": path_access,
        }))
        .unwrap_or_else(|_| "{}".to_string());
    }

    fn reload_history(&mut self) -> rusqlite::Result<()> {
        self.history = self.cache.list_history(200)?;
        Ok(())
    }

    fn start_scan(&mut self) {
        self.page = Page::CurrentScan;
        self.status = AppStatus::Scanning;
        self.scan_current_path = None;
        self.scan_last_event_at = Some(Instant::now());
        self.scan_dropped_batches = 0;
        self.scan_dropped_snapshots = 0;
        self.scan_dropped_progress = 0;
        self.pending_batch_events.clear();
        self.pending_snapshots.clear();
        self.live_files.clear();
        self.live_top_files.clear();
        self.live_top_dirs.clear();
        self.completed_top_files.clear();
        self.completed_top_dirs.clear();
        self.store = None;
        self.delete_session = None;
        self.queued_delete = None;
        self.pending_delete_confirmation = None;
        self.execution_report = None;
        self.last_coalesce_commit = Instant::now();

        let handle = start_scan(
            PathBuf::from(self.root_input.clone()),
            ScanConfig {
                profile: self.scan_profile,
                batch_size: self.event_batch_size.max(1),
                snapshot_ms: self.snapshot_interval_ms.max(50),
                metadata_parallelism: 4,
                deep_tasks_throttle: 64,
            },
        );
        let (events, cancel) = handle.into_parts();
        let relay = Arc::new(Mutex::new(ScanRelayState::default()));
        let relay_thread_state = Arc::clone(&relay);
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(event) = events.recv() {
                let mut state = relay_thread_state.lock().expect("scan relay lock");
                state.last_event_at = Instant::now();
                match event {
                    ScanEvent::Progress(progress) => {
                        if state.latest_progress.is_some() {
                            state.dropped_progress = state.dropped_progress.saturating_add(1);
                        }
                        state.latest_progress = Some(progress);
                    }
                    ScanEvent::Batch(batch) => {
                        state.pending_batches.push_back(batch);
                        if state.pending_batches.len() > MAX_PENDING_BATCH_EVENTS {
                            let drop_n = state.pending_batches.len() - MAX_PENDING_BATCH_EVENTS;
                            state.pending_batches.drain(0..drop_n);
                            state.dropped_batches =
                                state.dropped_batches.saturating_add(drop_n as u64);
                        }
                    }
                    ScanEvent::Snapshot { delta, view } => {
                        if state.latest_snapshot.is_some() {
                            state.dropped_snapshots = state.dropped_snapshots.saturating_add(1);
                        }
                        state.latest_snapshot = Some((delta, view));
                    }
                    ScanEvent::Finished {
                        summary,
                        errors,
                        top_files,
                        top_dirs,
                    } => {
                        state.finished = Some(FinishedPayload {
                            summary,
                            errors,
                            top_files,
                            top_dirs,
                        });
                    }
                }
                drop(state);
                ctx.request_repaint();
            }
        });
        self.scan_session = Some(ScanSession { cancel, relay });
        self.page = Page::CurrentScan;
    }

    fn process_scan_events(&mut self) {
        let frame_start = Instant::now();
        let mut finished: Option<FinishedPayload> = None;

        if let Some(session) = &self.scan_session {
            let (
                progress,
                batches,
                snapshot,
                relay_finished,
                last_event_at,
                dropped_batches,
                dropped_snapshots,
                dropped_progress,
            ) = {
                let mut relay = session.relay.lock().expect("scan relay lock");
                (
                    relay.latest_progress.take(),
                    std::mem::take(&mut relay.pending_batches),
                    relay.latest_snapshot.take(),
                    relay.finished.take(),
                    relay.last_event_at,
                    relay.dropped_batches,
                    relay.dropped_snapshots,
                    relay.dropped_progress,
                )
            };

            self.scan_last_event_at = Some(last_event_at);
            self.scan_dropped_batches = dropped_batches;
            self.scan_dropped_snapshots = dropped_snapshots;
            self.scan_dropped_progress = dropped_progress;

            if let Some(progress) = progress {
                self.scan_current_path = progress.current_path.clone();
                self.summary = progress.summary;
                self.perf.snapshot_queue_depth = progress
                    .queue_depth
                    .max(progress.metadata_backlog)
                    .max(progress.publisher_lag);
            }

            for batch in batches {
                self.pending_batch_events.push_back(batch);
                if self.pending_batch_events.len() > MAX_PENDING_BATCH_EVENTS {
                    let drop_n = self.pending_batch_events.len() - MAX_PENDING_BATCH_EVENTS;
                    self.pending_batch_events.drain(0..drop_n);
                    telemetry::record_ui_backpressure(drop_n as u64, 0);
                }
            }

            if let Some((delta, view)) = snapshot {
                self.live_top_files = view.top_files;
                self.live_top_dirs = view.top_dirs;
                self.pending_snapshots.push_back(delta);
                let store = self.store.get_or_insert_with(NodeStore::default);
                for node in view.nodes {
                    if node.id.0 >= store.nodes.len() {
                        store.nodes.push(node.clone());
                    } else {
                        store.nodes[node.id.0] = node.clone();
                    }
                    store.path_index.insert(node.path.clone(), node.id);
                    if let Some(parent) = node.parent {
                        let children = store.children.entry(parent).or_default();
                        if !children.contains(&node.id) {
                            children.push(node.id);
                        }
                    }
                }
                if self.pending_snapshots.len() > MAX_PENDING_SNAPSHOTS {
                    let drop_n = self.pending_snapshots.len() - MAX_PENDING_SNAPSHOTS;
                    self.pending_snapshots.drain(0..drop_n);
                    telemetry::record_ui_backpressure(0, drop_n as u64);
                }
            }

            finished = relay_finished;
        }

        // Snapshot coalescing: commit once per 50~100ms
        if self.last_coalesce_commit.elapsed()
            >= Duration::from_millis(self.snapshot_interval_ms.max(50))
        {
            while let Some(batch) = self.pending_batch_events.pop_front() {
                for item in batch {
                    if !item.is_dir {
                        self.live_files.push((item.path, item.size));
                    }
                }
            }
            if self.live_files.len() > MAX_LIVE_FILES {
                let drop_n = self.live_files.len() - MAX_LIVE_FILES;
                self.live_files.drain(0..drop_n);
            }
            while let Some(snapshot) = self.pending_snapshots.pop_front() {
                self.summary = snapshot.summary;
            }
            self.last_coalesce_commit = Instant::now();
        }

        if let Some(finished) = finished {
            self.summary = finished.summary.clone();
            self.status = AppStatus::Completed;
            self.scan_current_path = None;
            self.scan_last_event_at = None;
            self.completed_top_files = finished.top_files;
            self.completed_top_dirs = finished.top_dirs;
            let _ = export_errors_csv(&finished.errors, "dirotter_errors.csv");
            let history_id = self
                .cache
                .record_scan_history(
                    &self.root_input,
                    finished.summary.scanned_files,
                    finished.summary.scanned_dirs,
                    finished.summary.bytes_observed,
                    finished.summary.error_count,
                    &finished.errors,
                )
                .ok();

            self.errors = finished.errors;
            self.execution_report = None;
            if let Some(id) = history_id {
                self.selected_history_id = Some(id);
            }
            let _ = self.reload_history();
            self.refresh_diagnostics();
            self.scan_session = None;
        }

        let t = telemetry::snapshot();
        self.perf.avg_snapshot_commit_ms = t.avg_snapshot_commit_ms;
        self.perf.avg_scan_batch_size = t.avg_scan_batch_size;
        self.perf.frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.perf.last_update = Some(Instant::now());
        telemetry::record_ui_frame();
    }

    fn ui_nav(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(self.t("空间分析工作台", "Storage Intelligence"))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.heading("DirOtter");
        ui.label(
            egui::RichText::new(self.t(
                "冷静地理解目录树，而不是急着清理一切。",
                "A calmer way to understand your file tree.",
            ))
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(12.0);

        ui.label(
            egui::RichText::new(self.t("导航", "Navigation"))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(6.0);

        for (p, label_zh, label_en) in [
            (Page::Dashboard, "概览", "Overview"),
            (Page::CurrentScan, "扫描进行中", "Live Scan"),
            (Page::Treemap, "矩形树图", "Treemap"),
            (Page::History, "历史记录", "History"),
            (Page::Errors, "错误中心", "Errors"),
            (Page::Diagnostics, "诊断导出", "Diagnostics"),
            (Page::Settings, "偏好设置", "Settings"),
        ] {
            let selected = self.page == p;
            let text = egui::RichText::new(self.t(label_zh, label_en))
                .size(14.0)
                .strong();
            if ui
                .add_sized(
                    [ui.available_width(), NAV_ITEM_HEIGHT],
                    egui::SelectableLabel::new(selected, text),
                )
                .clicked()
            {
                self.page = p;
            }
        }
    }

    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("磁盘概览", "Drive Overview"),
            self.t(
                "像主流磁盘分析器一样，先看卷空间，再看最大的文件夹和文件。",
                "Like mainstream disk analyzers: start with volume space, then inspect the largest folders and files.",
            ),
        );
        ui.add_space(10.0);
        if !self.scan_active() {
            let preferred_scope = if self.root_input.trim().is_empty() {
                self.t("先选一个盘符开始扫描。", "Pick a drive to begin scanning.")
            } else {
                self.t(
                    "用快速盘符立刻开始，或调整路径后再扫描。",
                    "Start from a quick-drive button, or refine the path before scanning.",
                )
            };
            tone_banner(
                ui,
                self.t("准备开始一次目录巡检", "Ready for a New Pass"),
                &format!(
                    "{} {}",
                    preferred_scope,
                    self.t(
                        "完成后，这个页面会优先给出卷空间、最大目录和最大文件。",
                        "When the scan completes, this page will surface volume usage, largest folders, and largest files first.",
                    )
                ),
            );
            ui.add_space(12.0);
        }
        if self.scan_active() {
            let current_path = self
                .scan_current_path
                .as_deref()
                .map(|path| truncate_middle(path, 72))
                .unwrap_or_else(|| {
                    self.t("正在准备扫描路径…", "Preparing scan path...")
                        .to_string()
                });
            tone_banner(
                ui,
                self.t("扫描仍在进行", "Scan Still Running"),
                &format!(
                    "{} {}\n{}",
                    self.t("当前正在处理：", "Currently working on:"),
                    current_path,
                    self.scan_health_summary()
                ),
            );
            ui.add_space(10.0);
        }

        let gap = 14.0;
        let usable_width = (ui.available_width() - gap).max(0.0);
        let left_width = (usable_width * 0.50).round();
        let right_width = (usable_width - left_width).max(0.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.render_scan_target_card(ui),
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.render_volume_summary_card(ui),
            );
        });

        ui.add_space(14.0);
        let ranked_dirs = self.current_ranked_dirs(10);
        let ranked_files = self.current_ranked_files(10);
        let folders_title = self.t("最大文件夹", "Largest Folders").to_string();
        let folders_subtitle = self
            .t(
                "优先看哪些目录占空间最多。",
                "Start with the folders consuming the most space.",
            )
            .to_string();
        let files_title = self.t("最大文件", "Largest Files").to_string();
        let files_subtitle = self
            .t(
                "这些通常是最直接可处理的空间占用点。",
                "These are usually the quickest wins for reclaiming space.",
            )
            .to_string();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_ranked_size_list(
                        ui,
                        &folders_title,
                        &folders_subtitle,
                        &ranked_dirs,
                        self.summary.bytes_observed,
                        &mut self.selection,
                        &mut self.execution_report,
                    )
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_ranked_size_list(
                        ui,
                        &files_title,
                        &files_subtitle,
                        &ranked_files,
                        self.summary.bytes_observed,
                        &mut self.selection,
                        &mut self.execution_report,
                    )
                },
            );
        });
    }

    fn render_scan_target_card(&mut self, ui: &mut egui::Ui) {
        surface_frame(ui).show(ui, |ui| {
            let root_hint = self
                .t("输入目录，例如 D:\\", "Enter a folder, e.g. D:\\")
                .to_string();
            ui.label(
                egui::RichText::new(self.t("扫描目标", "Scan Target"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.label(
                egui::RichText::new(self.t(
                    "先确定扫描范围，再决定性能策略。",
                    "Set the scan scope first, then tune the performance profile.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            ui.label(self.t("根目录", "Root path"));
            ui.add_sized(
                [ui.available_width(), CONTROL_HEIGHT],
                egui::TextEdit::singleline(&mut self.root_input)
                    .desired_width(f32::INFINITY)
                    .hint_text(root_hint),
            );
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(self.t("快速盘符", "Quick Drives"))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
            if self.available_volumes.is_empty() {
                ui.label(self.t(
                    "未检测到可用卷，仍可手动输入任意目录。",
                    "No mounted volumes were detected. You can still enter any path manually.",
                ));
            } else {
                let volumes = self.available_volumes.clone();
                ui.horizontal_wrapped(|ui| {
                    for volume in volumes {
                        let used = volume.total_bytes.saturating_sub(volume.available_bytes);
                        let selected = self.root_input == volume.mount_point;
                        let label = format!(
                            "{}  {} / {}",
                            short_volume_label(&volume),
                            format_bytes(used),
                            format_bytes(volume.total_bytes)
                        );
                        let response = ui
                            .add_enabled_ui(!self.scan_active(), |ui| {
                                sized_selectable(ui, 144.0, selected, label.clone())
                            })
                            .inner
                            .on_hover_text(format!(
                                "{}\n{} {}\n{} {}",
                                volume.name,
                                self.t("已用", "Used"),
                                format_bytes(used),
                                self.t("总量", "Total"),
                                format_bytes(volume.total_bytes)
                            ));
                        if response.clicked() {
                            self.start_scan_for_root(volume.mount_point.clone());
                        }
                    }
                });
                ui.label(
                    egui::RichText::new(self.t(
                        "点击盘符按钮可直接开始扫描；文本框仍可输入任意目录。",
                        "Click a drive button to scan it immediately, or type any custom path in the field above.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
            }
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.t("扫描策略", "Profile"));
                let ssd = ui
                    .add_sized(
                        [72.0, CONTROL_HEIGHT],
                        egui::SelectableLabel::new(self.scan_profile == ScanProfile::Ssd, "SSD"),
                    )
                    .clicked();
                let hdd = ui
                    .add_sized(
                        [72.0, CONTROL_HEIGHT],
                        egui::SelectableLabel::new(self.scan_profile == ScanProfile::Hdd, "HDD"),
                    )
                    .clicked();
                let network = ui
                    .add_sized(
                        [84.0, CONTROL_HEIGHT],
                        egui::SelectableLabel::new(self.scan_profile == ScanProfile::Network, "Network"),
                    )
                    .clicked();
                if ssd {
                    self.scan_profile = ScanProfile::Ssd;
                }
                if hdd {
                    self.scan_profile = ScanProfile::Hdd;
                }
                if network {
                    self.scan_profile = ScanProfile::Network;
                }
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(self.t("批大小", "Batch size"));
                    ui.add_sized(
                        [CONTROL_MIN_WIDTH, CONTROL_HEIGHT],
                        egui::DragValue::new(&mut self.event_batch_size)
                            .range(32..=4096)
                            .speed(8),
                    );
                });
                ui.vertical(|ui| {
                    ui.label(self.t("快照间隔", "Snapshot interval"));
                    ui.add_sized(
                        [CONTROL_MIN_WIDTH + 20.0, CONTROL_HEIGHT],
                        egui::DragValue::new(&mut self.snapshot_interval_ms)
                            .range(50..=1000)
                            .suffix(" ms")
                            .speed(5),
                    );
                });
            });
            ui.add_space(16.0);
            let start_label = if self.scan_active() {
                self.t("扫描进行中", "Scanning")
            } else {
                self.t("开始扫描", "Start Scan")
            };
            if ui
                .add_enabled_ui(!self.scan_active(), |ui| {
                    sized_primary_button(ui, ui.available_width(), start_label)
                })
                .inner
                .on_hover_text(self.t(
                    "扫描进行中时请使用右上角的停止按钮。",
                    "Use the top-right stop button while a scan is running.",
                ))
                .clicked()
            {
                self.start_scan();
            }
        });
    }

    fn render_volume_summary_card(&mut self, ui: &mut egui::Ui) {
        surface_frame(ui).show(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("卷空间摘要", "Volume Summary"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.label(
                egui::RichText::new(self.t(
                    "先用卷级别摘要确认空间方向，再决定接下来要展开哪些目录。",
                    "Use the volume-level summary to orient yourself before drilling into directories.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            status_badge(ui, self.status_text(), self.scan_active());
            ui.add_space(12.0);

            if let Some((used, free, total)) = self.volume_numbers() {
                ui.columns(2, |columns| {
                    compact_metric_block(
                        &mut columns[0],
                        self.t("磁盘已用", "Used"),
                        &format_bytes(used),
                        &format!("{} {}", format_bytes(total), self.t("总容量", "total")),
                    );
                    compact_metric_block(
                        &mut columns[1],
                        self.t("磁盘可用", "Free"),
                        &format_bytes(free),
                        self.t("系统卷信息", "System volume info"),
                    );
                });
                ui.add_space(8.0);
                ui.columns(2, |columns| {
                    compact_metric_block(
                        &mut columns[0],
                        self.t("已扫描", "Scanned"),
                        &format_bytes(self.summary.bytes_observed),
                        self.t(
                            "本次已遍历到的文件总大小",
                            "Total file bytes scanned so far",
                        ),
                    );
                    compact_metric_block(
                        &mut columns[1],
                        self.t("错误", "Errors"),
                        &format_count(self.summary.error_count),
                        self.t("无法读取或被跳过的路径", "Unreadable or skipped paths"),
                    );
                });

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(self.t("扫描覆盖率", "Scan Coverage"))
                        .text_style(egui::TextStyle::Small),
                );
                ui.add(
                    egui::ProgressBar::new(self.scanned_coverage_ratio().unwrap_or_default())
                        .text(format!(
                            "{} / {}",
                            format_bytes(self.summary.bytes_observed),
                            format_bytes(used)
                        ))
                        .desired_width(ui.available_width().max(120.0)),
                );
            }

            ui.add_space(12.0);
            ui.columns(2, |columns| {
                compact_metric_block(
                    &mut columns[0],
                    self.t("文件数", "Files"),
                    &format_count(self.summary.scanned_files),
                    self.t("当前已统计文件", "Files counted"),
                );
                compact_metric_block(
                    &mut columns[1],
                    self.t("目录数", "Folders"),
                    &format_count(self.summary.scanned_dirs),
                    self.t("当前已遍历目录", "Folders traversed"),
                );
            });
        });
    }

    fn ui_current_scan(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("实时扫描", "Live Scan"),
            self.t(
                "这里展示的是“扫描中已发现的最大项”，不是最终结果。内部性能指标已移到诊断页。",
                "This page shows the largest items discovered so far, not the final result. Internal performance counters have been moved to Diagnostics.",
            ),
        );
        ui.add_space(8.0);
        if self.scan_active() {
            let current_path = self
                .scan_current_path
                .as_deref()
                .map(|path| truncate_middle(path, 84))
                .unwrap_or_else(|| {
                    self.t("正在准备扫描路径…", "Preparing scan path...")
                        .to_string()
                });
            tone_banner(
                ui,
                self.t("这是实时增量视图", "This Is a Live Incremental View"),
                &format!(
                    "{} {}\n{}",
                    self.t(
                        "当前结果会持续更新，最终结论请以扫描完成后的概览页为准。正在处理：",
                        "Results keep updating while the scan runs. Use Overview after completion for the final summary. Working on:",
                    ),
                    current_path,
                    self.scan_health_summary()
                ),
            );
            ui.add_space(10.0);
        }

        ui.columns(5, |columns| {
            let cards = self.summary_cards();
            let accents = [
                river_teal(),
                info_blue(),
                success_green(),
                egui::Color32::from_rgb(0x5F, 0x8D, 0x96),
                danger_red(),
            ];
            for (idx, column) in columns.iter_mut().enumerate() {
                if let Some(card) = cards.get(idx) {
                    metric_card(column, &card.0, &card.1, &card.2, accents[idx]);
                }
            }
        });

        ui.add_space(12.0);
        let ranked_dirs = self.current_ranked_dirs(12);
        let (live_files_title, live_files_subtitle, ranked_files) =
            self.contextual_ranked_files_panel(12);
        let live_folders_title = self
            .t("当前最大的文件夹", "Largest Folders Found So Far")
            .to_string();
        let live_folders_subtitle = self
            .t(
                "扫描还未结束时，这里会持续更新。",
                "This keeps updating until the scan finishes.",
            )
            .to_string();
        ui.columns(2, |columns| {
            render_ranked_size_list(
                &mut columns[0],
                &live_folders_title,
                &live_folders_subtitle,
                &ranked_dirs,
                self.summary.bytes_observed,
                &mut self.selection,
                &mut self.execution_report,
            );
            render_ranked_size_list(
                &mut columns[1],
                &live_files_title,
                &live_files_subtitle,
                &ranked_files,
                self.summary.bytes_observed,
                &mut self.selection,
                &mut self.execution_report,
            );
        });

        ui.add_space(12.0);
        surface_frame(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(self.t("最近扫描到的文件", "Recently Scanned Files"))
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            format_count(self.live_files.len() as u64),
                            self.t("条", "rows")
                        ))
                        .color(ui.visuals().weak_text_color()),
                    );
                });
            });
            ui.add_space(6.0);
            let rows = self.live_files.len();
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show_rows(ui, 28.0, rows, |ui, row_range| {
                    for row in row_range {
                        if let Some((path, size)) = self.live_files.get(row).cloned() {
                            let row_width = (ui.available_width() - 120.0).max(120.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized(
                                        [row_width, 24.0],
                                        egui::SelectableLabel::new(
                                            self.selection.selected_path.as_deref()
                                                == Some(path.as_str()),
                                            truncate_middle(&path, 92),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.select_path(&path, SelectionSource::Table);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(format_bytes(size));
                                    },
                                );
                            });
                        }
                    }
                });
        });
    }

    fn treemap_tiles_for_viewport(
        &mut self,
        store: &NodeStore,
        viewport: egui::Rect,
    ) -> Vec<TreemapTile> {
        let key = (
            viewport.width() as u32,
            viewport.height() as u32,
            store.nodes.len(),
        );
        if self.treemap_cache.key == Some(key) {
            return self.treemap_cache.tiles.clone();
        }

        let dirs = store.largest_dirs(TREEMAP_TILE_LIMIT);
        let mut tiles = Vec::new();
        layout_treemap_recursive(viewport, &dirs, &mut tiles);
        self.treemap_cache = TreemapViewportCache {
            key: Some(key),
            tiles: tiles.clone(),
        };
        tiles
    }

    fn ui_treemap(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("矩形树图", "Treemap"),
            self.t(
                "只在有阅读价值的区域展示标签，悬浮可查看完整路径与体积。",
                "Only render labels where they remain legible; hover for full path and size.",
            ),
        );
        ui.add_space(8.0);
        let desired = egui::vec2(ui.available_width(), ui.available_height() - 12.0);
        let (rect, _response) = ui.allocate_exact_size(desired, egui::Sense::hover());
        if let Some(store) = self.store.clone() {
            let painter = ui.painter_at(rect);
            let tiles = self.treemap_tiles_for_viewport(&store, rect);

            for tile in &tiles {
                let mut resp = ui.interact(
                    tile.rect,
                    ui.make_persistent_id(("treemap", tile.node_id.0)),
                    egui::Sense::click(),
                );
                let mut color = palette_color(tile.node_id.0);
                if self.selection.selected_node == Some(tile.node_id) {
                    color = egui::Color32::from_rgb(0x4B, 0xA3, 0xAC);
                }
                if resp.clicked() {
                    self.selection.selected_node = Some(tile.node_id);
                    if let Some(node) = store.nodes.get(tile.node_id.0) {
                        self.selection.selected_path = Some(node.path.clone());
                    }
                    self.selection.source = Some(SelectionSource::Treemap);
                    self.execution_report = None;
                }
                if resp.hovered() {
                    resp = resp.on_hover_ui(|ui| {
                        ui.label(egui::RichText::new(&tile.path).strong());
                        ui.label(format!(
                            "{}: {}",
                            self.t("体积", "Size"),
                            format_bytes(tile.size_bytes)
                        ));
                    });
                }
                painter.rect_filled(tile.rect, 6.0, color);
                painter.rect_stroke(
                    tile.rect,
                    6.0,
                    egui::Stroke::new(1.0, egui::Color32::from_black_alpha(70)),
                );
                if !tile.label.is_empty() {
                    painter.text(
                        tile.rect.left_top() + egui::vec2(8.0, 8.0),
                        egui::Align2::LEFT_TOP,
                        &tile.label,
                        egui::FontId::new(13.0, egui::FontFamily::Proportional),
                        egui::Color32::WHITE,
                    );
                }
            }

            if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                if let Some(hit) = treemap_hit_test(&tiles, pointer_pos) {
                    ui.add_space(6.0);
                    ui.label(format!(
                        "{} {}  |  {} {}",
                        self.t("悬浮", "Hover"),
                        truncate_middle(&hit.path, 40),
                        self.t("体积", "Size"),
                        format_bytes(hit.size_bytes)
                    ));
                }
            }
        } else {
            surface_frame(ui).show(ui, |ui| {
                ui.label(self.t(
                    "暂无扫描结果，请先执行一次扫描。",
                    "No scan data yet. Start a scan first.",
                ));
            });
        }
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("历史快照", "History"),
            self.t(
                "按时间回看扫描快照，所有数字都改为适合人读的格式。",
                "Review previous scans with human-friendly formatting and clearer snapshot summaries.",
            ),
        );
        ui.add_space(8.0);
        if ui.button(self.t("刷新列表", "Refresh")).clicked() {
            let _ = self.reload_history();
        }

        let selected = self
            .selected_history_id
            .and_then(|id| self.history.iter().find(|h| h.id == id))
            .cloned();

        if let Some(h) = selected {
            surface_frame(ui).show(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.t("快照详情", "Snapshot Detail"))
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                ui.add_space(8.0);
                stat_row(ui, "ID", &h.id.to_string(), &truncate_middle(&h.root, 44));
                stat_row(
                    ui,
                    self.t("文件", "Files"),
                    &format_count(h.scanned_files),
                    self.t("扫描到的文件数", "File count"),
                );
                stat_row(
                    ui,
                    self.t("目录", "Dirs"),
                    &format_count(h.scanned_dirs),
                    self.t("扫描到的目录数", "Directory count"),
                );
                stat_row(
                    ui,
                    self.t("体积", "Bytes"),
                    &format_bytes(h.bytes_observed),
                    self.t("历史扫描到的文件体积", "Historical scanned file size"),
                );
                stat_row(
                    ui,
                    self.t("错误", "Errors"),
                    &format_count(h.error_count),
                    &h.created_at.to_string(),
                );
            });
            ui.separator();
        }

        egui::ScrollArea::vertical().show_rows(ui, 22.0, self.history.len(), |ui, range| {
            for i in range {
                if let Some(h) = self.history.get(i) {
                    let label = format!(
                        "#{} {}  |  {} {}  |  {} {}  |  {} {}",
                        h.id,
                        truncate_middle(&h.root, 34),
                        format_count(h.scanned_files),
                        self.t("文件", "files"),
                        format_count(h.scanned_dirs),
                        self.t("目录", "dirs"),
                        format_bytes(h.bytes_observed),
                        self.t("扫描体积", "scanned")
                    );
                    if ui
                        .selectable_label(self.selected_history_id == Some(h.id), label)
                        .clicked()
                    {
                        self.selected_history_id = Some(h.id);
                        if let Ok(e) = self.cache.list_errors_by_history(h.id) {
                            self.errors = e;
                        }
                        self.selection.source = Some(SelectionSource::History);
                        self.execution_report = None;
                    }
                }
            }
        });
    }

    fn ui_errors(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("错误中心", "Errors"),
            self.t(
                "保留错误分类与路径跳转，但避免把原始状态直接堆叠成噪声。",
                "Keep error categories and jump actions while reducing raw-text noise.",
            ),
        );
        ui.add_space(8.0);
        let mut user = 0usize;
        let mut transient = 0usize;
        let mut system = 0usize;
        for e in &self.errors {
            match e.kind {
                ErrorKind::User => user += 1,
                ErrorKind::Transient => transient += 1,
                ErrorKind::System => system += 1,
            }
        }
        ui.columns(3, |columns| {
            metric_card(
                &mut columns[0],
                "User",
                &format_count(user as u64),
                self.t("用户输入或权限问题", "Input or permission issues"),
                warning_amber(),
            );
            metric_card(
                &mut columns[1],
                "Transient",
                &format_count(transient as u64),
                self.t("可重试的瞬时失败", "Retryable transient failures"),
                info_blue(),
            );
            metric_card(
                &mut columns[2],
                "System",
                &format_count(system as u64),
                self.t("系统级故障", "System-level failures"),
                danger_red(),
            );
        });

        let filter_label = self.t("全部", "All").to_string();
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(self.t("过滤", "Filter"));
            ui.selectable_value(&mut self.error_filter, ErrorFilter::All, filter_label);
            ui.selectable_value(&mut self.error_filter, ErrorFilter::User, "User");
            ui.selectable_value(&mut self.error_filter, ErrorFilter::Transient, "Transient");
            ui.selectable_value(&mut self.error_filter, ErrorFilter::System, "System");
        });

        let filtered: Vec<_> = self
            .errors
            .iter()
            .filter(|e| match self.error_filter {
                ErrorFilter::All => true,
                ErrorFilter::User => matches!(e.kind, ErrorKind::User),
                ErrorFilter::Transient => matches!(e.kind, ErrorKind::Transient),
                ErrorFilter::System => matches!(e.kind, ErrorKind::System),
            })
            .cloned()
            .collect();

        ui.add_space(10.0);
        egui::ScrollArea::vertical().show_rows(ui, 72.0, filtered.len(), |ui, range| {
            for i in range {
                if let Some(e) = filtered.get(i) {
                    surface_frame(ui).show(ui, |ui| {
                        if ui
                            .selectable_label(
                                self.selection.selected_path.as_deref() == Some(&e.path),
                                format!("[{:?}] {}", e.kind, truncate_middle(&e.path, 68)),
                            )
                            .clicked()
                        {
                            self.select_path(&e.path, SelectionSource::Error);
                        }
                        ui.horizontal(|ui| {
                            if ui.button(self.t("选中查看", "Inspect")).clicked() {
                                self.select_path(&e.path, SelectionSource::Error);
                            }
                            ui.label(
                                egui::RichText::new(&e.reason)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    });
                }
            }
        });
    }

    fn ui_diagnostics(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("诊断导出", "Diagnostics"),
            self.t(
                "保留结构化 JSON，但给导出动作更明确的位置和说明。",
                "Keep the structured JSON, but surface export actions and explanation more clearly.",
            ),
        );
        ui.add_space(8.0);
        if ui
            .button(self.t("刷新诊断", "Refresh diagnostics"))
            .clicked()
        {
            self.refresh_diagnostics();
        }
        if ui
            .button(self.t("导出诊断包", "Export diagnostics bundle"))
            .clicked()
        {
            let mut manifest = default_manifest();
            manifest.diagnostics_payload_file = "dirotter_diagnostics.json".to_string();
            manifest.summary_report_file = "dirotter_summary.json".to_string();
            manifest.duplicate_report_file = "dirotter_duplicates.csv".to_string();
            manifest.error_report_file = "dirotter_errors.csv".to_string();
            let _ = export_diagnostics_bundle(
                &self.diagnostics_json,
                "dirotter_diagnostics.json",
                &manifest,
            );
            let _ = export_diagnostics_archive(
                &self.diagnostics_json,
                "diagnostics",
                "dirotter",
                &manifest,
            );
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.diagnostics_json)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(26)
                    .code_editor()
                    .interactive(false),
            );
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let content_width = ui.available_width().min(1180.0);
        ui.allocate_ui_with_layout(
            egui::vec2(content_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                page_header(
                    ui,
                    self.t("偏好设置", "Settings"),
                    self.t(
                        "让 DirOtter 保持冷静、低对比、长时间可用的工作状态。",
                        "Keep DirOtter calm, low-contrast, and comfortable for long sessions.",
                    ),
                );
                ui.add_space(10.0);
                tone_banner(
                    ui,
                    self.t("舒适优先的工作台", "A Comfort-First Workspace"),
                    self.t(
                        "语言、主题和字体回退都会立即生效。这里的目标不是“更花哨”，而是让长时间浏览目录树时更稳定、更耐看。",
                        "Language, theme, and font fallback all apply immediately. The goal here is not flashy UI, but a steadier workspace for long file-tree sessions.",
                    ),
                );
                ui.add_space(12.0);

                ui.columns(2, |columns| {
                    surface_frame(&columns[0]).show(&mut columns[0], |ui| {
                        ui.label(
                            egui::RichText::new(self.t("界面语言", "Interface Language"))
                                .text_style(egui::TextStyle::Name("title".into())),
                        );
                        ui.label(
                            egui::RichText::new(self.t(
                                "手动选择会覆盖系统语言检测。",
                                "Manual selection overrides automatic locale detection.",
                            ))
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [96.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(
                                        self.language == Lang::Zh,
                                        self.t("中文", "中文"),
                                    ),
                                )
                                .clicked()
                            {
                                self.language = Lang::Zh;
                                let _ = self.cache.set_setting("language", "zh");
                            }
                            if ui
                                .add_sized(
                                    [96.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(self.language == Lang::En, "English"),
                                )
                                .clicked()
                            {
                                self.language = Lang::En;
                                let _ = self.cache.set_setting("language", "en");
                            }
                        });
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new(self.t("界面主题", "Interface Theme"))
                                .text_style(egui::TextStyle::Name("title".into())),
                        );
                        ui.label(
                            egui::RichText::new(self.t(
                                "深色更适合长时间分析；浅色则保持低对比和柔和明度。",
                                "Dark is better for long analysis sessions; light stays restrained and low contrast.",
                            ))
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [120.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(!self.theme_dark, self.t("浅色", "Light")),
                                )
                                .clicked()
                            {
                                self.theme_dark = false;
                                self.apply_theme(ctx);
                                let _ = self.cache.set_setting("theme", "light");
                            }
                            if ui
                                .add_sized(
                                    [120.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(self.theme_dark, self.t("深色", "Dark")),
                                )
                                .clicked()
                            {
                                self.theme_dark = true;
                                self.apply_theme(ctx);
                                let _ = self.cache.set_setting("theme", "dark");
                            }
                        });
                    });

                    surface_frame(&columns[1]).show(&mut columns[1], |ui| {
                        ui.label(
                            egui::RichText::new(self.t("视觉方向", "Visual Direction"))
                                .text_style(egui::TextStyle::Name("title".into())),
                        );
                        ui.label(
                            egui::RichText::new(self.t(
                                "DirOtter 不是“垃圾清理器”的高噪音视觉，而是偏冷静、偏克制的分析界面。",
                                "DirOtter is not a loud cleaner UI. It is a quieter, analytical workspace with restrained emphasis.",
                            ))
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(10.0);
                        color_note_row(
                            ui,
                            river_teal(),
                            self.t("River Teal", "River Teal"),
                            self.t("主品牌色，用于主按钮、选中与重点数据。", "Primary brand accent for key actions, selection, and emphasis."),
                        );
                        ui.add_space(8.0);
                        color_note_row(
                            ui,
                            if self.theme_dark {
                                egui::Color32::from_rgb(0x18, 0x22, 0x27)
                            } else {
                                egui::Color32::from_rgb(0xEE, 0xF1, 0xF0)
                            },
                            self.t("基础面板", "Base Surfaces"),
                            self.t("保持低对比、长时间查看不刺眼。", "Kept low-contrast so long sessions stay easy on the eyes."),
                        );
                        ui.add_space(8.0);
                        color_note_row(
                            ui,
                            sand_accent(),
                            self.t("暖色辅助", "Warm Accent"),
                            self.t("只做轻微平衡，不大面积出现。", "Used sparingly to soften the palette, not dominate it."),
                        );
                        ui.add_space(12.0);
                        tone_banner(
                            ui,
                            self.t("当前模式", "Current Mode"),
                            if self.theme_dark {
                                self.t(
                                    "深色主题已启用：更适合长时间扫描和对比文件体积。",
                                    "Dark theme is enabled: better for extended scanning and file-size comparison.",
                                )
                            } else {
                                self.t(
                                    "浅色主题已启用：保持低对比和柔和明度，避免纯白带来的刺眼感。",
                                    "Light theme is enabled: low contrast and softer luminance to avoid harsh white surfaces.",
                                )
                            },
                        );
                    });
                });

                ui.add_space(12.0);
                ui.columns(2, |columns| {
                    surface_frame(&columns[0]).show(&mut columns[0], |ui| {
                        ui.label(
                            egui::RichText::new(self.t("本地化说明", "Localization Notes"))
                                .text_style(egui::TextStyle::Name("title".into())),
                        );
                        ui.label(self.t(
                            "应用会优先加载系统中的中文字体回退（Windows 优先 Microsoft YaHei / DengXian），避免中文标题和设置项显示为方框。",
                            "The app now prefers CJK-capable system fallback fonts (Windows prioritizes Microsoft YaHei / DengXian) so Chinese labels do not render as tofu boxes.",
                        ));
                        ui.add_space(6.0);
                        ui.label(self.t(
                            "首次启动仍可根据系统语言环境推断中英文，但这里的手动选择会覆盖自动检测结果。",
                            "The first launch can still infer language from the system locale, but the manual choice here overrides auto-detection.",
                        ));
                    });

                    surface_frame(&columns[1]).show(&mut columns[1], |ui| {
                        ui.label(
                            egui::RichText::new(self.t("品牌含义", "Why DirOtter"))
                                .text_style(egui::TextStyle::Name("title".into())),
                        );
                        ui.label(self.t(
                            "Dir 指 directory，Otter 借用水獭聪明、灵活、善于整理的联想。它更像一个冷静探索存储结构的分析工具，而不是只会“清理垃圾”的工具。",
                            "Dir points to directories, while Otter brings a clever, tidy, exploratory character. The product should feel like a calm storage analyzer, not a noisy junk cleaner.",
                        ));
                    });
                });
            },
        );
    }

    fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("DirOtter")
                    .size(22.0)
                    .strong()
                    .color(ui.visuals().text_color()),
            );
            ui.add_space(10.0);
            status_badge(ui, self.status_text(), self.scan_active() || self.delete_active());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let active = self.scan_active();
                let deleting = self.delete_active();
                let stop_label = if active {
                    self.t("停止扫描", "Stop Scan")
                } else {
                    self.t("取消", "Cancel")
                };
                if ui
                    .add_enabled_ui(active, |ui| sized_button(ui, 108.0, stop_label))
                    .inner
                    .clicked()
                {
                    if let Some(session) = &self.scan_session {
                        session.cancel.store(true, Ordering::SeqCst);
                        self.status = AppStatus::Cancelled;
                        self.scan_current_path = None;
                    }
                }
                let start_label = if active {
                    self.t("扫描中", "Scanning")
                } else if deleting {
                    self.t("删除中", "Deleting")
                } else {
                    self.t("开始扫描", "Start Scan")
                };
                if ui
                    .add_enabled_ui(!active && !deleting, |ui| {
                        sized_button(ui, 108.0, start_label)
                    })
                    .inner
                    .clicked()
                {
                    self.start_scan();
                }
            });
        });
    }

    fn ui_inspector(&mut self, ui: &mut egui::Ui) {
        let selected_target = self.selected_target();
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(self.t("检查器", "Inspector"))
                .text_style(egui::TextStyle::Name("title".into())),
        );
        ui.label(
            egui::RichText::new(self.t("当前聚焦对象详情", "Details for the current selection"))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(10.0);

        surface_frame(ui).show(ui, |ui| {
            if let Some(target) = selected_target.as_ref() {
                stat_row(
                    ui,
                    self.t("名称", "Name"),
                    &target.name,
                    match target.kind {
                        NodeKind::Dir => self.t("目录", "Directory"),
                        NodeKind::File => self.t("文件", "File"),
                    },
                );
                stat_row(
                    ui,
                    self.t("路径", "Path"),
                    &truncate_middle(&target.path, 34),
                    self.t("完整路径可在悬浮提示中查看", "Full path available on hover"),
                );
                stat_row(
                    ui,
                    self.t("大小", "Size"),
                    &format_bytes(target.size_bytes),
                    &format!(
                        "{} {} / {} {}",
                        format_count(target.file_count),
                        self.t("文件", "files"),
                        format_count(target.dir_count),
                        self.t("目录", "dirs")
                    ),
                );
            } else {
                ui.label(self.t(
                    "尚未选择任何文件或目录。可以从实时列表、历史、错误页或 treemap 中点选对象。",
                    "No file or folder selected yet. Pick one from the live list, history, errors, or treemap.",
                ));
            }
        });

        if let Some(session) = self.delete_session.as_ref() {
            let snapshot = session.snapshot();
            ui.add_space(10.0);
            surface_frame(ui).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(match snapshot.mode {
                            ExecutionMode::RecycleBin => {
                                self.t("后台任务：移到回收站", "Background Task: Recycle Bin")
                            }
                            ExecutionMode::Permanent => {
                                self.t("后台任务：永久删除", "Background Task: Permanent Delete")
                            }
                        })
                        .text_style(egui::TextStyle::Name("title".into())),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::Spinner::new().size(18.0));
                    });
                });
                ui.label(
                    egui::RichText::new(self.t(
                        "删除正在后台执行。你可以继续浏览结果，但新的删除操作会暂时锁定。",
                        "Deletion is running in the background. You can keep browsing results, but new delete actions stay locked for now.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(8.0);
                stat_row(
                    ui,
                    self.t("目标", "Target"),
                    &truncate_middle(&snapshot.target_path, 34),
                    self.t("当前正在处理的路径", "Path currently being processed"),
                );
                stat_row(
                    ui,
                    self.t("已耗时", "Elapsed"),
                    &format!("{:.1}s", snapshot.started_at.elapsed().as_secs_f32()),
                    match snapshot.mode {
                        ExecutionMode::RecycleBin => {
                            self.t("回收站删除", "Recycle-bin delete")
                        }
                        ExecutionMode::Permanent => self.t("永久删除", "Permanent delete"),
                    },
                );
            });
        }

        ui.add_space(10.0);
        surface_frame(ui).show(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("快速操作", "Quick Actions"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.label(
                egui::RichText::new(self.t(
                    "直接在右侧完成清理，不再跳到单独的操作页。",
                    "Delete directly from the inspector instead of jumping to a separate page.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            let has_selection = selected_target.is_some();
            let delete_active = self.delete_active();
            ui.vertical(|ui| {
                if ui
                    .add_enabled_ui(has_selection, |ui| {
                        sized_button(ui, ui.available_width(), self.t("打开所在位置", "Open File Location"))
                    })
                    .inner
                    .clicked()
                {
                    if let Some(target) = selected_target.as_ref() {
                        match dirotter_platform::select_in_explorer(&target.path) {
                            Ok(_) => {
                                self.explorer_feedback = Some((
                                    self.t(
                                        "已在系统文件管理器中打开目标位置。",
                                        "Opened the target location in the system file manager.",
                                    )
                                    .to_string(),
                                    true,
                                ));
                            }
                            Err(err) => {
                                self.explorer_feedback = Some((
                                    format!(
                                        "{}: {}",
                                        self.t("打开位置失败", "Failed to open location"),
                                        err.message
                                    ),
                                    false,
                                ));
                            }
                        }
                    }
                }
                if ui
                    .add_enabled_ui(has_selection && !delete_active, |ui| {
                        sized_button(ui, ui.available_width(), self.t("移到回收站", "Move to Recycle Bin"))
                    })
                    .inner
                    .clicked()
                {
                    self.execute_selected_delete(ExecutionMode::RecycleBin);
                }
                let permanent = egui::Button::new(self.t("永久删除", "Delete Permanently"))
                    .fill(danger_red());
                if ui
                    .add_enabled_ui(has_selection && !delete_active, |ui| {
                        ui.add_sized([ui.available_width(), CONTROL_HEIGHT], permanent)
                    })
                    .inner
                    .clicked()
                {
                    if let Some(target) = selected_target.clone() {
                        self.pending_delete_confirmation = Some(PendingDeleteConfirmation {
                            risk: self.risk_for_path(&target.path),
                            target,
                        });
                    }
                }
            });
            if delete_active {
                ui.label(
                    egui::RichText::new(self.t(
                        "后台删除任务正在执行。你可以继续浏览列表，但新的删除动作会在完成前保持禁用。",
                        "A background delete task is running. You can keep browsing, but new delete actions stay disabled until it finishes.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
            } else if !has_selection {
                ui.label(
                    egui::RichText::new(self.t(
                        "先从列表、树图、历史或错误列表里选中一个文件或文件夹。",
                        "Select a file or folder from a list, treemap, history, or errors first.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
            }
            if let Some((message, success)) = self.explorer_feedback.as_ref() {
                ui.add_space(8.0);
                if *success {
                    tone_banner(
                        ui,
                        self.t("已打开所在位置", "Opened Location"),
                        message,
                    );
                } else {
                    tone_banner(
                        ui,
                        self.t("打开位置失败", "Open Location Failed"),
                        message,
                    );
                }
            }
            if let Some((title, hint, success)) = self.delete_feedback_message() {
                ui.add_space(10.0);
                tone_banner(ui, &title, &hint);
                if !success {
                    ui.add_space(6.0);
                }
            }
            if let Some(report) = self.execution_report.as_ref() {
                ui.add_space(10.0);
                stat_row(
                    ui,
                    self.t("最近执行", "Last Action"),
                    match report.mode {
                        ExecutionMode::RecycleBin => self.t("移到回收站", "Moved to recycle bin"),
                        ExecutionMode::Permanent => self.t("永久删除", "Permanent delete"),
                    },
                    &format!(
                        "{} {} / {} {}",
                        format_count(report.succeeded as u64),
                        self.t("成功", "succeeded"),
                        format_count(report.failed as u64),
                        self.t("失败", "failed")
                    ),
                );
                if let Some(item) = report.items.first() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            if item.success {
                                self.t("结果", "Result")
                            } else {
                                self.t("失败原因", "Failure")
                            },
                            item.message
                        ))
                        .text_style(egui::TextStyle::Small)
                        .color(if item.success {
                            ui.visuals().text_color()
                        } else {
                            danger_red()
                        }),
                    );
                }
            }
        });

        ui.add_space(10.0);
        surface_frame(ui).show(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("工作上下文", "Workspace Context"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            stat_row(
                ui,
                self.t("根目录", "Root"),
                &truncate_middle(&self.root_input, 32),
                self.t("当前扫描目标", "Current scan target"),
            );
            stat_row(
                ui,
                self.t("来源", "Source"),
                self.selection
                    .source
                    .map(|s| self.source_label(s))
                    .unwrap_or_else(|| self.t("无", "None")),
                self.t("当前聚焦来源", "Selection source"),
            );
        });
    }

    fn ui_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_delete_confirmation.clone() else {
            return;
        };

        let mut keep_open = true;
        let mut confirmed_delete: Option<SelectedTarget> = None;
        egui::Window::new(self.t("确认永久删除", "Confirm Permanent Delete"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(
                    egui::RichText::new(self.t(
                        "该操作会直接删除文件或目录，不进入回收站。",
                        "This action deletes the file or folder directly without using the recycle bin.",
                    ))
                    .strong(),
                );
                ui.add_space(8.0);
                stat_row(
                    ui,
                    self.t("目标", "Target"),
                    &truncate_middle(&pending.target.path, 42),
                    match pending.target.kind {
                        NodeKind::Dir => self.t("目录", "Directory"),
                        NodeKind::File => self.t("文件", "File"),
                    },
                );
                stat_row(
                    ui,
                    self.t("大小", "Size"),
                    &format_bytes(pending.target.size_bytes),
                    &format!("{:?}", pending.risk),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(self.t(
                        "建议：如果只是普通清理，优先使用“移到回收站”。永久删除适合明确确认后再执行。",
                        "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let confirm = egui::Button::new(self.t("确认永久删除", "Delete Permanently"))
                        .fill(danger_red());
                    if ui.add(confirm).clicked() {
                        confirmed_delete = Some(pending.target.clone());
                        keep_open = false;
                    }
                    if ui.button(self.t("取消", "Cancel")).clicked() {
                        keep_open = false;
                    }
                });
            });

        if !keep_open {
            self.pending_delete_confirmation = None;
        }
        if let Some(target) = confirmed_delete {
            self.queue_delete_for_target(target, ExecutionMode::Permanent);
        }
    }

    fn ui_statusbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} {}  |  {} {}  |  {} {}  |  {} {}",
                    format_count(self.summary.scanned_files),
                    self.t("文件", "files"),
                    format_count(self.summary.scanned_dirs),
                    self.t("目录", "dirs"),
                    format_bytes(self.summary.bytes_observed),
                    self.t("扫描体积", "scanned"),
                    format_count(self.summary.error_count),
                    self.t("错误", "errors")
                ))
                .text_style(egui::TextStyle::Small),
            );
            if let Some(volume) = self.current_volume_info() {
                let used = volume.total_bytes.saturating_sub(volume.available_bytes);
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} / {} {}",
                        format_bytes(used),
                        self.t("已用", "used"),
                        format_bytes(volume.total_bytes),
                        self.t("总量", "total")
                    ))
                    .text_style(egui::TextStyle::Small),
                );
            }
            if self.scan_active() {
                ui.separator();
                ui.label(
                    egui::RichText::new(self.scan_health_short())
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
            }
            if let Some(session) = self.delete_session.as_ref() {
                let snapshot = session.snapshot();
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "{} {:.1}s  |  {}",
                        self.t("删除中", "Deleting"),
                        snapshot.started_at.elapsed().as_secs_f32(),
                        truncate_middle(&snapshot.target_path, 42)
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
            }
        });
    }

    fn ui_delete_activity_banner(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.delete_session.as_ref() else {
            return;
        };
        let snapshot = session.snapshot();
        let phase = snapshot.started_at.elapsed().as_secs_f32();
        let pulse = ((phase.sin() + 1.0) * 0.5 * 0.7 + 0.15).clamp(0.08, 0.92);

        tone_banner(
            ui,
            match snapshot.mode {
                ExecutionMode::RecycleBin => {
                    self.t("正在后台移到回收站", "Moving to Recycle Bin in Background")
                }
                ExecutionMode::Permanent => {
                    self.t("正在后台永久删除", "Deleting Permanently in Background")
                }
            },
            &format!(
                "{}  |  {} {:.1}s  |  {}",
                truncate_middle(&snapshot.target_path, 72),
                self.t("已耗时", "Elapsed"),
                phase,
                self.t(
                    "你可以继续浏览扫描结果，删除完成后界面会自动同步。",
                    "You can keep browsing scan results. The UI will synchronize automatically when deletion finishes.",
                )
            ),
        );
        ui.add_space(6.0);
        ui.add(
            egui::ProgressBar::new(pulse)
                .desired_width(ui.available_width().max(220.0))
                .text(self.t(
                    "系统正在处理删除请求",
                    "System is processing the delete request",
                )),
        );
    }
}

impl eframe::App for DirOtterNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_scan_events();
        self.process_delete_events();
        self.process_queued_delete();
        self.apply_theme(ctx);
        let delete_active = self.delete_active();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "DirOtter {}",
            self.status_text()
        )));
        if self.scan_active() || delete_active {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        egui::TopBottomPanel::top("top_bar")
            .exact_height(TOOLBAR_HEIGHT)
            .show_separator_line(false)
            .frame(toolbar_frame(ctx))
            .show(ctx, |ui| self.ui_toolbar(ui));

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(STATUSBAR_HEIGHT)
            .show_separator_line(false)
            .frame(statusbar_frame(ctx))
            .show(ctx, |ui| self.ui_statusbar(ui));

        egui::SidePanel::left("nav")
            .exact_width(NAV_WIDTH)
            .resizable(false)
            .show_separator_line(false)
            .frame(panel_frame(ctx))
            .show(ctx, |ui| self.ui_nav(ui));

        egui::SidePanel::right("inspector")
            .exact_width(INSPECTOR_WIDTH)
            .resizable(true)
            .show_separator_line(false)
            .frame(panel_frame(ctx))
            .show(ctx, |ui| self.ui_inspector(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(ctx.style().visuals.window_fill)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                if delete_active {
                    self.ui_delete_activity_banner(ui);
                    ui.add_space(12.0);
                }
                match self.page {
                    Page::Dashboard => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH, |ui| self.ui_dashboard(ui))
                    }
                    Page::CurrentScan => with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 40.0, |ui| {
                        self.ui_current_scan(ui)
                    }),
                    Page::Treemap => self.ui_treemap(ui),
                    Page::History => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 20.0, |ui| self.ui_history(ui))
                    }
                    Page::Errors => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 20.0, |ui| self.ui_errors(ui))
                    }
                    Page::Diagnostics => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 20.0, |ui| self.ui_diagnostics(ui))
                    }
                    Page::Settings => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH, |ui| self.ui_settings(ui, ctx))
                    }
                }
            });

        self.ui_delete_confirm_dialog(ctx);
    }
}

fn layout_treemap_recursive(
    rect: egui::Rect,
    dirs: &[&dirotter_core::Node],
    out: &mut Vec<TreemapTile>,
) {
    if dirs.is_empty()
        || rect.width() < MIN_TREEMAP_TILE_EDGE
        || rect.height() < MIN_TREEMAP_TILE_EDGE
    {
        return;
    }
    if dirs.len() == 1 {
        let node = dirs[0];
        out.push(TreemapTile {
            node_id: node.id,
            rect,
            label: treemap_label_for_rect(&node.name, node.size_subtree.max(node.size_self), rect)
                .unwrap_or_default(),
            size_bytes: node.size_subtree.max(node.size_self),
            path: node.path.clone(),
        });
        return;
    }

    let total: u64 = dirs.iter().map(|d| d.size_subtree.max(1)).sum();
    let mut acc = 0u64;
    let mut split_idx = 0usize;
    for (i, d) in dirs.iter().enumerate() {
        acc += d.size_subtree.max(1);
        if acc * 2 >= total {
            split_idx = i + 1;
            break;
        }
    }
    let split_idx = split_idx.clamp(1, dirs.len() - 1);
    let left = &dirs[..split_idx];
    let right = &dirs[split_idx..];
    let left_sum: u64 = left.iter().map(|d| d.size_subtree.max(1)).sum();

    if rect.width() >= rect.height() {
        let w = rect.width() * (left_sum as f32 / total as f32);
        let a = egui::Rect::from_min_size(rect.min, egui::vec2(w.max(1.0), rect.height()));
        let b = egui::Rect::from_min_max(egui::pos2(a.right(), rect.top()), rect.max);
        layout_treemap_recursive(a, left, out);
        layout_treemap_recursive(b, right, out);
    } else {
        let h = rect.height() * (left_sum as f32 / total as f32);
        let a = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), h.max(1.0)));
        let b = egui::Rect::from_min_max(egui::pos2(rect.left(), a.bottom()), rect.max);
        layout_treemap_recursive(a, left, out);
        layout_treemap_recursive(b, right, out);
    }
}

fn treemap_hit_test(tiles: &[TreemapTile], pos: egui::Pos2) -> Option<&TreemapTile> {
    tiles.iter().find(|t| t.rect.contains(pos))
}

fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(data) = load_primary_ui_font_bytes() {
        fonts
            .font_data
            .insert("brand-ui".to_string(), egui::FontData::from_owned(data));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "brand-ui".to_string());
    }
    if let Some(data) = load_system_font_bytes() {
        fonts
            .font_data
            .insert("cjk-fallback".to_string(), egui::FontData::from_owned(data));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("cjk-fallback".to_string());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("cjk-fallback".to_string());
    }
    ctx.set_fonts(fonts);
}

fn load_primary_ui_font_bytes() -> Option<Vec<u8>> {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\seguisb.ttf",
            "C:\\Windows\\Fonts\\segoeuib.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
            "/usr/share/fonts/truetype/inter-vf/Inter.var.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
    };

    candidates.iter().find_map(|path| fs::read(path).ok())
}

fn load_system_font_bytes() -> Option<Vec<u8>> {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\msyh.ttf",
            "C:\\Windows\\Fonts\\Deng.ttf",
            "C:\\Windows\\Fonts\\simhei.ttf",
            "C:\\Windows\\Fonts\\simsun.ttc",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
        ]
    } else {
        &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.otf",
        ]
    };

    candidates.iter().find_map(|path| fs::read(path).ok())
}

fn river_teal() -> egui::Color32 {
    egui::Color32::from_rgb(0x2F, 0x7F, 0x86)
}

fn river_teal_hover() -> egui::Color32 {
    egui::Color32::from_rgb(0x27, 0x6D, 0x73)
}

fn river_teal_active() -> egui::Color32 {
    egui::Color32::from_rgb(0x1F, 0x5C, 0x61)
}

fn sand_accent() -> egui::Color32 {
    egui::Color32::from_rgb(0xD8, 0xC6, 0xA5)
}

fn success_green() -> egui::Color32 {
    egui::Color32::from_rgb(0x2E, 0x8B, 0x57)
}

fn warning_amber() -> egui::Color32 {
    egui::Color32::from_rgb(0xC9, 0x8B, 0x2E)
}

fn danger_red() -> egui::Color32 {
    egui::Color32::from_rgb(0xC9, 0x4F, 0x4F)
}

fn info_blue() -> egui::Color32 {
    egui::Color32::from_rgb(0x4B, 0x7B, 0xEC)
}

fn build_dark_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = egui::Color32::from_rgb(0x11, 0x18, 0x1C);
    visuals.panel_fill = egui::Color32::from_rgb(0x18, 0x22, 0x27);
    visuals.extreme_bg_color = egui::Color32::from_rgb(0x0F, 0x15, 0x19);
    visuals.faint_bg_color = egui::Color32::from_rgb(0x1F, 0x2C, 0x32);
    visuals.code_bg_color = egui::Color32::from_rgb(0x14, 0x1D, 0x21);
    visuals.override_text_color = Some(egui::Color32::from_rgb(0xEA, 0xF2, 0xF4));
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(0x18, 0x22, 0x27);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x29, 0x37, 0x3E));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(0x1C, 0x27, 0x2D);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x2E, 0x3D, 0x44));
    visuals.widgets.hovered.bg_fill = river_teal_hover();
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, river_teal());
    visuals.widgets.active.bg_fill = river_teal_active();
    visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x4B, 0xA3, 0xAC));
    visuals.selection.bg_fill = egui::Color32::from_rgb(0x4B, 0xA3, 0xAC);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.open.bg_fill = egui::Color32::from_rgb(0x1F, 0x2C, 0x32);
    visuals
}

fn build_light_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::light();
    visuals.window_fill = egui::Color32::from_rgb(0xE7, 0xEC, 0xEA);
    visuals.panel_fill = egui::Color32::from_rgb(0xEE, 0xF1, 0xF0);
    visuals.extreme_bg_color = egui::Color32::from_rgb(0xDD, 0xE4, 0xE2);
    visuals.faint_bg_color = egui::Color32::from_rgb(0xE7, 0xEC, 0xEA);
    visuals.code_bg_color = egui::Color32::from_rgb(0xE3, 0xE8, 0xE6);
    visuals.override_text_color = Some(egui::Color32::from_rgb(0x26, 0x32, 0x38));
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(0xEE, 0xF1, 0xF0);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0xCB, 0xD4, 0xD1));
    visuals.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x4E, 0x5D, 0x63));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(0xE5, 0xEA, 0xE8);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0xC8, 0xD1, 0xCE));
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(0xDC, 0xE5, 0xE3);
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x88, 0xA2, 0xA5));
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0xD2, 0xDD, 0xDA);
    visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x6E, 0x8E, 0x92));
    visuals.selection.bg_fill = egui::Color32::from_rgb(0x7A, 0x99, 0x9D);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.open.bg_fill = egui::Color32::from_rgb(0xE1, 0xE7, 0xE5);
    visuals
}

fn panel_frame(ctx: &egui::Context) -> egui::Frame {
    let visuals = &ctx.style().visuals;
    egui::Frame::default()
        .fill(visuals.panel_fill)
        .inner_margin(egui::Margin::same(CARD_PADDING))
        .rounding(egui::Rounding::same(SHELL_RADIUS as f32))
        .stroke(egui::Stroke::NONE)
}

fn toolbar_frame(ctx: &egui::Context) -> egui::Frame {
    let visuals = &ctx.style().visuals;
    egui::Frame::default()
        .fill(visuals.panel_fill)
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .rounding(egui::Rounding::same(SHELL_RADIUS as f32))
        .stroke(egui::Stroke::NONE)
}

fn statusbar_frame(ctx: &egui::Context) -> egui::Frame {
    let visuals = &ctx.style().visuals;
    egui::Frame::default()
        .fill(visuals.panel_fill)
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .rounding(egui::Rounding::same(SHELL_RADIUS as f32))
        .stroke(egui::Stroke::NONE)
}

fn surface_frame(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::default()
        .fill(visuals.faint_bg_color)
        .inner_margin(egui::Margin::same(CARD_PADDING))
        .rounding(egui::Rounding::same(CARD_RADIUS as f32))
        .stroke(egui::Stroke::new(CARD_STROKE_WIDTH, border_color(visuals)))
}

fn border_color(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.dark_mode {
        egui::Color32::from_rgb(0x2B, 0x38, 0x3E)
    } else {
        egui::Color32::from_rgb(0xC8, 0xD0, 0xCE)
    }
}

fn with_page_width<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available = ui.available_width();
    let width = (available - 24.0).max(320.0).min(max_width);
    let left_space = ((available - width) / 2.0).floor().max(0.0);
    let right_space = (available - width - left_space).max(0.0);
    ui.horizontal(|ui| {
        if left_space > 0.0 {
            ui.add_space(left_space);
        }
        let inner = ui
            .allocate_ui_with_layout(
            egui::vec2(width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            add_contents,
        )
            .inner;
        if right_space > 0.0 {
            ui.add_space(right_space);
        }
        inner
    })
    .inner
}

fn with_scrollable_page_width<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| with_page_width(ui, max_width, add_contents))
        .inner
}

fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(
        egui::RichText::new("DirOtter Workspace")
            .text_style(egui::TextStyle::Small)
            .color(river_teal()),
    );
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(title)
            .text_style(egui::TextStyle::Heading)
            .strong(),
    );
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(subtitle)
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
    );
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str, accent: egui::Color32) {
    surface_frame(ui).show(ui, |ui| {
        ui.colored_label(accent, egui::RichText::new(title).strong());
        ui.add_space(4.0);
        ui.label(egui::RichText::new(value).size(22.0).strong());
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
    });
}

fn sized_selectable(
    ui: &mut egui::Ui,
    width: f32,
    selected: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add_sized(
        [width.max(CONTROL_MIN_WIDTH), CONTROL_HEIGHT],
        egui::SelectableLabel::new(selected, text),
    )
}

fn sized_button(
    ui: &mut egui::Ui,
    width: f32,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add_sized(
        [width.max(CONTROL_MIN_WIDTH), CONTROL_HEIGHT],
        egui::Button::new(text),
    )
}

fn sized_primary_button(
    ui: &mut egui::Ui,
    width: f32,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add_sized(
        [width.max(CONTROL_MIN_WIDTH), PRIMARY_BUTTON_HEIGHT],
        egui::Button::new(text),
    )
}

fn render_ranked_size_list(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    items: &[(String, u64)],
    total: u64,
    selection: &mut SelectionState,
    execution_report: &mut Option<ExecutionReport>,
) {
    surface_frame(ui).show(ui, |ui| {
        ui.push_id(("ranked-panel", title), |ui| {
            ui.label(egui::RichText::new(title).text_style(egui::TextStyle::Name("title".into())));
            ui.label(
                egui::RichText::new(subtitle)
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);

            if items.is_empty() {
                empty_state_panel(
                    ui,
                    title,
                    if title.contains("Folder") || title.contains("文件夹") {
                        "Start a scan to see which directories consume the most space."
                    } else {
                        "Start a scan to surface the largest files worth reviewing first."
                    },
                );
                return;
            }

            let denom = total.max(items.iter().map(|(_, size)| *size).max().unwrap_or(1));
            for (idx, (path, size)) in items.iter().enumerate() {
                let ratio = (*size as f32 / denom as f32).clamp(0.0, 1.0);
                let label = format!("{}. {}", idx + 1, truncate_middle(path, 52));
                let row_width = (ui.available_width() - 150.0).max(120.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [row_width, 22.0],
                            egui::SelectableLabel::new(
                                selection.selected_path.as_deref() == Some(path.as_str()),
                                label,
                            ),
                        )
                        .clicked()
                    {
                        selection.selected_path = Some(path.clone());
                        selection.source = Some(SelectionSource::Table);
                        selection.selected_node = None;
                        *execution_report = None;
                    }
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(format_bytes(*size));
                        },
                    );
                });
                ui.add(
                    egui::ProgressBar::new(ratio)
                        .desired_width(ui.available_width().max(120.0))
                        .text(format!("{:.1}%", ratio * 100.0)),
                );
                if idx + 1 < items.len() {
                    ui.add_space(6.0);
                }
            }
        });
    });
}

fn empty_state_panel(ui: &mut egui::Ui, title: &str, body: &str) {
    let visuals = ui.visuals();
    egui::Frame::default()
        .fill(if visuals.dark_mode {
            egui::Color32::from_rgb(0x1A, 0x24, 0x29)
        } else {
            egui::Color32::from_rgb(0xEC, 0xF1, 0xEF)
        })
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::same(14.0))
        .stroke(egui::Stroke::new(1.0, border_color(visuals)))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .text_style(egui::TextStyle::Small)
                    .color(river_teal()),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(body)
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
        });
}

fn tone_banner(ui: &mut egui::Ui, title: &str, body: &str) {
    let visuals = ui.visuals();
    let width = ui.available_width();
    egui::Frame::default()
        .fill(if visuals.dark_mode {
            egui::Color32::from_rgb(0x1D, 0x2A, 0x30)
        } else {
            egui::Color32::from_rgb(0xEE, 0xF4, 0xF5)
        })
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::same(10.0))
        .stroke(egui::Stroke::new(
            1.0,
            if visuals.dark_mode {
                river_teal_hover()
            } else {
                sand_accent()
            },
        ))
        .show(ui, |ui| {
            ui.set_min_width(width);
            ui.label(egui::RichText::new(title).strong().color(river_teal()));
            ui.label(body);
        });
}

fn color_note_row(ui: &mut egui::Ui, swatch: egui::Color32, title: &str, body: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), swatch);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(title).strong());
            ui.label(
                egui::RichText::new(body)
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
        });
    });
}

fn status_badge(ui: &mut egui::Ui, status: &str, active: bool) {
    let bg = if active {
        if ui.visuals().dark_mode {
            egui::Color32::from_rgb(0x4B, 0xA3, 0xAC)
        } else {
            egui::Color32::from_rgb(0x7A, 0x99, 0x9D)
        }
    } else {
        egui::Color32::from_rgb(0x8B, 0x93, 0x97)
    };
    let text = egui::RichText::new(status)
        .color(egui::Color32::WHITE)
        .strong();
    egui::Frame::default()
        .fill(bg)
        .rounding(egui::Rounding::same(999.0))
        .inner_margin(egui::Margin::symmetric(
            12.0,
            ((STATUS_BADGE_HEIGHT - 20.0) / 2.0).max(4.0),
        ))
        .show(ui, |ui: &mut egui::Ui| {
            ui.label(text);
        });
}

fn compact_metric_block(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).strong());
        ui.label(egui::RichText::new(value).size(16.0).strong());
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
    });
}

fn stat_row(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(title).strong());
            ui.label(
                egui::RichText::new(subtitle)
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
        });
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let response = ui.add_sized(
                [ui.available_width().max(96.0), 22.0],
                egui::Label::new(egui::RichText::new(value).strong()).truncate(),
            );
            if value.chars().count() > 24 {
                response.on_hover_text(value);
            }
        });
    });
}

fn palette_color(seed: usize) -> egui::Color32 {
    let palette = [
        egui::Color32::from_rgb(0x2F, 0x7F, 0x86),
        egui::Color32::from_rgb(0x3E, 0x8E, 0x7A),
        egui::Color32::from_rgb(0x4B, 0x7B, 0xEC),
        egui::Color32::from_rgb(0x5F, 0x8D, 0x96),
        egui::Color32::from_rgb(0x2E, 0x8B, 0x57),
        egui::Color32::from_rgb(0xC9, 0x8B, 0x2E),
        egui::Color32::from_rgb(0x79, 0x95, 0x9B),
        egui::Color32::from_rgb(0x8A, 0xA8, 0xAD),
    ];
    palette[seed % palette.len()]
}

fn treemap_label_for_rect(name: &str, size_bytes: u64, rect: egui::Rect) -> Option<String> {
    if rect.width() < MIN_TREEMAP_LABEL_WIDTH || rect.height() < MIN_TREEMAP_LABEL_HEIGHT {
        return None;
    }

    let max_chars = ((rect.width() - 16.0) / 7.2).floor().max(6.0) as usize;
    let title = truncate_middle(name, max_chars);
    if rect.height() >= 58.0 && rect.width() >= 124.0 {
        Some(format!("{}\n{}", title, format_bytes(size_bytes)))
    } else {
        Some(title)
    }
}

fn truncate_middle(input: &str, max_chars: usize) -> String {
    let chars: Vec<char> = input.chars().collect();
    if chars.len() <= max_chars {
        return input.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    let head = (max_chars - 1) / 2;
    let tail = max_chars - head - 1;
    let left: String = chars.iter().take(head).collect();
    let right: String = chars
        .iter()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}…{}", left, right)
}

fn path_within_scope(path: &str, scope_path: &str) -> bool {
    if path == scope_path {
        return true;
    }
    let Some(rest) = path.strip_prefix(scope_path) else {
        return false;
    };
    rest.starts_with('\\') || rest.starts_with('/')
}

fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0usize;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else if value >= 100.0 {
        format!("{:.0} {}", value, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", value, UNITS[unit_idx])
    }
}

fn short_volume_label(volume: &dirotter_platform::VolumeInfo) -> String {
    #[cfg(target_os = "windows")]
    {
        return volume.mount_point.trim_end_matches(['\\', '/']).to_string();
    }

    #[cfg(not(target_os = "windows"))]
    {
        if volume.mount_point == "/" {
            volume.name.clone()
        } else {
            volume.mount_point.clone()
        }
    }
}

fn preferred_root_from_volumes(volumes: &[dirotter_platform::VolumeInfo]) -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(system_drive) = std::env::var("SystemDrive") {
            let system_root = format!("{}\\", system_drive.trim_end_matches(['\\', '/']));
            if volumes
                .iter()
                .any(|volume| volume.mount_point.eq_ignore_ascii_case(&system_root))
            {
                return system_root;
            }
        }
    }

    if let Some(first) = volumes.first() {
        return first.mount_point.clone();
    }

    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.display().to_string())
        .unwrap_or_else(|| ".".to_string())
}

fn detect_lang() -> Lang {
    let locale = std::env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .unwrap_or_default()
        .to_lowercase();

    if locale.starts_with("zh") {
        Lang::Zh
    } else {
        Lang::En
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use dirotter_core::{NodeKind, NodeStore};

    #[test]
    fn format_bytes_is_human_readable() {
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(1_536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn format_count_adds_grouping() {
        assert_eq!(format_count(12), "12");
        assert_eq!(format_count(1_234_567), "1,234,567");
    }

    #[test]
    fn truncate_middle_keeps_ends() {
        let truncated = truncate_middle("very-long-file-name.iso", 10);
        assert!(truncated.starts_with("very"));
        assert!(truncated.ends_with(".iso"));
    }

    #[test]
    fn treemap_label_hides_small_tiles() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(60.0, 20.0));
        assert!(treemap_label_for_rect("folder", 1024, rect).is_none());
    }

    #[test]
    fn preferred_root_uses_first_volume_when_available() {
        let volumes = vec![dirotter_platform::VolumeInfo {
            mount_point: "D:\\".to_string(),
            name: "Data".to_string(),
            total_bytes: 10,
            available_bytes: 5,
        }];
        assert_eq!(preferred_root_from_volumes(&volumes), "D:\\");
    }

    #[test]
    fn rebuild_store_without_target_removes_subtree_and_updates_rollup() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "e:\\".into(), "e:\\".into(), NodeKind::Dir, 0);
        let keep = store.add_node(
            Some(root),
            "keep".into(),
            "e:\\keep".into(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(keep),
            "file.bin".into(),
            "e:\\keep\\file.bin".into(),
            NodeKind::File,
            10,
        );
        let drop_dir = store.add_node(
            Some(root),
            "drop".into(),
            "e:\\drop".into(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(drop_dir),
            "trash.bin".into(),
            "e:\\drop\\trash.bin".into(),
            NodeKind::File,
            20,
        );
        store.rollup();

        let target = SelectedTarget {
            name: "drop".into(),
            path: "e:\\drop".into(),
            size_bytes: 20,
            kind: NodeKind::Dir,
            file_count: 1,
            dir_count: 1,
        };

        let rebuilt = DirOtterNativeApp::rebuild_store_without_target(&store, &target)
            .expect("rebuilt store");
        let root_node = rebuilt
            .nodes
            .iter()
            .find(|node| node.parent.is_none())
            .expect("root node");

        assert!(!rebuilt.path_index.contains_key("e:\\drop"));
        assert!(!rebuilt.path_index.contains_key("e:\\drop\\trash.bin"));
        assert_eq!(root_node.size_subtree, 10);
        assert_eq!(root_node.file_count, 1);
    }

    #[test]
    fn contextual_ranked_files_panel_uses_selected_directory_scope() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "d:\\".into(), "d:\\".into(), NodeKind::Dir, 0);
        let sdk = store.add_node(
            Some(root),
            "sdk".into(),
            "d:\\appdata\\local\\sdk".into(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(sdk),
            "system.img".into(),
            "d:\\appdata\\local\\sdk\\system.img".into(),
            NodeKind::File,
            20,
        );
        store.add_node(
            Some(sdk),
            "userdata.img".into(),
            "d:\\appdata\\local\\sdk\\userdata.img".into(),
            NodeKind::File,
            10,
        );
        store.add_node(
            Some(root),
            "other.bin".into(),
            "d:\\other.bin".into(),
            NodeKind::File,
            100,
        );
        store.rollup();

        let app = DirOtterNativeApp {
            egui_ctx: egui::Context::default(),
            page: Page::CurrentScan,
            available_volumes: Vec::new(),
            root_input: "d:\\".into(),
            status: "Completed".into(),
            summary: ScanSummary::default(),
            store: Some(store),
            scan_session: None,
            delete_session: None,
            scan_profile: ScanProfile::Ssd,
            snapshot_interval_ms: 75,
            event_batch_size: 256,
            scan_current_path: None,
            scan_last_event_at: None,
            scan_dropped_batches: 0,
            scan_dropped_snapshots: 0,
            scan_dropped_progress: 0,
            pending_batch_events: VecDeque::new(),
            pending_snapshots: VecDeque::new(),
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            execution_report: None,
            pending_delete_confirmation: None,
            queued_delete: None,
            explorer_feedback: None,
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: Lang::En,
            theme_dark: true,
            cache: CacheStore::new(":memory:").expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState {
                selected_node: Some(sdk),
                selected_path: Some("d:\\appdata\\local\\sdk".into()),
                source: Some(SelectionSource::Table),
            },
            error_filter: ErrorFilter::All,
            treemap_cache: TreemapViewportCache::default(),
        };

        let (_, _, files) = app.contextual_ranked_files_panel(8);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|(path, _)| path.starts_with("d:\\appdata\\local\\sdk\\")));
        assert_eq!(files[0].0, "d:\\appdata\\local\\sdk\\system.img");
    }
}
