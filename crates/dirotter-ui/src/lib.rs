mod advanced_pages;
mod cleanup;
mod controller;
mod dashboard;
mod duplicates_pages;
mod i18n;
mod result_pages;
mod settings_pages;
mod theme;
#[allow(dead_code)]
mod ui_shell;
mod view_models;

use dirotter_actions::{
    build_deletion_plan_with_origin, ActionFailureKind, ExecutionMode, ExecutionReport,
    SelectionOrigin,
};
use dirotter_cache::CacheStore;
use dirotter_core::{
    ErrorKind, NodeId, NodeKind, NodeStore, RiskLevel, ScanErrorRecord, ScanSummary, SnapshotDelta,
};
use dirotter_scan::{start_scan, BatchEntry, ScanConfig, ScanEvent, ScanMode};
use dirotter_telemetry as telemetry;
use eframe::egui;
#[cfg(test)]
use i18n::has_translation;
use i18n::{
    detect_lang_from_locale, lang_native_label, lang_picker_label, lang_setting_value,
    parse_lang_setting, supported_languages, translate_ui,
};
#[cfg(test)]
use i18n::{translate_es, translate_fr};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cleanup::{CleanupAnalysis, CleanupCandidate, CleanupCategory};
use controller::{
    start_delete_finalize_session, start_delete_session, start_duplicate_scan_session,
    start_memory_release_session, start_result_store_load_session, take_finished_delete,
    take_finished_delete_finalize, take_finished_duplicate_scan, take_finished_memory_release,
    take_finished_result_store_load, DeleteFinalizeInput, DeleteFinalizeSession, DeleteSession,
    DuplicateScanSession, MemoryReleaseSession, QueuedDeleteRequest, ResultStoreLoadSession,
};

const MAX_PENDING_BATCH_EVENTS: usize = 32;
const MAX_PENDING_SNAPSHOTS: usize = 8;
const MAX_LIVE_FILES: usize = 20_000;
const MAX_CLEANUP_DETAIL_ITEMS: usize = 48;
const MAX_CACHE_CLEANUP_ITEMS: usize = 96;
const MAX_CLEANUP_ITEMS_PER_CATEGORY: usize = 24;
const MAX_BLOCKED_ITEMS_PER_CATEGORY: usize = 12;
const MAX_CLEANUP_TOTAL_ITEMS: usize = 180;
const MIN_CLEANUP_BYTES: u64 = 64 * 1024 * 1024;
const MIN_CACHE_DIR_BYTES: u64 = 16 * 1024 * 1024;
const MEMORY_STATUS_REFRESH_MS: u64 = 2_000;
const IDLE_MEMORY_RELEASE_SECS: u64 = 45;
const AUTO_MEMORY_RELEASE_COOLDOWN_SECS: u64 = 120;
const HIGH_MEMORY_LOAD_PERCENT: u32 = 85;
const NAV_WIDTH: f32 = 224.0;
const INSPECTOR_WIDTH: f32 = 320.0;
const TOOLBAR_HEIGHT: f32 = 56.0;
const STATUSBAR_HEIGHT: f32 = 26.0;
const CONTROL_RADIUS: u8 = 0;
const CARD_PADDING: f32 = 24.0;
const CARD_STROKE_WIDTH: f32 = 1.0;
const CONTROL_HEIGHT: f32 = 42.0;
const NAV_ITEM_HEIGHT: f32 = 56.0;
const STATUS_BADGE_HEIGHT: f32 = 32.0;
const PAGE_MAX_WIDTH: f32 = 1360.0;
const DUPLICATES_PAGE_MAX_WIDTH: f32 = 1480.0;
const DASHBOARD_PAGE_MAX_WIDTH: f32 = 1280.0;
const SETTINGS_PAGE_MAX_WIDTH: f32 = 1040.0;
const PAGE_SIDE_GUTTER: f32 = 64.0;
const DUPLICATE_AUTO_SELECT_MIN_WASTE_BYTES: u64 = 16 * 1024 * 1024;
const DUPLICATE_AUTO_SELECT_MIN_AGE_DAYS: u64 = 30;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Dashboard,
    CurrentScan,
    Duplicates,
    Errors,
    Diagnostics,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Ar,
    De,
    En,
    He,
    Hi,
    Id,
    It,
    Ja,
    Ko,
    Nl,
    Pl,
    Ru,
    Zh,
    Fr,
    Es,
    Th,
    Tr,
    Uk,
    Vi,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppStatus {
    Idle,
    Scanning,
    Finalizing,
    Completed,
    Deleting,
    DeleteExecuted,
    DeleteFailed,
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectionSource {
    Table,
    Duplicate,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoostAction {
    StartScan,
    QuickCacheCleanup,
    ReviewSuggestions,
    NoImmediateAction,
}

struct ScanSession {
    cancel: Arc<AtomicBool>,
    relay: Arc<Mutex<ScanRelayState>>,
}

struct ScanFinalizeSession {
    relay: Arc<Mutex<ScanFinalizeRelayState>>,
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

#[derive(Default)]
struct ScanFinalizeRelayState {
    finished: Option<ScanFinalizePayload>,
}

struct FinishedPayload {
    summary: ScanSummary,
    store: NodeStore,
    errors: Vec<ScanErrorRecord>,
}

struct ScanFinalizePayload {
    summary: ScanSummary,
    store: NodeStore,
    errors: Vec<ScanErrorRecord>,
    cleanup_analysis: CleanupAnalysis,
}

#[derive(Clone)]
pub(crate) struct SelectedTarget {
    node_id: Option<NodeId>,
    name: Arc<str>,
    path: Arc<str>,
    size_bytes: u64,
    kind: NodeKind,
    file_count: u64,
    dir_count: u64,
}

#[derive(Clone)]
pub(crate) struct DeleteRequestScope {
    label: String,
    targets: Vec<SelectedTarget>,
    selection_origin: SelectionOrigin,
}

#[derive(Clone)]
struct CleanupDeleteRequest {
    label: String,
    targets: Vec<SelectedTarget>,
    estimated_bytes: u64,
    mode: ExecutionMode,
}

#[derive(Default)]
struct CleanupPanelState {
    analysis: Option<CleanupAnalysis>,
    detail_category: Option<CleanupCategory>,
    selected_paths: HashSet<Arc<str>>,
    pending_delete: Option<CleanupDeleteRequest>,
}

#[derive(Clone)]
struct DuplicateGroupSelection {
    keep_path: Arc<str>,
    enabled: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DuplicateSort {
    Waste,
    Size,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum DuplicateReviewMode {
    #[default]
    Quick,
    Full,
}

#[derive(Clone)]
struct DuplicateDeleteRequest {
    label: String,
    targets: Vec<SelectedTarget>,
    estimated_bytes: u64,
    group_count: usize,
}

#[derive(Default)]
struct DuplicatePanelState {
    groups: Vec<dirotter_dup::DuplicateGroup>,
    visible_groups: usize,
    expanded_group_ids: HashSet<u64>,
    selections: HashMap<u64, DuplicateGroupSelection>,
    pending_delete: Option<DuplicateDeleteRequest>,
    sort: Option<DuplicateSort>,
    show_large_only: bool,
    follow_recommended_selection: bool,
    total_duplicate_files: usize,
    total_reclaimable_bytes: u64,
    selected_groups_cache: usize,
    selected_files_cache: usize,
    selected_bytes_cache: u64,
    selection_totals_dirty: bool,
    review_mode: DuplicateReviewMode,
    review_completed: bool,
}

#[derive(Default)]
struct DuplicatePrepState {
    by_size: HashMap<u64, Vec<Arc<str>>>,
    scanned_files: usize,
    candidate_groups: usize,
    candidate_files: usize,
}

#[derive(Clone)]
struct PendingDeleteConfirmation {
    request: DeleteRequestScope,
    risk: RiskLevel,
}

enum CleanupDetailsAction {
    SelectCategory(CleanupCategory),
    ToggleTarget { path: Arc<str>, checked: bool },
    FocusTarget(SelectedTarget),
    SelectAllSafe,
    ClearSelected,
    OpenSelectedLocation,
    TriggerPrimary,
    TriggerPermanent,
    Close,
}

enum DeleteConfirmAction {
    Confirm,
    Close,
}

enum CleanupDeleteConfirmAction {
    Confirm,
    Close,
}

enum DuplicateDeleteConfirmAction {
    RecycleBin,
    Permanent,
    Close,
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
    scan_finalize_session: Option<ScanFinalizeSession>,
    delete_session: Option<DeleteSession>,
    delete_finalize_session: Option<DeleteFinalizeSession>,
    duplicate_scan_session: Option<DuplicateScanSession>,
    result_store_load_session: Option<ResultStoreLoadSession>,
    memory_release_session: Option<MemoryReleaseSession>,
    scan_mode: ScanMode,
    scan_current_path: Option<Arc<str>>,
    scan_last_event_at: Option<Instant>,
    scan_cancel_requested: bool,
    scan_dropped_batches: u64,
    scan_dropped_snapshots: u64,
    scan_dropped_progress: u64,

    pending_batch_events: VecDeque<Vec<BatchEntry>>,
    pending_snapshots: VecDeque<SnapshotDelta>,
    live_files: Vec<dirotter_scan::RankedPath>,
    live_top_files: Vec<dirotter_scan::RankedPath>,
    live_top_dirs: Vec<dirotter_scan::RankedPath>,
    completed_top_files: Vec<dirotter_scan::RankedPath>,
    completed_top_dirs: Vec<dirotter_scan::RankedPath>,
    last_coalesce_commit: Instant,
    cleanup: CleanupPanelState,
    duplicates: DuplicatePanelState,
    duplicate_prep: DuplicatePrepState,

    execution_report: Option<ExecutionReport>,
    pending_delete_confirmation: Option<PendingDeleteConfirmation>,
    queued_delete: Option<QueuedDeleteRequest>,
    explorer_feedback: Option<(String, bool)>,
    maintenance_feedback: Option<(String, bool)>,
    last_system_memory_release: Option<dirotter_platform::SystemMemoryReleaseReport>,
    process_memory: Option<dirotter_platform::ProcessMemoryStats>,
    system_memory: Option<dirotter_platform::SystemMemoryStats>,
    last_memory_status_refresh: Option<Instant>,
    last_user_activity: Instant,
    last_auto_memory_release_at: Option<Instant>,

    errors: Vec<ScanErrorRecord>,
    language: Lang,
    theme_dark: bool,
    advanced_tools_enabled: bool,
    cache: CacheStore,

    perf: PerfMetrics,
    diagnostics_json: String,
    selection: SelectionState,
    error_filter: ErrorFilter,
    missing_result_store_root: Option<String>,
}

impl DirOtterNativeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        let cache = CacheStore::open_default()
            .or_else(|_| CacheStore::open_ephemeral())
            .expect("open settings and session storage");
        let language = cache
            .get_setting("language")
            .ok()
            .flatten()
            .and_then(|v| parse_lang_setting(&v))
            .unwrap_or_else(detect_lang);
        let theme_dark = cache
            .get_setting("theme")
            .ok()
            .flatten()
            .map(|v| v != "light")
            .unwrap_or(true);
        let advanced_tools_enabled = cache
            .get_setting("advanced_tools")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        let scan_mode = cache
            .get_setting("scan_mode")
            .ok()
            .flatten()
            .and_then(|value| ScanMode::from_setting(&value))
            .unwrap_or(ScanMode::Quick);
        let available_volumes = dirotter_platform::list_volumes().unwrap_or_default();
        let initial_root = preferred_root_from_volumes(&available_volumes);
        std::thread::spawn(|| {
            let _ = dirotter_platform::purge_all_staging_roots();
        });

        let mut app = Self {
            egui_ctx: cc.egui_ctx.clone(),
            page: Page::Dashboard,
            available_volumes,
            root_input: initial_root,
            status: AppStatus::Idle,
            summary: ScanSummary::default(),
            store: None,
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
            delete_finalize_session: None,
            duplicate_scan_session: None,
            result_store_load_session: None,
            memory_release_session: None,
            scan_mode,
            scan_current_path: None,
            scan_last_event_at: None,
            scan_cancel_requested: false,
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
            cleanup: CleanupPanelState::default(),
            duplicates: DuplicatePanelState {
                sort: Some(DuplicateSort::Waste),
                follow_recommended_selection: true,
                ..DuplicatePanelState::default()
            },
            duplicate_prep: DuplicatePrepState::default(),
            execution_report: None,
            pending_delete_confirmation: None,
            queued_delete: None,
            explorer_feedback: None,
            maintenance_feedback: None,
            last_system_memory_release: None,
            process_memory: None,
            system_memory: None,
            last_memory_status_refresh: None,
            last_user_activity: Instant::now(),
            last_auto_memory_release_at: None,
            errors: Vec::new(),
            language,
            theme_dark,
            advanced_tools_enabled,
            cache,
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
            missing_result_store_root: None,
        };

        if app.advanced_tools_enabled {
            if let Ok(Some(snapshot)) = app.cache.load_latest_snapshot(&app.root_input) {
                app.store = Some(snapshot);
                app.sync_summary_from_store();
                app.sync_rankings_from_store();
                app.refresh_cleanup_analysis();
            }
        }
        app.apply_theme(&cc.egui_ctx);
        app.refresh_memory_status();
        app.refresh_diagnostics();
        app
    }

    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        translate_ui(self.language, zh, en)
    }

    fn execution_failure_details_window_id() -> egui::Id {
        egui::Id::new("execution_failure_details_window_open")
    }

    fn execution_failure_details_open(&self) -> bool {
        self.egui_ctx
            .data(|data| data.get_temp::<bool>(Self::execution_failure_details_window_id()))
            .unwrap_or(false)
    }

    fn set_execution_failure_details_open(&self, open: bool) {
        self.egui_ctx.data_mut(|data| {
            let id = Self::execution_failure_details_window_id();
            if open {
                data.insert_temp(id, true);
            } else {
                data.remove::<bool>(id);
            }
        });
    }

    fn set_language(&mut self, language: Lang) {
        self.language = language;
        let _ = self
            .cache
            .set_setting("language", lang_setting_value(language));
    }

    fn set_advanced_tools_enabled(&mut self, enabled: bool) {
        self.advanced_tools_enabled = enabled;
        let _ = self
            .cache
            .set_setting("advanced_tools", if enabled { "true" } else { "false" });
        self.refresh_diagnostics();
        if !enabled && matches!(self.page, Page::Errors | Page::Diagnostics) {
            self.page = Page::Dashboard;
        }
    }

    fn selected_scan_config(&self) -> ScanConfig {
        ScanConfig::for_mode(self.scan_mode)
    }

    fn scan_mode_title(&self, mode: ScanMode) -> &'static str {
        match mode {
            ScanMode::Quick => self.t("推荐策略", "Recommended strategy"),
            ScanMode::Deep => self.t("复杂目录", "Complex Directory"),
            ScanMode::LargeDisk => self.t("外置/超大硬盘", "External / Huge Drive"),
        }
    }

    fn scan_mode_description(&self, mode: ScanMode) -> &'static str {
        match mode {
            ScanMode::Quick => self.t(
                "使用默认的响应式节奏完整扫描，适合日常整理和大多数本地磁盘。",
                "Complete scanning with the default responsive pacing for daily cleanup and most local disks.",
            ),
            ScanMode::Deep => self.t(
                "放慢结果发布节奏，适合首次排查复杂目录树。",
                "Slow the publishing cadence for first-pass investigations of complex directory trees.",
            ),
            ScanMode::LargeDisk => self.t(
                "使用最保守的批处理和界面刷新节奏，适合外置盘、超大硬盘或文件数极多的目录。",
                "Use the most conservative batching and UI refresh cadence for external drives, very large disks, or extremely dense folders.",
            ),
        }
    }

    fn scan_mode_note(&self) -> &'static str {
        self.t(
            "所有节奏都会扫描同一范围并产出同一组结果，只改变批处理和界面刷新频率。",
            "All pacing options scan the same scope and produce the same result set. They only change batching and UI refresh cadence.",
        )
    }

    fn set_scan_mode(&mut self, mode: ScanMode) {
        self.scan_mode = mode;
        let _ = self
            .cache
            .set_setting("scan_mode", self.scan_mode.as_setting_value());
    }

    fn status_text(&self) -> &'static str {
        match self.status {
            AppStatus::Idle => self.t("空闲", "Idle"),
            AppStatus::Scanning => self.t("扫描中", "Scanning"),
            AppStatus::Finalizing => self.t("整理结果中", "Finalizing"),
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
        // 使用 theme.rs 中的主题系统
        let palette = if self.theme_dark {
            theme::ColorPalette::dark()
        } else {
            theme::ColorPalette::light()
        };
        let theme_mode = if self.theme_dark {
            theme::ThemeMode::Dark
        } else {
            theme::ThemeMode::Light
        };
        theme::apply_theme(&mut style.visuals, theme_mode, &palette);
        style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(CONTROL_RADIUS as f32);
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

    fn target_from_node_id(&self, node_id: NodeId) -> Option<SelectedTarget> {
        let store = self.store.as_ref()?;
        let node = store.nodes.get(node_id.0)?;
        Some(SelectedTarget {
            node_id: Some(node_id),
            name: store
                .resolve_string_arc(node.name_id)
                .unwrap_or_else(|| Arc::from("")),
            path: node.path.clone(),
            size_bytes: node.size_subtree.max(node.size_self),
            kind: node.kind,
            file_count: node.file_count,
            dir_count: node.dir_count,
        })
    }

    fn selected_target(&self) -> Option<SelectedTarget> {
        if let Some(node_id) = self.selection.selected_node {
            if let Some(target) = self.target_from_node_id(node_id) {
                return Some(target);
            }
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
            .map(Arc::<str>::from)
            .unwrap_or_else(|| Arc::<str>::from(path.as_str()));
        Some(SelectedTarget {
            node_id: None,
            name,
            path: Arc::<str>::from(path),
            size_bytes,
            kind,
            file_count: if kind == NodeKind::File { 1 } else { 0 },
            dir_count: if kind == NodeKind::Dir { 1 } else { 0 },
        })
    }

    fn delete_request_for_target(
        target: SelectedTarget,
        selection_origin: SelectionOrigin,
    ) -> DeleteRequestScope {
        DeleteRequestScope {
            label: target.path.to_string(),
            targets: vec![target],
            selection_origin,
        }
    }

    fn selection_matches_path(&self, path: &str) -> bool {
        self.selection.selected_path.as_deref() == Some(path)
    }

    fn selection_matches_target(&self, target: &SelectedTarget) -> bool {
        target
            .node_id
            .is_some_and(|node_id| self.selection.selected_node == Some(node_id))
            || self.selection_matches_path(target.path.as_ref())
    }

    fn cleanup_category_label(&self, category: CleanupCategory) -> &'static str {
        match category {
            CleanupCategory::Cache => self.t("缓存文件", "Cache"),
            CleanupCategory::Downloads => self.t("下载文件", "Downloads"),
            CleanupCategory::Video => self.t("视频文件", "Videos"),
            CleanupCategory::Archive => self.t("压缩包", "Archives"),
            CleanupCategory::Installer => self.t("安装包", "Installers"),
            CleanupCategory::Image => self.t("图片文件", "Images"),
            CleanupCategory::System => self.t("系统文件", "System"),
            CleanupCategory::Other => self.t("其他文件", "Other"),
        }
    }

    fn cleanup_risk_label(&self, risk: RiskLevel) -> &'static str {
        match risk {
            RiskLevel::Low => self.t("可清理", "Safe"),
            RiskLevel::Medium => self.t("谨慎", "Warning"),
            RiskLevel::High => self.t("手动处理", "Manual Review"),
        }
    }

    fn cleanup_reason_text(&self, item: &CleanupCandidate) -> &'static str {
        match item.category {
            CleanupCategory::Cache => self.t(
                "命中 AppData / Temp / Cache 路径规则。",
                "Matched AppData / Temp / Cache path rules.",
            ),
            CleanupCategory::Downloads => self.t(
                "位于 Downloads 目录，通常需要人工确认。",
                "Located under Downloads and usually needs human review.",
            ),
            CleanupCategory::Video => self.t("大体积视频文件。", "Large video file."),
            CleanupCategory::Archive => self.t("压缩包或归档文件。", "Archive package."),
            CleanupCategory::Installer => self.t("安装程序或安装包。", "Installer package."),
            CleanupCategory::Image => self.t("图片类文件。", "Image asset."),
            CleanupCategory::System => self.t(
                "系统目录或系统托管文件。请打开所在位置后再手动确认处理。",
                "System path or system-managed file. Open its location and review manually.",
            ),
            CleanupCategory::Other => self.t("大体积未分类文件。", "Large unclassified file."),
        }
    }

    fn cleanup_risk_color(&self, risk: RiskLevel) -> egui::Color32 {
        match risk {
            RiskLevel::Low => egui::Color32::from_rgb(0x4C, 0xB1, 0x7D),
            RiskLevel::Medium => egui::Color32::from_rgb(0xD9, 0xA4, 0x41),
            RiskLevel::High => danger_red(),
        }
    }

    fn duplicate_safety_label(&self, class: dirotter_dup::DuplicateSafetyClass) -> &'static str {
        match class {
            dirotter_dup::DuplicateSafetyClass::NeverAutoDelete => {
                self.t("手动处理", "Manual Review Only")
            }
            dirotter_dup::DuplicateSafetyClass::ManualReview => {
                self.t("请确认保留版本", "Review Needed")
            }
            dirotter_dup::DuplicateSafetyClass::CautiousAuto => {
                self.t("可谨慎处理", "Cautious Auto")
            }
            dirotter_dup::DuplicateSafetyClass::SafeAuto => self.t("可安全处理", "Safe Auto"),
        }
    }

    fn duplicate_safety_note(
        &self,
        decision: &dirotter_dup::DuplicateSafetyDecision,
    ) -> &'static str {
        match decision.class {
            dirotter_dup::DuplicateSafetyClass::NeverAutoDelete => self.t(
                "该组命中了系统目录、安装目录或运行依赖规则，不会自动选择删除项。",
                "This group matched system paths, installed app paths, or runtime dependency rules, so no files are auto-selected for deletion.",
            ),
            dirotter_dup::DuplicateSafetyClass::ManualReview => self.t(
                "该组位于用户资料或高价值目录，请确认你真正想保留的版本。",
                "This group lives in user content or other high-value locations. Confirm which version you truly want to keep.",
            ),
            dirotter_dup::DuplicateSafetyClass::CautiousAuto => self.t(
                "该组多见于下载副本或导出副本，可自动给出建议，但删除前仍需二次确认。",
                "This group usually comes from repeated downloads or exported copies. Suggestions can be auto-selected, but deletion still requires confirmation.",
            ),
            dirotter_dup::DuplicateSafetyClass::SafeAuto => self.t(
                "该组属于低风险重复文件，适合进入自动整理流程。",
                "This group is low-risk duplicate content and fits the automatic cleanup flow.",
            ),
        }
    }

    fn cleanup_category_color(&self, category: CleanupCategory) -> egui::Color32 {
        match category {
            CleanupCategory::Cache => river_teal(),
            CleanupCategory::Downloads => egui::Color32::from_rgb(0x78, 0xB3, 0x5C),
            CleanupCategory::Video => egui::Color32::from_rgb(0x4D, 0x9C, 0xD3),
            CleanupCategory::Archive => egui::Color32::from_rgb(0xC8, 0x8F, 0x44),
            CleanupCategory::Installer => egui::Color32::from_rgb(0xD7, 0x73, 0x58),
            CleanupCategory::Image => egui::Color32::from_rgb(0xB2, 0x7A, 0xC7),
            CleanupCategory::System => danger_red(),
            CleanupCategory::Other => {
                if self.theme_dark {
                    egui::Color32::from_rgb(0x92, 0x9B, 0xA1)
                } else {
                    egui::Color32::from_rgb(0x66, 0x71, 0x75)
                }
            }
        }
    }

    fn refresh_cleanup_analysis(&mut self) {
        self.apply_cleanup_analysis(self.store.as_ref().map(Self::build_cleanup_analysis));
    }

    fn apply_cleanup_analysis(&mut self, analysis: Option<CleanupAnalysis>) {
        self.cleanup.analysis = analysis;
        self.cleanup.detail_category = None;
        self.cleanup.selected_paths = self
            .cleanup
            .analysis
            .as_ref()
            .map(|analysis| {
                analysis
                    .items
                    .iter()
                    .filter(|item| item.risk == RiskLevel::Low)
                    .map(|item| item.target.path.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.cleanup.pending_delete = None;
    }

    fn build_cleanup_analysis(store: &NodeStore) -> CleanupAnalysis {
        cleanup::build_cleanup_analysis(store)
    }

    fn cleanup_items_for_category(&self, category: CleanupCategory) -> Vec<CleanupCandidate> {
        let Some(analysis) = self.cleanup.analysis.as_ref() else {
            return Vec::new();
        };
        let mut items: Vec<_> = analysis
            .items
            .iter()
            .filter(|item| item.category == category)
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            b.target
                .size_bytes
                .cmp(&a.target.size_bytes)
                .then_with(|| a.target.path.cmp(&b.target.path))
        });
        items.truncate(MAX_CLEANUP_DETAIL_ITEMS);
        items
    }

    fn cleanup_delete_mode_for_category(category: CleanupCategory) -> ExecutionMode {
        cleanup::cleanup_delete_mode_for_category(category)
    }

    fn can_fast_purge_path(&self, path: &str) -> bool {
        cleanup::can_fast_purge_path(path)
    }

    fn selected_cleanup_totals(&self, category: CleanupCategory) -> (usize, u64) {
        self.cleanup_items_for_category(category)
            .into_iter()
            .filter(|item| {
                item.risk != RiskLevel::High
                    && self
                        .cleanup
                        .selected_paths
                        .contains(item.target.path.as_ref())
            })
            .fold((0usize, 0u64), |(count, bytes), item| {
                (count + 1, bytes.saturating_add(item.target.size_bytes))
            })
    }

    fn select_all_safe_cleanup_items(&mut self, category: CleanupCategory) {
        for item in self.cleanup_items_for_category(category) {
            if item.risk != RiskLevel::High {
                self.cleanup.selected_paths.insert(item.target.path.clone());
            }
        }
    }

    fn clear_selected_cleanup_items(&mut self, category: CleanupCategory) {
        for item in self.cleanup_items_for_category(category) {
            self.cleanup
                .selected_paths
                .remove(item.target.path.as_ref());
        }
    }

    fn queue_cleanup_delete(&mut self, request: CleanupDeleteRequest) {
        if request.targets.is_empty() {
            return;
        }
        self.cleanup.pending_delete = Some(request);
        self.egui_ctx.request_repaint();
    }

    fn queue_cleanup_category_delete(&mut self, category: CleanupCategory) {
        self.queue_cleanup_category_delete_with_mode(
            category,
            Self::cleanup_delete_mode_for_category(category),
        );
    }

    fn queue_cleanup_category_delete_with_mode(
        &mut self,
        category: CleanupCategory,
        mode: ExecutionMode,
    ) {
        let selected_targets: Vec<SelectedTarget> = self
            .cleanup_items_for_category(category)
            .into_iter()
            .filter(|item| {
                item.risk != RiskLevel::High
                    && self
                        .cleanup
                        .selected_paths
                        .contains(item.target.path.as_ref())
            })
            .map(|item| item.target)
            .collect();
        let estimated_bytes = selected_targets
            .iter()
            .map(|target| target.size_bytes)
            .sum();
        let label = match mode {
            ExecutionMode::FastPurge => self.t("快速清理选中缓存", "Fast Cleanup Selected"),
            ExecutionMode::RecycleBin => self.t("移到回收站", "Move to Recycle Bin"),
            ExecutionMode::Permanent => self.t("永久删除选中项", "Delete Selected Permanently"),
        }
        .to_string();
        self.queue_cleanup_delete(CleanupDeleteRequest {
            label,
            targets: selected_targets,
            estimated_bytes,
            mode,
        });
    }

    fn queue_cleanup_cache_delete(&mut self) {
        let Some(analysis) = self.cleanup.analysis.as_ref() else {
            return;
        };
        let targets: Vec<SelectedTarget> = analysis
            .items
            .iter()
            .filter(|item| item.category == CleanupCategory::Cache && item.risk == RiskLevel::Low)
            .map(|item| item.target.clone())
            .collect();
        self.queue_cleanup_delete(CleanupDeleteRequest {
            label: self.t("一键清理缓存", "Quick Cache Cleanup").to_string(),
            estimated_bytes: analysis.quick_clean_bytes,
            targets,
            mode: ExecutionMode::FastPurge,
        });
    }

    fn reset_duplicate_review(&mut self) {
        let review_mode = self.duplicates.review_mode;
        self.duplicate_scan_session = None;
        self.duplicates = DuplicatePanelState {
            sort: Some(DuplicateSort::Waste),
            follow_recommended_selection: true,
            selection_totals_dirty: true,
            review_mode,
            review_completed: false,
            ..DuplicatePanelState::default()
        };
    }

    fn duplicate_dup_config(&self) -> dirotter_dup::DupConfig {
        let mut cfg = dirotter_dup::DupConfig::default();
        match self.duplicates.review_mode {
            DuplicateReviewMode::Quick => {
                cfg.min_candidate_size = 1024 * 1024;
                cfg.min_candidate_total_waste = 8 * 1024 * 1024;
                cfg.quick_actionable_only = true;
            }
            DuplicateReviewMode::Full => {}
        }
        cfg
    }

    fn set_duplicate_review_mode(&mut self, mode: DuplicateReviewMode) {
        if self.duplicates.review_mode == mode || self.duplicate_scan_session.is_some() {
            return;
        }
        self.duplicates.review_mode = mode;
        self.reset_duplicate_review();
        self.reset_duplicate_prep();
        self.start_duplicate_scan_if_needed();
        self.egui_ctx.request_repaint();
    }

    fn reset_duplicate_prep(&mut self) {
        self.duplicate_prep = DuplicatePrepState::default();
    }

    fn absorb_duplicate_prep_batch(&mut self, batch: &[BatchEntry]) {
        let cfg = self.duplicate_dup_config();
        for item in batch {
            if item.is_dir {
                continue;
            }

            self.duplicate_prep.scanned_files = self.duplicate_prep.scanned_files.saturating_add(1);
            if item.size < cfg.min_candidate_size
                || (cfg.quick_actionable_only
                    && !dirotter_dup::allow_quick_duplicate_candidate_path(item.path.as_ref()))
            {
                continue;
            }

            let bucket = self.duplicate_prep.by_size.entry(item.size).or_default();
            let next_count = bucket.len() + 1;
            let current_waste = item
                .size
                .saturating_mul(bucket.len().saturating_sub(1) as u64);
            let next_waste = item
                .size
                .saturating_mul(next_count.saturating_sub(1) as u64);
            if current_waste < cfg.min_candidate_total_waste
                && next_waste >= cfg.min_candidate_total_waste
            {
                self.duplicate_prep.candidate_groups =
                    self.duplicate_prep.candidate_groups.saturating_add(1);
                self.duplicate_prep.candidate_files = self
                    .duplicate_prep
                    .candidate_files
                    .saturating_add(next_count);
            } else if next_waste >= cfg.min_candidate_total_waste {
                self.duplicate_prep.candidate_files =
                    self.duplicate_prep.candidate_files.saturating_add(1);
            }
            bucket.push(item.path.clone());
        }
    }

    fn take_prebuilt_duplicate_candidates(
        &mut self,
    ) -> Option<Vec<dirotter_dup::DuplicateSizeCandidate>> {
        if self.duplicate_prep.by_size.is_empty() {
            return None;
        }

        let cfg = self.duplicate_dup_config();
        let by_size = std::mem::take(&mut self.duplicate_prep.by_size);
        let mut candidates: Vec<_> = by_size
            .into_iter()
            .filter_map(|(size, paths)| {
                let total_waste = size.saturating_mul(paths.len().saturating_sub(1) as u64);
                (paths.len() >= 2 && total_waste >= cfg.min_candidate_total_waste).then_some(
                    dirotter_dup::DuplicateSizeCandidate {
                        size,
                        paths: paths.into_iter().map(|path| path.to_string()).collect(),
                    },
                )
            })
            .collect();
        dirotter_dup::sort_size_candidates(&mut candidates);
        Some(candidates)
    }

    fn duplicate_prep_snapshot(&self) -> (usize, usize, usize) {
        (
            self.duplicate_prep.scanned_files,
            self.duplicate_prep.candidate_groups,
            self.duplicate_prep.candidate_files,
        )
    }

    fn duplicate_group_auto_select_enabled(&self, group: &dirotter_dup::DuplicateGroup) -> bool {
        if !group.safety.auto_select_allowed
            || group.total_waste < DUPLICATE_AUTO_SELECT_MIN_WASTE_BYTES
        {
            return false;
        }

        let Some(newest_modified) = group
            .files
            .iter()
            .filter_map(|file| file.modified_unix_secs)
            .max()
        else {
            return true;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let min_age_secs = DUPLICATE_AUTO_SELECT_MIN_AGE_DAYS * 24 * 60 * 60;
        now.saturating_sub(newest_modified) >= min_age_secs
    }

    fn start_duplicate_scan_if_needed(&mut self) {
        if self.scan_active()
            || self.delete_active()
            || self.duplicate_scan_session.is_some()
            || self.duplicates.review_completed
            || !self.duplicates.groups.is_empty()
        {
            return;
        }
        let Some(store) = self.store.as_ref().cloned() else {
            return;
        };
        let cfg = self.duplicate_dup_config();
        let prebuilt_candidates = self.take_prebuilt_duplicate_candidates();
        self.duplicate_scan_session = Some(start_duplicate_scan_session(
            self.egui_ctx.clone(),
            store,
            prebuilt_candidates,
            cfg,
        ));
    }

    fn process_duplicate_scan_events(&mut self) {
        if self.duplicate_scan_session.is_none() {
            return;
        }
        let finished = {
            let session = self
                .duplicate_scan_session
                .as_ref()
                .expect("duplicate scan session");
            take_finished_duplicate_scan(session)
        };
        if let Some(payload) = finished {
            self.apply_duplicate_groups(payload.groups);
            self.duplicate_scan_session = None;
        }
    }

    fn apply_duplicate_groups(&mut self, groups: Vec<dirotter_dup::DuplicateGroup>) {
        self.duplicates.groups = groups;
        self.duplicates.review_completed = true;
        self.sort_duplicate_groups();
        self.duplicates.visible_groups = self.duplicates.groups.len().min(20);
        self.duplicates.pending_delete = None;
        self.duplicates.total_duplicate_files = self
            .duplicates
            .groups
            .iter()
            .map(|group| group.files.len())
            .sum();
        self.duplicates.total_reclaimable_bytes = self
            .duplicates
            .groups
            .iter()
            .map(|group| group.total_waste)
            .sum();
        self.duplicates.selection_totals_dirty = true;
        if self.duplicates.follow_recommended_selection || self.duplicates.selections.is_empty() {
            self.reset_duplicate_selection_to_recommended();
        }
    }

    fn sort_duplicate_groups(&mut self) {
        match self.duplicates.sort.unwrap_or(DuplicateSort::Waste) {
            DuplicateSort::Waste => self.duplicates.groups.sort_by(|a, b| {
                b.total_waste
                    .cmp(&a.total_waste)
                    .then_with(|| b.size.cmp(&a.size))
                    .then_with(|| a.id.cmp(&b.id))
            }),
            DuplicateSort::Size => self.duplicates.groups.sort_by(|a, b| {
                b.size
                    .cmp(&a.size)
                    .then_with(|| b.total_waste.cmp(&a.total_waste))
                    .then_with(|| a.id.cmp(&b.id))
            }),
        }
    }

    fn reset_duplicate_selection_to_recommended(&mut self) {
        self.duplicates.follow_recommended_selection = true;
        self.duplicates.selections = self
            .duplicates
            .groups
            .iter()
            .filter_map(|group| {
                group
                    .files
                    .get(group.recommended_keep_index)
                    .map(|recommended| {
                        (
                            group.id,
                            DuplicateGroupSelection {
                                keep_path: Arc::<str>::from(recommended.path.clone()),
                                enabled: self.duplicate_group_auto_select_enabled(group),
                            },
                        )
                    })
            })
            .collect();
        self.duplicates.selection_totals_dirty = true;
    }

    fn clear_duplicate_selection(&mut self) {
        self.duplicates.follow_recommended_selection = false;
        let fallback: Vec<_> = self
            .duplicates
            .groups
            .iter()
            .filter_map(|group| {
                group
                    .files
                    .get(group.recommended_keep_index)
                    .map(|recommended| (group.id, Arc::<str>::from(recommended.path.clone())))
            })
            .collect();
        for (group_id, keep_path) in fallback {
            self.duplicates.selections.insert(
                group_id,
                DuplicateGroupSelection {
                    keep_path,
                    enabled: false,
                },
            );
        }
        self.duplicates.selection_totals_dirty = true;
    }

    fn duplicate_group_selection(
        &self,
        group: &dirotter_dup::DuplicateGroup,
    ) -> DuplicateGroupSelection {
        self.duplicates
            .selections
            .get(&group.id)
            .cloned()
            .or_else(|| {
                group
                    .files
                    .get(group.recommended_keep_index)
                    .map(|recommended| DuplicateGroupSelection {
                        keep_path: Arc::<str>::from(recommended.path.clone()),
                        enabled: self.duplicate_group_auto_select_enabled(group),
                    })
            })
            .unwrap_or_else(|| DuplicateGroupSelection {
                keep_path: Arc::<str>::from(""),
                enabled: false,
            })
    }

    fn set_duplicate_group_enabled(&mut self, group_id: u64, enabled: bool) {
        let Some(group) = self
            .duplicates
            .groups
            .iter()
            .find(|group| group.id == group_id)
        else {
            return;
        };
        self.duplicates.follow_recommended_selection = false;
        let current = self.duplicate_group_selection(group);
        self.duplicates.selections.insert(
            group_id,
            DuplicateGroupSelection {
                keep_path: current.keep_path,
                enabled,
            },
        );
        self.duplicates.selection_totals_dirty = true;
    }

    fn set_duplicate_group_keep_path(&mut self, group_id: u64, keep_path: Arc<str>) {
        let Some(group) = self
            .duplicates
            .groups
            .iter()
            .find(|group| group.id == group_id)
        else {
            return;
        };
        self.duplicates.follow_recommended_selection = false;
        let current = self.duplicate_group_selection(group);
        self.duplicates.selections.insert(
            group_id,
            DuplicateGroupSelection {
                keep_path,
                enabled: current.enabled,
            },
        );
        self.duplicates.selection_totals_dirty = true;
    }

    fn duplicate_delete_totals(&mut self) -> (usize, usize, u64) {
        if self.duplicates.selection_totals_dirty {
            let (groups, files, bytes) = self.duplicates.groups.iter().fold(
                (0usize, 0usize, 0u64),
                |(groups, files, bytes), group| {
                    let selection = self.duplicate_group_selection(group);
                    if !selection.enabled {
                        return (groups, files, bytes);
                    }
                    let delete_count = group
                        .files
                        .iter()
                        .filter(|file| file.path != selection.keep_path.as_ref())
                        .count();
                    if delete_count == 0 {
                        return (groups, files, bytes);
                    }
                    (
                        groups + 1,
                        files + delete_count,
                        bytes.saturating_add(group.size.saturating_mul(delete_count as u64)),
                    )
                },
            );
            self.duplicates.selected_groups_cache = groups;
            self.duplicates.selected_files_cache = files;
            self.duplicates.selected_bytes_cache = bytes;
            self.duplicates.selection_totals_dirty = false;
        }
        (
            self.duplicates.selected_groups_cache,
            self.duplicates.selected_files_cache,
            self.duplicates.selected_bytes_cache,
        )
    }

    fn duplicate_target_from_path(&self, path: &str, size: u64) -> SelectedTarget {
        if let Some(store) = self.store.as_ref() {
            if let Some(node_id) = store.path_index.get(path).copied() {
                if let Some(target) = self.target_from_node_id(node_id) {
                    return target;
                }
            }
        }
        let name = PathBuf::from(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(Arc::<str>::from)
            .unwrap_or_else(|| Arc::<str>::from(path));
        SelectedTarget {
            node_id: None,
            name,
            path: Arc::<str>::from(path),
            size_bytes: size,
            kind: NodeKind::File,
            file_count: 1,
            dir_count: 0,
        }
    }

    fn queue_duplicate_delete_review(&mut self) {
        let mut targets = Vec::new();
        let mut group_count = 0usize;
        for group in &self.duplicates.groups {
            let selection = self.duplicate_group_selection(group);
            if !selection.enabled {
                continue;
            }
            let mut any_selected = false;
            for file in &group.files {
                if file.path == selection.keep_path.as_ref() {
                    continue;
                }
                any_selected = true;
                targets.push(self.duplicate_target_from_path(&file.path, file.size));
            }
            if any_selected {
                group_count += 1;
            }
        }
        if targets.is_empty() {
            return;
        }
        let estimated_bytes = targets.iter().map(|target| target.size_bytes).sum();
        self.duplicates.pending_delete = Some(DuplicateDeleteRequest {
            label: self
                .t("本次将处理的项目", "Items In This Cleanup")
                .to_string(),
            targets,
            estimated_bytes,
            group_count,
        });
    }

    fn handle_duplicate_delete_confirm_action(&mut self, action: DuplicateDeleteConfirmAction) {
        let Some(request) = self.duplicates.pending_delete.clone() else {
            return;
        };
        match action {
            DuplicateDeleteConfirmAction::RecycleBin => {
                self.queue_delete_request(
                    DeleteRequestScope {
                        label: request.label,
                        targets: request.targets,
                        selection_origin: SelectionOrigin::Duplicates,
                    },
                    ExecutionMode::RecycleBin,
                );
            }
            DuplicateDeleteConfirmAction::Permanent => {
                self.queue_delete_request(
                    DeleteRequestScope {
                        label: request.label,
                        targets: request.targets,
                        selection_origin: SelectionOrigin::Duplicates,
                    },
                    ExecutionMode::Permanent,
                );
            }
            DuplicateDeleteConfirmAction::Close => {}
        }
    }

    fn open_duplicate_file_location(&mut self, path: &str) {
        self.open_path_location(path);
    }

    fn open_path_location(&mut self, path: &str) {
        match dirotter_platform::select_in_explorer(path) {
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

    fn selection_origin(&self) -> SelectionOrigin {
        match self.selection.source {
            Some(SelectionSource::Table) => SelectionOrigin::TopFiles,
            Some(SelectionSource::Duplicate) => SelectionOrigin::Duplicates,
            Some(SelectionSource::Error) | None => SelectionOrigin::Manual,
        }
    }

    fn risk_for_path(&self, path: &str) -> RiskLevel {
        let category = Self::cleanup_category_for_path(path, NodeKind::File);
        let risk = Self::cleanup_risk_for_path(path, category);
        if risk == RiskLevel::Low && path.to_ascii_lowercase().ends_with(":\\") {
            RiskLevel::Medium
        } else {
            risk
        }
    }

    fn cleanup_category_for_path(path: &str, kind: NodeKind) -> CleanupCategory {
        cleanup::cleanup_category_for_path(path, kind)
    }

    fn cleanup_risk_for_path(path: &str, category: CleanupCategory) -> RiskLevel {
        cleanup::cleanup_risk_for_path(path, category)
    }

    fn start_scan_for_root(&mut self, root: String) {
        self.root_input = root;
        self.result_store_load_session = None;
        self.missing_result_store_root = None;
        self.page = Page::CurrentScan;
        self.start_scan();
    }

    fn delete_feedback_message(&self) -> Option<(String, String, bool)> {
        let report = self.execution_report.as_ref()?;
        let item = report.items.first()?;
        if report.succeeded > 0 && report.failed == 0 {
            return Some(match report.mode {
                ExecutionMode::RecycleBin => (
                    self.t("已移到回收站", "Moved to Recycle Bin").to_string(),
                    format!(
                        "{} {}",
                        format_count(report.succeeded as u64),
                        self.t(
                            "个项目已进入系统回收站，可从系统回收站恢复。",
                            "items were moved to the system recycle bin and can be restored there.",
                        )
                    ),
                    true,
                ),
                ExecutionMode::FastPurge => (
                    self.t("已快速移出", "Cleared from View").to_string(),
                    format!(
                        "{} {}",
                        format_count(report.succeeded as u64),
                        self.t(
                            "个项目已移入后台清理区，空间会在后台继续释放。",
                            "items were moved into the background cleanup area and disk space will continue to be reclaimed.",
                        )
                    ),
                    true,
                ),
                ExecutionMode::Permanent => (
                    self.t("已永久删除", "Deleted Permanently").to_string(),
                    format!(
                        "{} {}",
                        format_count(report.succeeded as u64),
                        self.t(
                            "个项目已永久删除，当前版本不提供撤销。",
                            "items were permanently deleted and cannot be undone in the current build.",
                        )
                    ),
                    true,
                ),
            });
        } else if report.succeeded > 0 && report.failed > 0 {
            return Some((
                self.t("部分项目已处理", "Cleanup Partially Completed")
                    .to_string(),
                format!(
                    "{} {} / {} {}",
                    format_count(report.succeeded as u64),
                    self.t("成功", "succeeded"),
                    format_count(report.failed as u64),
                    self.t("失败", "failed")
                ),
                false,
            ));
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
            format!(
                "{} {}",
                self.t(
                    "删除执行失败，请查看下方消息并重试。",
                    "Delete action failed. Review the message below and try again.",
                ),
                hint
            ),
            false,
        ))
    }

    #[cfg(test)]
    fn path_matches_target(path: &str, target: &SelectedTarget) -> bool {
        if path == target.path.as_ref() {
            return true;
        }
        if target.kind != NodeKind::Dir {
            return false;
        }
        let Some(rest) = path.strip_prefix(target.path.as_ref()) else {
            return false;
        };
        rest.starts_with('\\') || rest.starts_with('/')
    }

    #[cfg(test)]
    fn path_matches_any_target(path: &str, targets: &[SelectedTarget]) -> bool {
        targets
            .iter()
            .any(|target| Self::path_matches_target(path, target))
    }

    #[cfg(test)]
    fn rebuild_store_without_targets(
        store: &NodeStore,
        targets: &[SelectedTarget],
    ) -> Option<NodeStore> {
        let mut next = NodeStore::default();
        let mut id_map: HashMap<NodeId, NodeId> = HashMap::new();

        for node in &store.nodes {
            let node_path = store.node_path(node);
            if Self::path_matches_any_target(node_path, targets) {
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

        let top_files: Vec<dirotter_scan::RankedPath> = store
            .top_n_largest_files(32)
            .into_iter()
            .map(|node| (node.path.clone(), node.size_self))
            .collect();
        let top_dirs: Vec<dirotter_scan::RankedPath> = store
            .largest_dirs(32)
            .into_iter()
            .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
            .collect();

        self.live_top_files = top_files.clone();
        self.live_top_dirs = top_dirs.clone();
        self.completed_top_files = top_files;
        self.completed_top_dirs = top_dirs;
    }

    fn start_system_memory_release(&mut self) {
        if self.memory_release_session.is_some() {
            return;
        }

        self.trim_transient_runtime_memory();
        self.maintenance_feedback = None;
        self.last_system_memory_release = None;
        self.memory_release_session = Some(start_memory_release_session(self.egui_ctx.clone()));
    }

    fn process_memory_release_events(&mut self) {
        let Some(session) = self.memory_release_session.as_ref() else {
            return;
        };
        let Some(result) = take_finished_memory_release(session) else {
            return;
        };

        self.memory_release_session = None;
        self.refresh_memory_status();
        if matches!(self.page, Page::Diagnostics) {
            self.refresh_diagnostics();
        }
        match result {
            Ok(report) => {
                self.last_system_memory_release = Some(report);
                let freed_bytes = report.available_phys_delta();
                let mut message = format!(
                    "{} {}",
                    self.t(
                        "系统可用内存增加约",
                        "System free memory increased by about"
                    ),
                    format_bytes(freed_bytes)
                );
                message.push_str(&format!(
                    "  |  {} {}  |  {} {}",
                    self.t("已收缩进程", "Trimmed processes"),
                    report.trimmed_process_count,
                    self.t("扫描候选", "Scanned candidates"),
                    report.scanned_process_count
                ));
                if report.trimmed_system_file_cache {
                    message.push_str(&format!(
                        "  |  {}",
                        self.t("已裁剪系统文件缓存", "System file cache trimmed")
                    ));
                }
                self.set_maintenance_feedback(message, true);
            }
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t("系统内存释放失败", "System memory release failed"),
                    err.message
                ),
                false,
            ),
        }
    }

    fn execute_selected_delete(&mut self, mode: ExecutionMode) {
        let Some(target) = self.selected_target() else {
            return;
        };
        self.queue_delete_for_target(target, mode);
    }

    fn queue_delete_for_target(&mut self, target: SelectedTarget, mode: ExecutionMode) {
        let origin = self.selection_origin();
        self.queue_delete_request(Self::delete_request_for_target(target, origin), mode);
    }

    fn queue_delete_request(&mut self, request: DeleteRequestScope, mode: ExecutionMode) {
        self.pending_delete_confirmation = None;
        self.execution_report = None;
        self.queued_delete = Some(QueuedDeleteRequest { request, mode });
        self.egui_ctx.request_repaint();
    }

    fn process_queued_delete(&mut self) {
        if self.delete_active() {
            return;
        }
        let Some(request) = self.queued_delete.take() else {
            return;
        };
        self.execute_delete_request(request.request, request.mode);
    }

    fn execute_delete_request(&mut self, request: DeleteRequestScope, mode: ExecutionMode) {
        let plan = build_deletion_plan_with_origin(
            request
                .targets
                .iter()
                .map(|target| {
                    (
                        target.path.to_string(),
                        target.size_bytes,
                        self.risk_for_path(target.path.as_ref()),
                    )
                })
                .collect(),
            request.selection_origin,
        );
        self.pending_delete_confirmation = None;
        self.execution_report = None;
        self.explorer_feedback = None;
        self.status = AppStatus::Deleting;
        self.delete_session = Some(start_delete_session(
            self.egui_ctx.clone(),
            request,
            plan,
            mode,
        ));
        self.egui_ctx.request_repaint();
    }

    fn delete_active(&self) -> bool {
        self.delete_session.is_some() || self.delete_finalize_session.is_some()
    }

    fn system_memory_release_active(&self) -> bool {
        self.memory_release_session.is_some()
    }

    fn scan_finalizing(&self) -> bool {
        self.scan_finalize_session.is_some()
    }

    fn process_delete_events(&mut self) {
        let Some(session) = &self.delete_session else {
            return;
        };
        let session_snapshot = session.snapshot();

        let Some(payload) = take_finished_delete(session) else {
            return;
        };

        let report = payload.report;
        if report.succeeded > 0 {
            let mut succeeded_targets: Vec<_> = payload
                .request
                .targets
                .iter()
                .filter(|target| {
                    report
                        .items
                        .iter()
                        .any(|item| item.success && item.path == target.path.as_ref())
                })
                .cloned()
                .collect();
            succeeded_targets.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
            let finalize_store = if matches!(session_snapshot.mode, ExecutionMode::FastPurge) {
                self.store = None;
                None
            } else {
                self.store.take()
            };
            self.delete_session = None;
            self.delete_finalize_session = Some(start_delete_finalize_session(
                self.egui_ctx.clone(),
                DeleteFinalizeInput {
                    started_at: session_snapshot.started_at,
                    label: session_snapshot.label,
                    target_count: session_snapshot.target_count,
                    mode: session_snapshot.mode,
                    succeeded_count: report.succeeded,
                    failed_count: report.failed,
                    report,
                    succeeded_targets,
                    summary: self.summary.clone(),
                    store: finalize_store,
                    cleanup_analysis: self.cleanup.analysis.clone(),
                    live_files: self.live_files.clone(),
                    live_top_files: self.live_top_files.clone(),
                    live_top_dirs: self.live_top_dirs.clone(),
                    completed_top_files: self.completed_top_files.clone(),
                    completed_top_dirs: self.completed_top_dirs.clone(),
                    errors: self.errors.clone(),
                },
            ));
            self.egui_ctx.request_repaint();
            return;
        } else {
            self.status = AppStatus::DeleteFailed;
        }
        self.set_execution_failure_details_open(false);
        self.execution_report = Some(report);
        self.delete_session = None;
        self.refresh_diagnostics();
    }

    fn process_delete_finalize_events(&mut self) {
        let Some(session) = &self.delete_finalize_session else {
            return;
        };

        let Some(mut payload) = take_finished_delete_finalize(session) else {
            return;
        };

        payload.summary.error_count = payload.errors.len() as u64;
        self.live_files = payload.live_files;
        self.live_top_files = payload.live_top_files;
        self.live_top_dirs = payload.live_top_dirs;
        self.completed_top_files = payload.completed_top_files;
        self.completed_top_dirs = payload.completed_top_dirs;
        self.errors = payload.errors;
        self.store = payload.store;
        self.result_store_load_session = None;
        self.missing_result_store_root = None;
        self.summary = payload.summary;
        self.apply_cleanup_analysis(payload.cleanup_analysis);
        self.reset_duplicate_review();
        self.selection = SelectionState::default();
        self.status = if payload.report.succeeded > 0 {
            AppStatus::DeleteExecuted
        } else {
            AppStatus::DeleteFailed
        };
        self.set_execution_failure_details_open(false);
        self.execution_report = Some(payload.report);
        self.delete_finalize_session = None;
        self.refresh_diagnostics();
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

    fn select_node(&mut self, node_id: NodeId, source: SelectionSource) {
        self.selection.selected_node = Some(node_id);
        self.selection.selected_path = self
            .store
            .as_ref()
            .and_then(|store| store.nodes.get(node_id.0))
            .map(|node| node.path.to_string());
        self.selection.source = Some(source);
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
        self.scan_session.is_some() || self.scan_finalizing()
    }

    fn finish_cancelled_scan(&mut self) {
        self.status = AppStatus::Cancelled;
        self.scan_current_path = None;
        self.scan_last_event_at = None;
        self.scan_cancel_requested = false;
        self.pending_batch_events.clear();
        self.pending_snapshots.clear();
        self.live_files.clear();
        self.completed_top_files = self.live_top_files.clone();
        self.completed_top_dirs = self.live_top_dirs.clone();
        self.execution_report = None;
        self.cleanup = CleanupPanelState::default();
        self.reset_duplicate_review();
        self.reset_duplicate_prep();
        self.refresh_diagnostics();
        self.scan_session = None;
        self.scan_finalize_session = None;
    }

    fn process_scan_finalize_events(&mut self) {
        let Some(session) = &self.scan_finalize_session else {
            return;
        };

        let finished = {
            let mut relay = session.relay.lock().expect("scan finalize relay lock");
            relay.finished.take()
        };

        let Some(payload) = finished else {
            return;
        };

        self.store = Some(payload.store);
        self.result_store_load_session = None;
        self.missing_result_store_root = None;
        self.summary = payload.summary;
        self.errors = payload.errors;
        self.sync_rankings_from_store();
        self.apply_cleanup_analysis(Some(payload.cleanup_analysis));
        let released_result_store = self.release_result_store_to_snapshot();
        self.reset_duplicate_review();
        self.status = AppStatus::Completed;
        self.scan_current_path = None;
        self.scan_last_event_at = None;
        self.scan_cancel_requested = false;
        self.execution_report = None;
        self.scan_finalize_session = None;
        if released_result_store {
            let _ = dirotter_platform::trim_process_memory();
            self.refresh_memory_status();
        }
        self.refresh_diagnostics();
    }

    fn refresh_diagnostics(&mut self) {
        self.refresh_memory_status();
        let telemetry_snapshot = telemetry::snapshot();
        let system_snapshot = telemetry::system_snapshot();
        let metrics = telemetry::metric_descriptors();
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

        self.diagnostics_json = serde_json::to_string_pretty(&serde_json::json!({
            "bundle_structure_version": 2,
            "settings_path": self.cache.settings_path().display().to_string(),
            "ephemeral_settings_fallback": self.cache.uses_ephemeral_settings(),
            "session_snapshot_root": self.cache.session_root().display().to_string(),
            "telemetry_snapshot": telemetry_snapshot,
            "system_snapshot": system_snapshot,
            "process_memory": self.process_memory,
            "system_memory": self.system_memory,
            "last_system_memory_release": self.last_system_memory_release,
            "result_store_resident": self.store.is_some(),
            "metrics": metrics,
            "current_errors": self.errors.len(),
            "path_access": path_access,
        }))
        .unwrap_or_else(|_| "{}".to_string());
    }

    fn set_maintenance_feedback(&mut self, message: String, success: bool) {
        self.maintenance_feedback = Some((message, success));
    }

    fn refresh_memory_status(&mut self) {
        self.process_memory = dirotter_platform::process_memory_stats().ok();
        self.system_memory = dirotter_platform::system_memory_stats().ok();
        self.last_memory_status_refresh = Some(Instant::now());
    }

    fn maybe_refresh_memory_status(&mut self) {
        let stale = self
            .last_memory_status_refresh
            .is_none_or(|at| at.elapsed() >= Duration::from_millis(MEMORY_STATUS_REFRESH_MS));
        if stale {
            self.refresh_memory_status();
        }
    }

    fn system_memory_pressure_active(&self) -> bool {
        self.system_memory.as_ref().is_some_and(|memory| {
            memory.low_memory_signal == Some(true)
                || memory.memory_load_percent >= HIGH_MEMORY_LOAD_PERCENT
        })
    }

    fn trim_transient_runtime_memory(&mut self) {
        self.pending_batch_events.clear();
        self.pending_batch_events.shrink_to_fit();
        self.pending_snapshots.clear();
        self.pending_snapshots.shrink_to_fit();
        self.live_files.clear();
        self.live_files.shrink_to_fit();
        self.live_top_files.clear();
        self.live_top_files.shrink_to_fit();
        self.live_top_dirs.clear();
        self.live_top_dirs.shrink_to_fit();
        self.diagnostics_json.clear();
        self.diagnostics_json.shrink_to_fit();
        self.errors.clear();
        self.errors.shrink_to_fit();
    }

    fn save_snapshot_before_memory_release(&mut self) -> bool {
        let Some(store) = self.store.as_ref() else {
            return false;
        };
        self.cache.save_snapshot(&self.root_input, store).is_ok()
    }

    fn release_result_store_to_snapshot(&mut self) -> bool {
        if self.store.is_none()
            || self.result_store_load_session.is_some()
            || self.duplicate_scan_session.is_some()
        {
            return false;
        }
        if !self.save_snapshot_before_memory_release() {
            return false;
        }
        self.store = None;
        self.result_store_load_session = None;
        self.missing_result_store_root = None;
        self.selection = SelectionState::default();
        self.reset_duplicate_review();
        self.reset_duplicate_prep();
        true
    }

    #[cfg(test)]
    fn ensure_store_loaded_from_cache(&mut self) -> bool {
        if self.store.is_some() {
            return true;
        }
        match self.cache.load_latest_snapshot(&self.root_input) {
            Ok(Some(snapshot)) => {
                self.store = Some(snapshot);
                self.sync_summary_from_store();
                self.sync_rankings_from_store();
                if self.cleanup.analysis.is_none() {
                    self.refresh_cleanup_analysis();
                }
                self.reset_duplicate_review();
                self.reset_duplicate_prep();
                self.refresh_memory_status();
                true
            }
            _ => false,
        }
    }

    fn can_reload_result_store_from_cache(&self) -> bool {
        self.store.is_none()
            && (self.summary.bytes_observed > 0
                || !self.completed_top_files.is_empty()
                || !self.completed_top_dirs.is_empty())
    }

    fn result_store_is_in_active_use(&self) -> bool {
        self.result_store_load_session.is_some()
            || self.duplicate_scan_session.is_some()
            || matches!(self.page, Page::Duplicates)
    }

    fn result_store_load_active(&self) -> bool {
        self.result_store_load_session.is_some()
    }

    fn begin_result_store_load_if_needed(&mut self) {
        if self.store.is_some()
            || self.result_store_load_session.is_some()
            || self.scan_active()
            || self.delete_active()
            || !self.can_reload_result_store_from_cache()
            || self.missing_result_store_root.as_deref() == Some(self.root_input.as_str())
        {
            return;
        }

        self.result_store_load_session = Some(start_result_store_load_session(
            self.egui_ctx.clone(),
            self.cache.session_root().to_path_buf(),
            self.root_input.clone(),
        ));
        self.egui_ctx.request_repaint();
    }

    fn process_result_store_load_events(&mut self) {
        let Some(session) = &self.result_store_load_session else {
            return;
        };

        let Some(payload) = take_finished_result_store_load(session) else {
            return;
        };

        self.result_store_load_session = None;
        let keep_loaded_store = matches!(self.page, Page::Duplicates);
        if let Some(store) = payload.store {
            if keep_loaded_store {
                self.store = Some(store);
            } else {
                self.store = None;
            }
            if let Some(summary) = payload.summary {
                self.summary.scanned_files = summary.scanned_files;
                self.summary.scanned_dirs = summary.scanned_dirs;
                self.summary.bytes_observed = summary.bytes_observed;
            }
            self.live_top_files = payload.top_files.clone();
            self.completed_top_files = payload.top_files;
            self.live_top_dirs = payload.top_dirs.clone();
            self.completed_top_dirs = payload.top_dirs;
            self.apply_cleanup_analysis(payload.cleanup_analysis);
            self.reset_duplicate_review();
            self.reset_duplicate_prep();
            self.refresh_memory_status();
            self.missing_result_store_root = None;
        } else {
            self.missing_result_store_root = Some(payload.root);
        }
    }

    fn maybe_auto_release_memory(&mut self) {
        if self.scan_active() || self.delete_active() || self.result_store_is_in_active_use() {
            return;
        }
        if self.last_user_activity.elapsed() < Duration::from_secs(IDLE_MEMORY_RELEASE_SECS) {
            return;
        }
        if self
            .last_auto_memory_release_at
            .is_some_and(|at| at.elapsed() < Duration::from_secs(AUTO_MEMORY_RELEASE_COOLDOWN_SECS))
        {
            return;
        }
        if !self.system_memory_pressure_active() {
            return;
        }

        self.trim_transient_runtime_memory();
        if matches!(self.status, AppStatus::Completed) {
            let _ = self.release_result_store_to_snapshot();
        }
        let _ = dirotter_platform::trim_process_memory();
        self.refresh_memory_status();
        if matches!(self.page, Page::Diagnostics) {
            self.refresh_diagnostics();
        }
        self.last_auto_memory_release_at = Some(Instant::now());
    }

    fn recommended_boost_action(&self) -> BoostAction {
        if self
            .cleanup
            .analysis
            .as_ref()
            .is_some_and(|analysis| analysis.quick_clean_bytes > 0)
        {
            BoostAction::QuickCacheCleanup
        } else if self.summary.bytes_observed == 0 {
            BoostAction::StartScan
        } else if self
            .cleanup
            .analysis
            .as_ref()
            .is_some_and(|analysis| analysis.reclaimable_bytes > 0)
        {
            BoostAction::ReviewSuggestions
        } else {
            BoostAction::NoImmediateAction
        }
    }

    fn execute_recommended_boost(&mut self) {
        match self.recommended_boost_action() {
            BoostAction::StartScan => self.start_scan(),
            BoostAction::QuickCacheCleanup => self.queue_cleanup_cache_delete(),
            BoostAction::ReviewSuggestions => {
                let default_category = self
                    .cleanup
                    .analysis
                    .as_ref()
                    .and_then(|analysis| analysis.categories.first().map(|entry| entry.category));
                self.cleanup.detail_category = default_category;
            }
            BoostAction::NoImmediateAction => self.set_maintenance_feedback(
                self.t(
                    "当前没有明确的一键提速动作，先从下方最大的文件夹和文件开始。",
                    "There is no obvious one-tap boost action right now. Start from the largest folders and files below.",
                )
                .to_string(),
                true,
            ),
        }
    }

    fn release_dir_otter_memory(&mut self) {
        let before_process = dirotter_platform::process_memory_stats().ok();
        self.trim_transient_runtime_memory();
        self.completed_top_files.clear();
        self.completed_top_files.shrink_to_fit();
        self.completed_top_dirs.clear();
        self.completed_top_dirs.shrink_to_fit();
        self.cleanup = CleanupPanelState::default();
        self.execution_report = None;
        self.explorer_feedback = None;
        self.selection = SelectionState::default();
        self.errors.clear();
        self.errors.shrink_to_fit();
        let snapshot_saved = self.save_snapshot_before_memory_release();
        self.store = None;
        self.summary = ScanSummary::default();
        self.scan_current_path = None;
        self.scan_last_event_at = None;
        self.status = AppStatus::Idle;
        self.reset_duplicate_review();
        self.reset_duplicate_prep();
        let trimmed = dirotter_platform::trim_process_memory();
        self.refresh_memory_status();
        self.refresh_diagnostics();
        let reclaimed = before_process
            .zip(self.process_memory)
            .map(|(before, after)| {
                before
                    .working_set_bytes
                    .saturating_sub(after.working_set_bytes)
            });
        match trimmed {
            Ok(()) => {
                let mut message = self
                    .t(
                        "已清空当前结果并优化 DirOtter 内存占用。",
                        "Cleared the current result and optimized DirOtter memory usage.",
                    )
                    .to_string();
                if let Some(bytes) = reclaimed {
                    message.push(' ');
                    message.push_str(
                        format!(
                            "{} {}。",
                            self.t("工作集回收约", "Working set reclaimed about"),
                            format_bytes(bytes)
                        )
                        .as_str(),
                    );
                }
                if snapshot_saved {
                    message.push(' ');
                    message.push_str(self.t(
                        "已先写入当前会话的临时快照，可在需要时重新载入结果。",
                        "A disk snapshot was saved first, so the result can be reloaded later.",
                    ));
                }
                self.set_maintenance_feedback(message, true);
            }
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t(
                        "已清空当前结果，但 Windows 工作集收缩失败",
                        "Cleared current results, but Windows working-set trimming failed"
                    ),
                    err.message
                ),
                false,
            ),
        }
    }

    fn purge_staging_manually(&mut self) {
        match dirotter_platform::purge_all_staging_roots() {
            Ok(()) => self.set_maintenance_feedback(
                self.t(
                    "已清理异常中断后残留的临时删除区内容。",
                    "Cleaned leftover items from the interrupted cleanup area.",
                )
                .to_string(),
                true,
            ),
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t(
                        "清理异常中断的临时删除区失败",
                        "Failed to clean the interrupted cleanup area",
                    ),
                    err.message
                ),
                false,
            ),
        }
    }

    fn start_scan(&mut self) {
        self.page = Page::CurrentScan;
        self.status = AppStatus::Scanning;
        self.scan_current_path = None;
        self.scan_last_event_at = Some(Instant::now());
        self.scan_cancel_requested = false;
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
        self.result_store_load_session = None;
        self.missing_result_store_root = None;
        self.cleanup = CleanupPanelState::default();
        self.reset_duplicate_review();
        self.reset_duplicate_prep();
        self.delete_session = None;
        self.delete_finalize_session = None;
        self.queued_delete = None;
        self.pending_delete_confirmation = None;
        self.execution_report = None;
        self.last_coalesce_commit = Instant::now();
        let scan_config = self.selected_scan_config();

        let handle = start_scan(PathBuf::from(self.root_input.clone()), scan_config);
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
                        store,
                        errors,
                    } => {
                        state.finished = Some(FinishedPayload {
                            summary,
                            store,
                            errors,
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
                self.scan_current_path = progress.current_path;
                self.summary = progress.summary;
                self.perf.snapshot_queue_depth = progress
                    .queue_depth
                    .max(progress.metadata_backlog)
                    .max(progress.publisher_lag);
            }

            for batch in batches {
                self.absorb_duplicate_prep_batch(&batch);
                self.pending_batch_events.push_back(batch);
                if self.pending_batch_events.len() > MAX_PENDING_BATCH_EVENTS {
                    let drop_n = self.pending_batch_events.len() - MAX_PENDING_BATCH_EVENTS;
                    self.pending_batch_events.drain(0..drop_n);
                    telemetry::record_ui_backpressure(drop_n as u64, 0);
                }
            }

            if let Some((delta, view)) = snapshot {
                let (top_files, top_dirs) = view.into_rankings();
                self.live_top_files = top_files;
                self.live_top_dirs = top_dirs;
                self.pending_snapshots.push_back(delta);
                // Keep live scan snapshots lightweight on the UI thread.
                // The full result store is delivered once the scan finishes.
                if self.pending_snapshots.len() > MAX_PENDING_SNAPSHOTS {
                    let drop_n = self.pending_snapshots.len() - MAX_PENDING_SNAPSHOTS;
                    self.pending_snapshots.drain(0..drop_n);
                    telemetry::record_ui_backpressure(0, drop_n as u64);
                }
            }

            finished = relay_finished;
        }

        // Coalesce UI updates according to the active scan mode cadence.
        if self.last_coalesce_commit.elapsed()
            >= Duration::from_millis(self.selected_scan_config().effective_snapshot_ms())
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
            if self.scan_cancel_requested {
                self.errors = finished.errors;
                self.finish_cancelled_scan();
                return;
            }

            self.status = AppStatus::Finalizing;
            self.summary = finished.summary.clone();
            self.scan_current_path = Some(Arc::<str>::from(
                self.t("正在整理最终结果…", "Finalizing final results...")
                    .to_string(),
            ));
            self.scan_last_event_at = Some(Instant::now());
            self.completed_top_files = self.live_top_files.clone();
            self.completed_top_dirs = self.live_top_dirs.clone();
            self.execution_report = None;
            self.scan_session = None;
            let relay = Arc::new(Mutex::new(ScanFinalizeRelayState::default()));
            let relay_state = Arc::clone(&relay);
            let ctx = self.egui_ctx.clone();
            std::thread::spawn(move || {
                let cleanup_analysis = Self::build_cleanup_analysis(&finished.store);
                let mut state = relay_state.lock().expect("scan finalize relay lock");
                state.finished = Some(ScanFinalizePayload {
                    summary: finished.summary,
                    store: finished.store,
                    errors: finished.errors,
                    cleanup_analysis,
                });
                drop(state);
                ctx.request_repaint();
            });
            self.scan_finalize_session = Some(ScanFinalizeSession { relay });
            self.refresh_diagnostics();
        }

        let t = telemetry::snapshot();
        self.perf.avg_snapshot_commit_ms = t.avg_snapshot_commit_ms;
        self.perf.avg_scan_batch_size = t.avg_scan_batch_size;
        self.perf.frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.perf.last_update = Some(Instant::now());
        telemetry::record_ui_frame();
    }

    #[allow(dead_code)]
    fn ui_nav(&mut self, ui: &mut egui::Ui) {
        ui.add_space(24.0);

        // 品牌区域 - 更清晰的层次
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.heading("DirOtter");
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(self.t(
                    "冷静地理解目录树，而不是急着清理一切。",
                    "A calmer way to understand your file tree.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().text_color()),
            );
        });
        ui.add_space(32.0);

        // 导航标题 - 使用品牌色和更好的间距
        ui.label(
            egui::RichText::new(self.t("导航", "Navigation"))
                .text_style(egui::TextStyle::Small)
                .color(river_teal())
                .strong(),
        );
        ui.add_space(16.0);

        // 主导航项 - 更大的点击区域和更好的视觉反馈
        for (p, label_zh, label_en) in [
            (Page::Dashboard, "概览", "Overview"),
            (Page::CurrentScan, "扫描进行中", "Live Scan"),
            (Page::Duplicates, "重复文件", "Duplicate Files"),
            (Page::Settings, "偏好设置", "Settings"),
        ] {
            let selected = self.page == p;
            let text = egui::RichText::new(self.t(label_zh, label_en))
                .size(15.0)
                .color(ui.visuals().text_color());
            let response = ui.add_sized(
                [ui.available_width(), NAV_ITEM_HEIGHT],
                egui::SelectableLabel::new(selected, text),
            );
            if response.clicked() {
                self.page = p;
            }
        }

        // 高级工具区域
        if self.advanced_tools_enabled {
            ui.add_space(24.0);
            ui.separator();
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(self.t("高级工具", "Advanced Tools"))
                    .text_style(egui::TextStyle::Small)
                    .color(river_teal())
                    .strong(),
            );
            ui.add_space(12.0);
            for (p, label_zh, label_en) in [
                (Page::Errors, "错误中心", "Errors"),
                (Page::Diagnostics, "诊断信息", "Diagnostics"),
            ] {
                let selected = self.page == p;
                let text = egui::RichText::new(self.t(label_zh, label_en))
                    .size(14.0)
                    .color(if selected {
                        ui.visuals().selection.bg_fill
                    } else {
                        ui.visuals().text_color()
                    });
                let response = ui.add_sized(
                    [ui.available_width(), NAV_ITEM_HEIGHT],
                    egui::SelectableLabel::new(selected, text),
                );
                if response.clicked() {
                    self.page = p;
                }
            }
        }
    }

    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        dashboard::ui_dashboard(self, ui);
    }

    fn ui_current_scan(&mut self, ui: &mut egui::Ui) {
        result_pages::ui_current_scan(self, ui);
    }

    fn ui_duplicates(&mut self, ui: &mut egui::Ui) {
        duplicates_pages::ui_duplicates(self, ui);
    }

    fn ui_errors(&mut self, ui: &mut egui::Ui) {
        advanced_pages::ui_errors(self, ui);
    }

    fn ui_diagnostics(&mut self, ui: &mut egui::Ui) {
        settings_pages::ui_diagnostics(self, ui);
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        settings_pages::ui_settings(self, ui, ctx);
    }

    #[allow(dead_code)]
    fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("DirOtter")
                    .size(22.0)
                    .strong()
                    .color(ui.visuals().text_color()),
            );
            ui.add_space(10.0);
            let scanning = self.scan_session.is_some();
            let finalizing = self.scan_finalizing();
            status_badge(
                ui,
                self.status_text(),
                scanning || finalizing || self.delete_active(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let active = scanning;
                let deleting = self.delete_active();
                let stop_label = if self.scan_cancel_requested {
                    self.t("正在停止", "Stopping")
                } else if active {
                    self.t("停止扫描", "Stop Scan")
                } else if finalizing {
                    self.t("整理中", "Finalizing")
                } else {
                    self.t("取消", "Cancel")
                };
                if ui
                    .add_enabled_ui(active && !self.scan_cancel_requested, |ui| {
                        sized_button(ui, 108.0, stop_label)
                    })
                    .inner
                    .clicked()
                {
                    if let Some(session) = &self.scan_session {
                        session.cancel.store(true, Ordering::SeqCst);
                        self.scan_cancel_requested = true;
                        self.status = AppStatus::Cancelled;
                        self.scan_current_path = None;
                    }
                }
                let start_label = if active {
                    self.t("扫描中", "Scanning")
                } else if finalizing {
                    self.t("整理中", "Finalizing")
                } else if deleting {
                    self.t("删除中", "Deleting")
                } else {
                    self.t("开始扫描", "Start Scan")
                };
                if ui
                    .add_enabled_ui(!active && !finalizing && !deleting, |ui| {
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

    #[allow(dead_code)]
    fn ui_inspector(&mut self, ui: &mut egui::Ui) {
        let selected_target = self.selected_target();
        let selected_target_view = selected_target
            .as_ref()
            .map(|target| self.inspector_target_view_model(target));
        let delete_task_view = self.delete_task_view_model();
        let inspector_actions_view = self.inspector_actions_view_model(selected_target.as_ref());
        let explorer_feedback_view = self.inspector_explorer_feedback_view_model();
        let delete_feedback_view = self.inspector_delete_feedback_view_model();
        let execution_report_view = self.inspector_execution_report_view_model();
        let memory_status_view = self.inspector_memory_status_view_model();
        let maintenance_feedback_view = self.inspector_maintenance_feedback_view_model();
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
        egui::ScrollArea::vertical()
            .id_source("inspector-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                surface_panel(ui, |ui| {
                    if let Some(target) = selected_target_view.as_ref() {
                        stat_row(
                            ui,
                            self.t("名称", "Name"),
                            target.name_value.as_ref(),
                            target.name_hint,
                        );
                        stat_row(
                            ui,
                            self.t("路径", "Path"),
                            &target.path_value,
                            target.path_hint,
                        );
                        stat_row(
                            ui,
                            self.t("大小", "Size"),
                            &target.size_value,
                            &target.size_hint,
                        );
                    } else {
                        ui.label(self.t(
                            "尚未选择任何文件或目录。可以从实时列表、重复文件或错误中心点选对象。",
                            "No file or folder is selected yet. Pick one from the live list, duplicate review, or errors.",
                        ));
                    }
                });

                if let Some(snapshot) = delete_task_view.as_ref() {
                    ui.add_space(10.0);
                    surface_panel(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(snapshot.title)
                                    .text_style(egui::TextStyle::Name("title".into())),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add(egui::Spinner::new().size(18.0));
                            });
                        });
                        ui.label(
                            egui::RichText::new(snapshot.description)
                                .text_style(egui::TextStyle::Small)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(8.0);
                        stat_row(
                            ui,
                            self.t("目标", "Target"),
                            &snapshot.target_value,
                            &snapshot.target_hint,
                        );
                        stat_row(
                            ui,
                            &snapshot.progress_title,
                            &snapshot.progress_value,
                            &snapshot.progress_hint,
                        );
                        stat_row(
                            ui,
                            self.t("已耗时", "Elapsed"),
                            &snapshot.elapsed_value,
                            snapshot.elapsed_hint,
                        );
                        if let (Some(current_title), Some(current_target)) = (
                            snapshot.current_target_title.as_ref(),
                            snapshot.current_target_value.as_ref(),
                        ) {
                            stat_row(
                                ui,
                                current_title,
                                current_target,
                                snapshot.current_target_hint.unwrap_or(""),
                            );
                        }
                    });
                }

                ui.add_space(10.0);
                surface_panel(ui, |ui| {
                    ui.label(
                        egui::RichText::new(self.t("快速操作", "Quick Actions"))
                            .text_style(egui::TextStyle::Name("title".into())),
                    );
                    ui.label(
                        egui::RichText::new(&inspector_actions_view.section_description)
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        if ui
                            .add_enabled_ui(inspector_actions_view.can_open_location, |ui| {
                                sized_button(
                                    ui,
                                    ui.available_width(),
                                    &inspector_actions_view.open_location_label,
                                )
                            })
                            .inner
                            .clicked()
                        {
                            if let Some(target) = selected_target.as_ref() {
                                self.open_path_location(target.path.as_ref());
                            }
                        }
                        if inspector_actions_view.show_fast_cleanup
                            && ui
                                .add_enabled_ui(inspector_actions_view.can_fast_cleanup, |ui| {
                                    sized_primary_button(
                                        ui,
                                        ui.available_width(),
                                        &inspector_actions_view.fast_cleanup_label,
                                    )
                                })
                                .inner
                                .clicked()
                        {
                            if let Some(target) = selected_target.clone() {
                                self.queue_delete_for_target(target, ExecutionMode::FastPurge);
                            }
                        }
                        if ui
                            .add_enabled_ui(inspector_actions_view.can_recycle, |ui| {
                                sized_button(
                                    ui,
                                    ui.available_width(),
                                    &inspector_actions_view.recycle_label,
                                )
                            })
                            .inner
                            .clicked()
                        {
                            self.execute_selected_delete(ExecutionMode::RecycleBin);
                        }
                        let permanent = egui::Button::new(&inspector_actions_view.permanent_label)
                            .fill(danger_red());
                        if ui
                            .add_enabled_ui(inspector_actions_view.can_permanent_delete, |ui| {
                                ui.add_sized([ui.available_width(), CONTROL_HEIGHT], permanent)
                            })
                            .inner
                            .clicked()
                        {
                            if let Some(target) = selected_target.clone() {
                                self.pending_delete_confirmation = Some(PendingDeleteConfirmation {
                                    request: Self::delete_request_for_target(
                                        target.clone(),
                                        self.selection_origin(),
                                    ),
                                    risk: self.risk_for_path(target.path.as_ref()),
                                });
                            }
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);
                        if ui
                            .add_enabled_ui(inspector_actions_view.can_release_memory, |ui| {
                                sized_primary_button(
                                    ui,
                                    ui.available_width(),
                                    &inspector_actions_view.release_memory_label,
                                )
                            })
                            .inner
                            .on_hover_text(&inspector_actions_view.release_memory_tooltip)
                            .clicked()
                        {
                            self.start_system_memory_release();
                        }
                    });
                    if let Some(message) = inspector_actions_view.info_message.as_ref() {
                        ui.label(
                            egui::RichText::new(message)
                                .text_style(egui::TextStyle::Small)
                                .color(ui.visuals().weak_text_color()),
                        );
                    }
                    if let Some(feedback) = explorer_feedback_view.as_ref() {
                        ui.add_space(8.0);
                        tone_banner(ui, &feedback.title, &feedback.message);
                    }
                    if let Some((feedback, success)) = delete_feedback_view.as_ref() {
                        ui.add_space(10.0);
                        tone_banner(ui, &feedback.title, &feedback.message);
                        if !success {
                            ui.add_space(6.0);
                        }
                    }
                    if let Some(report) = execution_report_view.as_ref() {
                        ui.add_space(10.0);
                        stat_row(
                            ui,
                            &report.title,
                            &report.summary_value,
                            &report.summary_hint,
                        );
                        if let Some(label) = report.failure_detail_label.as_ref() {
                            ui.add_space(8.0);
                            let response = ui.add_sized(
                                [ui.available_width(), CONTROL_HEIGHT],
                                egui::Button::new(label),
                            );
                            let response = if let Some(hint) = report.failure_detail_hint.as_ref()
                            {
                                response.on_hover_text(hint)
                            } else {
                                response
                            };
                            if response.clicked() {
                                self.set_execution_failure_details_open(true);
                            }
                        }
                    }
                });

                ui.add_space(10.0);
                surface_panel(ui, |ui| {
                    ui.label(
                        egui::RichText::new(self.t("一键释放系统内存", "Release System Memory"))
                            .text_style(egui::TextStyle::Name("title".into())),
                    );
                    ui.add_space(10.0);
                    if let Some(system_free) = memory_status_view.system_free_value.as_ref() {
                        ui.label(egui::RichText::new(system_free).size(28.0).strong());
                        ui.add_space(8.0);
                    }

                    if let Some(load_value) = memory_status_view.load_value.as_ref() {
                        stat_row(
                            ui,
                            self.t("内存负载", "load"),
                            load_value,
                            "",
                        );
                        ui.add_space(6.0);
                    }
                    if let Some(process_memory) =
                        memory_status_view.process_working_set_value.as_ref()
                    {
                        stat_row(ui, "DirOtter", process_memory, "");
                    }

                    if let Some(active_message) = memory_status_view.active_message.as_ref() {
                        ui.add_space(10.0);
                        tone_banner(
                            ui,
                            self.t("一键释放系统内存", "Release System Memory"),
                            active_message,
                        );
                    }

                    if let Some(delta) = memory_status_view.release_delta_value.as_ref() {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(self.t("最近执行", "Last Action"))
                                .text_style(egui::TextStyle::Small)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(6.0);
                        stacked_stat_block(
                            ui,
                            self.t(
                                "系统可用内存增加约",
                                "System free memory increased by about",
                            ),
                            delta,
                            memory_status_view
                                .release_delta_hint
                                .as_deref()
                                .unwrap_or(""),
                        );
                    }
                    if let Some((feedback, success)) = maintenance_feedback_view.as_ref() {
                        if !success {
                            ui.add_space(10.0);
                            tone_banner(ui, &feedback.title, &feedback.message);
                        }
                    }
                });
                ui.add_space(20.0);
            });
    }

    #[allow(dead_code)]
    fn ui_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_delete_confirmation.clone() else {
            return;
        };
        let Some(view_model) = self.delete_confirmation_view_model(&pending) else {
            self.pending_delete_confirmation = None;
            return;
        };

        let mut keep_open = true;
        let mut actions = Vec::new();
        egui::Window::new(self.t("确认永久删除", "Confirm Permanent Delete"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(egui::RichText::new(view_model.intro).strong());
                ui.add_space(8.0);
                stat_row(
                    ui,
                    self.t("目标", "Target"),
                    &view_model.target_value,
                    view_model.target_hint,
                );
                stat_row(
                    ui,
                    self.t("大小", "Size"),
                    &view_model.size_value,
                    &view_model.size_hint,
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(view_model.recommendation)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let confirm = egui::Button::new(self.t("确认永久删除", "Delete Permanently"))
                        .fill(danger_red());
                    if ui.add(confirm).clicked() {
                        actions.push(DeleteConfirmAction::Confirm);
                    }
                    if ui.button(self.t("取消", "Cancel")).clicked() {
                        actions.push(DeleteConfirmAction::Close);
                    }
                });
            });

        for action in actions {
            if matches!(
                action,
                DeleteConfirmAction::Close | DeleteConfirmAction::Confirm
            ) {
                keep_open = false;
            }
            self.handle_delete_confirm_action(pending.request.clone(), action);
        }
        if !keep_open {
            self.pending_delete_confirmation = None;
        }
    }

    #[allow(dead_code)]
    fn ui_cleanup_details_window(&mut self, ctx: &egui::Context) {
        let Some(category) = self.cleanup.detail_category else {
            return;
        };
        let items = self.cleanup_items_for_category(category);
        let view_model = self.cleanup_details_window_view_model(category, &items);
        let mut keep_open = true;
        let mut actions = Vec::new();
        let screen_size = ctx.input(|i| i.screen_rect().size());
        egui::Window::new(self.t("清理建议详情", "Cleanup Details"))
            .open(&mut keep_open)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_size(egui::vec2(780.0, 560.0))
            .max_size(egui::vec2(
                (screen_size.x - 48.0).max(760.0),
                (screen_size.y - 48.0).max(520.0),
            ))
            .show(ctx, |ui| {
                ui.set_min_size(egui::vec2(720.0, 480.0));
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&view_model.review_message)
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&view_model.close_label).clicked() {
                            actions.push(CleanupDetailsAction::Close);
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for tab in &view_model.category_tabs {
                        if sized_selectable(ui, 150.0, tab.selected, &tab.label).clicked() {
                            actions.push(CleanupDetailsAction::SelectCategory(tab.category));
                        }
                    }
                });
                ui.add_space(10.0);
                tone_banner(ui, &view_model.banner_title, &view_model.banner_message);
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    compact_stat_chip(
                        ui,
                        self.t("已选项目", "Selected"),
                        &view_model.selected_count_value,
                    );
                    compact_stat_chip(
                        ui,
                        self.t("预计释放", "Estimated Reclaim"),
                        &view_model.selected_bytes_value,
                    );
                    if ui
                        .add_enabled_ui(view_model.select_safe_enabled, |ui| {
                            sized_button(ui, 124.0, &view_model.select_safe_label)
                        })
                        .inner
                        .clicked()
                    {
                        actions.push(CleanupDetailsAction::SelectAllSafe);
                    }
                    if ui
                        .add_enabled_ui(view_model.clear_selected_enabled, |ui| {
                            sized_button(ui, 118.0, &view_model.clear_selected_label)
                        })
                        .inner
                        .clicked()
                    {
                        actions.push(CleanupDetailsAction::ClearSelected);
                    }
                    if ui
                        .add_enabled_ui(view_model.open_selected_enabled, |ui| {
                            sized_button(ui, 124.0, &view_model.open_selected_label)
                        })
                        .inner
                        .clicked()
                    {
                        actions.push(CleanupDetailsAction::OpenSelectedLocation);
                    }
                    if ui
                        .add_enabled_ui(view_model.header_primary_enabled, |ui| {
                            sized_button(ui, 176.0, &view_model.header_primary_label)
                        })
                        .inner
                        .clicked()
                    {
                        actions.push(CleanupDetailsAction::TriggerPrimary);
                    }
                    if ui
                        .add_enabled_ui(view_model.permanent_enabled, |ui| {
                            let button =
                                egui::Button::new(&view_model.permanent_label).fill(danger_red());
                            ui.add_sized([156.0, CONTROL_HEIGHT], button)
                        })
                        .inner
                        .clicked()
                    {
                        actions.push(CleanupDetailsAction::TriggerPermanent);
                    }
                });
                ui.add_space(10.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for item in &view_model.items {
                        surface_panel(ui, |ui| {
                            let size_width = 104.0;
                            let path_width = (ui.available_width() - size_width - 42.0).max(220.0);
                            ui.horizontal(|ui| {
                                let mut checked = item.checked;
                                if ui
                                    .add_enabled_ui(item.enabled, |ui| {
                                        ui.checkbox(&mut checked, "")
                                    })
                                    .inner
                                    .changed()
                                {
                                    actions.push(CleanupDetailsAction::ToggleTarget {
                                        path: item.target.path.clone(),
                                        checked,
                                    });
                                }
                                if ui
                                    .add_sized(
                                        [path_width, 22.0],
                                        egui::SelectableLabel::new(item.selected, &item.path_value),
                                    )
                                    .clicked()
                                {
                                    actions.push(CleanupDetailsAction::FocusTarget(
                                        item.target.clone(),
                                    ));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_sized(
                                            [size_width, 20.0],
                                            egui::Label::new(
                                                egui::RichText::new(&item.size_value).strong(),
                                            ),
                                        );
                                    },
                                );
                            });
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(self.cleanup_risk_color(item.risk), "●");
                                ui.label(item.risk_label);
                                ui.label("·");
                                ui.label(item.category_label);
                                if let Some(unused_days) = item.unused_days_label.as_ref() {
                                    ui.label("·");
                                    ui.label(unused_days);
                                }
                                ui.label("·");
                                ui.label(&item.score_label);
                            });
                            ui.label(
                                egui::RichText::new(item.reason_text)
                                    .text_style(egui::TextStyle::Small)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    }
                });

                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(view_model.footer_primary_enabled, |ui| {
                            sized_primary_button(ui, 220.0, &view_model.footer_primary_label)
                        })
                        .inner
                        .clicked()
                    {
                        actions.push(CleanupDetailsAction::TriggerPrimary);
                    }
                    if ui.button(&view_model.close_label).clicked() {
                        actions.push(CleanupDetailsAction::Close);
                    }
                });
            });

        for action in actions {
            if matches!(action, CleanupDetailsAction::Close) {
                keep_open = false;
            }
            self.handle_cleanup_details_action(category, action);
        }
        if !keep_open {
            self.cleanup.detail_category = None;
        }
    }

    #[allow(dead_code)]
    fn handle_cleanup_details_action(
        &mut self,
        category: CleanupCategory,
        action: CleanupDetailsAction,
    ) {
        match action {
            CleanupDetailsAction::SelectCategory(category) => {
                self.cleanup.detail_category = Some(category);
            }
            CleanupDetailsAction::ToggleTarget { path, checked } => {
                if checked {
                    self.cleanup.selected_paths.insert(path);
                } else {
                    self.cleanup.selected_paths.remove(path.as_ref());
                }
            }
            CleanupDetailsAction::FocusTarget(target) => {
                if let Some(node_id) = target.node_id {
                    self.select_node(node_id, SelectionSource::Table);
                } else {
                    self.select_path(target.path.as_ref(), SelectionSource::Table);
                }
            }
            CleanupDetailsAction::SelectAllSafe => self.select_all_safe_cleanup_items(category),
            CleanupDetailsAction::ClearSelected => self.clear_selected_cleanup_items(category),
            CleanupDetailsAction::OpenSelectedLocation => {
                self.open_selected_cleanup_target_location();
            }
            CleanupDetailsAction::TriggerPrimary => self.trigger_cleanup_details_primary(category),
            CleanupDetailsAction::TriggerPermanent => {
                self.queue_cleanup_category_delete_with_mode(category, ExecutionMode::Permanent);
            }
            CleanupDetailsAction::Close => {}
        }
    }

    #[allow(dead_code)]
    fn handle_delete_confirm_action(
        &mut self,
        request: DeleteRequestScope,
        action: DeleteConfirmAction,
    ) {
        match action {
            DeleteConfirmAction::Confirm => {
                self.queue_delete_request(request, ExecutionMode::Permanent);
            }
            DeleteConfirmAction::Close => {}
        }
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    fn handle_cleanup_delete_confirm_action(
        &mut self,
        request: CleanupDeleteRequest,
        action: CleanupDeleteConfirmAction,
    ) {
        match action {
            CleanupDeleteConfirmAction::Confirm => {
                self.queue_delete_request(
                    DeleteRequestScope {
                        label: request.label,
                        targets: request.targets,
                        selection_origin: SelectionOrigin::Manual,
                    },
                    request.mode,
                );
            }
            CleanupDeleteConfirmAction::Close => {}
        }
    }

    fn trigger_cleanup_details_primary(&mut self, category: CleanupCategory) {
        if category == CleanupCategory::Cache {
            self.queue_cleanup_category_delete(category);
        } else {
            self.queue_cleanup_category_delete_with_mode(category, ExecutionMode::RecycleBin);
        }
    }

    fn open_selected_cleanup_target_location(&mut self) {
        let Some(target) = self.selected_target() else {
            return;
        };
        self.open_path_location(target.path.as_ref());
    }

    #[allow(dead_code)]
    fn ui_cleanup_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.cleanup.pending_delete.clone() else {
            return;
        };
        let view_model = self.cleanup_delete_confirmation_view_model(&request);

        let mut keep_open = true;
        let mut actions = Vec::new();
        let screen_size = ctx.input(|i| i.screen_rect().size());
        egui::Window::new(self.t("一键清理确认", "Confirm Cleanup"))
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_size(egui::vec2(760.0, 560.0))
            .max_size(egui::vec2(
                (screen_size.x - 48.0).max(720.0),
                (screen_size.y - 48.0).max(520.0),
            ))
            .show(ctx, |ui| {
                ui.set_min_size(egui::vec2(680.0, 460.0));
                ui.label(egui::RichText::new(view_model.intro).strong());
                ui.add_space(10.0);
                stat_row(
                    ui,
                    self.t("任务", "Task"),
                    &view_model.task_value,
                    view_model.task_hint,
                );
                stat_row(
                    ui,
                    self.t("项目数", "Items"),
                    &view_model.item_count_value,
                    view_model.item_count_hint,
                );
                stat_row(
                    ui,
                    self.t("预计释放", "Estimated Reclaim"),
                    &view_model.estimated_reclaim_value,
                    view_model.estimated_reclaim_hint,
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(&view_model.preview_title)
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                ui.label(
                    egui::RichText::new(&view_model.preview_hint)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(8.0);
                let list_height = (ui.available_height() - 68.0).max(160.0);
                egui::ScrollArea::vertical()
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for item in &view_model.preview_items {
                            surface_panel(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(&item.size_value)
                                        .text_style(egui::TextStyle::Button)
                                        .color(river_teal()),
                                );
                                ui.add_space(4.0);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&item.path_value).monospace(),
                                    )
                                    .wrap(),
                                );
                            });
                            ui.add_space(8.0);
                        }
                    });
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(!self.delete_active(), |ui| {
                            sized_primary_button(ui, 150.0, view_model.confirm_label)
                        })
                        .inner
                        .clicked()
                    {
                        actions.push(CleanupDeleteConfirmAction::Confirm);
                    }
                    if ui.button(self.t("取消", "Cancel")).clicked() {
                        actions.push(CleanupDeleteConfirmAction::Close);
                    }
                });
            });

        for action in actions {
            if matches!(
                action,
                CleanupDeleteConfirmAction::Close | CleanupDeleteConfirmAction::Confirm
            ) {
                keep_open = false;
            }
            self.handle_cleanup_delete_confirm_action(request.clone(), action);
        }
        if !keep_open {
            self.cleanup.pending_delete = None;
        }
    }

    #[allow(dead_code)]
    fn ui_duplicate_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.duplicates.pending_delete.clone() else {
            return;
        };

        let mut keep_open = true;
        let mut actions = Vec::new();
        egui::Window::new(self.t("一键清理确认", "Confirm Cleanup"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(460.0);
                ui.label(
                    egui::RichText::new(self.t(
                        "建议：日常清理优先移到回收站。只有在你非常确定时才使用永久删除。",
                        "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
                    ))
                    .strong(),
                );
                ui.add_space(10.0);
                stat_row(
                    ui,
                    self.t("项目数", "Items"),
                    &format_count(request.targets.len() as u64),
                    "",
                );
                stat_row(
                    ui,
                    self.t("任务", "Task"),
                    &format_count(request.group_count as u64),
                    "",
                );
                stat_row(
                    ui,
                    self.t("预计释放", "Estimated Reclaim"),
                    &format_bytes(request.estimated_bytes),
                    self.t(
                        "建议：日常清理优先移到回收站。只有在你非常确定时才使用永久删除。",
                        "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
                    ),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(self.t(
                        "建议：日常清理优先移到回收站。只有在你非常确定时才使用永久删除。",
                        "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let permanent =
                        egui::Button::new(self.t("永久删除", "Delete Permanently")).fill(danger_red());
                    if ui.add_enabled(!self.delete_active(), permanent).clicked() {
                        actions.push(DuplicateDeleteConfirmAction::Permanent);
                    }
                    if ui
                        .add_enabled(
                            !self.delete_active(),
                            egui::Button::new(self.t("移到回收站", "Move to Recycle Bin")),
                        )
                        .clicked()
                    {
                        actions.push(DuplicateDeleteConfirmAction::RecycleBin);
                    }
                    if ui.button(self.t("取消", "Cancel")).clicked() {
                        actions.push(DuplicateDeleteConfirmAction::Close);
                    }
                });
            });

        for action in actions {
            if matches!(
                action,
                DuplicateDeleteConfirmAction::Close
                    | DuplicateDeleteConfirmAction::RecycleBin
                    | DuplicateDeleteConfirmAction::Permanent
            ) {
                keep_open = false;
            }
            self.handle_duplicate_delete_confirm_action(action);
        }
        if !keep_open {
            self.duplicates.pending_delete = None;
        }
    }

    #[allow(dead_code)]
    fn ui_execution_failure_details_dialog(&mut self, ctx: &egui::Context) {
        if !self.execution_failure_details_open() {
            return;
        }
        let Some(view_model) = self.execution_failure_details_view_model() else {
            self.set_execution_failure_details_open(false);
            return;
        };

        let mut keep_open = true;
        let mut requested_close = false;
        let screen_size = ctx.input(|i| i.screen_rect().size());
        egui::Window::new(&view_model.title)
            .open(&mut keep_open)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_size(egui::vec2(720.0, 560.0))
            .max_size(egui::vec2(
                (screen_size.x - 48.0).max(680.0),
                (screen_size.y - 48.0).max(500.0),
            ))
            .show(ctx, |ui| {
                ui.set_min_size(egui::vec2(640.0, 440.0));
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&view_model.title)
                                .text_style(egui::TextStyle::Name("title".into())),
                        );
                        ui.label(
                            egui::RichText::new(&view_model.intro)
                                .text_style(egui::TextStyle::Small)
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui
                            .button(&view_model.close_label)
                            .on_hover_text(&view_model.close_hint)
                            .clicked()
                        {
                            requested_close = true;
                        }
                    });
                });
                ui.add_space(10.0);
                surface_panel(ui, |ui| {
                    stat_row(
                        ui,
                        &view_model.summary_title,
                        &view_model.summary_value,
                        &view_model.summary_hint,
                    );
                });
                ui.add_space(10.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for item in &view_model.items {
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    surface_panel(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.colored_label(
                                            danger_red(),
                                            egui::RichText::new(&item.failure_title).strong(),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal_top(|ui| {
                                            let button_width = 160.0;
                                            let path_width =
                                                (ui.available_width() - button_width - 8.0)
                                                    .max(180.0);
                                            ui.add_sized(
                                                [path_width, 0.0],
                                                egui::Label::new(
                                                    egui::RichText::new(&item.path_value)
                                                        .monospace()
                                                        .color(ui.visuals().text_color()),
                                                )
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                            );
                                            if ui
                                                .add_sized(
                                                    [button_width, CONTROL_HEIGHT],
                                                    egui::Button::new(
                                                        &view_model.open_location_label,
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.open_path_location(&item.path_value);
                                            }
                                        });
                                        ui.add_space(6.0);
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&item.failure_body)
                                                    .text_style(egui::TextStyle::Small),
                                            )
                                            .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.add_space(6.0);
                                        ui.label(
                                            egui::RichText::new(&item.suggestion_title)
                                                .text_style(egui::TextStyle::Small)
                                                .color(river_teal()),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&item.suggestion_value)
                                                    .text_style(egui::TextStyle::Small)
                                                    .color(ui.visuals().weak_text_color()),
                                            )
                                            .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        if let Some(detail) = item.technical_detail_value.as_ref() {
                                            ui.add_space(6.0);
                                            ui.label(
                                                egui::RichText::new(&item.technical_detail_title)
                                                    .text_style(egui::TextStyle::Small)
                                                    .color(ui.visuals().weak_text_color()),
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(detail)
                                                        .text_style(egui::TextStyle::Small)
                                                        .monospace()
                                                        .color(ui.visuals().weak_text_color()),
                                                )
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                            );
                                        }
                                    });
                                },
                            );
                            ui.add_space(8.0);
                        }
                    });
            });

        if requested_close {
            keep_open = false;
        }
        if !keep_open {
            self.set_execution_failure_details_open(false);
        }
    }

    #[allow(dead_code)]
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
                        "{} {:.1}s  |  {}  |  {} {}",
                        self.t("删除中", "Deleting"),
                        snapshot.started_at.elapsed().as_secs_f32(),
                        truncate_middle(&snapshot.label, 32),
                        format_count(snapshot.target_count as u64),
                        self.t("项", "items")
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
            }
        });
    }

    #[allow(dead_code)]
    fn ui_delete_activity_banner(&mut self, ui: &mut egui::Ui) {
        if let Some(session) = self.delete_session.as_ref() {
            let snapshot = session.snapshot();
            let phase = snapshot.started_at.elapsed().as_secs_f32();
            let pulse = ((phase.sin() + 1.0) * 0.5 * 0.7 + 0.15).clamp(0.08, 0.92);

            tone_banner(
                ui,
                match snapshot.mode {
                    ExecutionMode::RecycleBin => {
                        self.t("正在后台移到回收站", "Moving to Recycle Bin in Background")
                    }
                    ExecutionMode::FastPurge => {
                        self.t("正在后台释放空间", "Reclaiming Space in Background")
                    }
                    ExecutionMode::Permanent => {
                        self.t("正在后台永久删除", "Deleting Permanently in Background")
                    }
                },
                &format!(
                    "{}  |  {} / {}  |  {} {} / {} {}  |  {} {:.1}s  |  {}",
                    truncate_middle(&snapshot.label, 56),
                    format_count(snapshot.completed_count as u64),
                    format_count(snapshot.target_count as u64),
                    format_count(snapshot.succeeded_count as u64),
                    self.t("成功", "succeeded"),
                    format_count(snapshot.failed_count as u64),
                    self.t("失败", "failed"),
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
            if let Some(current_path) = snapshot.current_path.as_ref() {
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(truncate_middle(current_path, 72))
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    )
                    .wrap(),
                );
            }
            return;
        }

        let Some(snapshot) = self
            .delete_finalize_session
            .as_ref()
            .and_then(|session| session.snapshot())
        else {
            return;
        };
        let phase = snapshot.started_at.elapsed().as_secs_f32();
        let pulse = ((phase.sin() + 1.0) * 0.5 * 0.7 + 0.15).clamp(0.08, 0.92);
        tone_banner(
            ui,
            self.t("正在同步删除结果", "Synchronizing Cleanup Results"),
            &format!(
                "{}  |  {} {} / {} {}  |  {} {:.1}s  |  {}",
                truncate_middle(&snapshot.label, 56),
                format_count(snapshot.succeeded_count as u64),
                self.t("成功", "succeeded"),
                format_count(snapshot.failed_count as u64),
                self.t("失败", "failed"),
                self.t("已耗时", "Elapsed"),
                phase,
                self.t(
                    "删除已经完成，正在后台整理清理建议和重复文件数据。",
                    "Deletion finished. Cleanup suggestions and duplicate data are being synchronized in the background.",
                )
            ),
        );
        ui.add_space(6.0);
        ui.add(
            egui::ProgressBar::new(pulse)
                .desired_width(ui.available_width().max(220.0))
                .text(self.t(
                    "系统正在同步删除后的结果",
                    "System is synchronizing post-delete results",
                )),
        );
    }
}

impl eframe::App for DirOtterNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| !i.events.is_empty() || i.pointer.delta() != egui::Vec2::ZERO) {
            self.last_user_activity = Instant::now();
        }
        self.process_scan_events();
        self.process_scan_finalize_events();
        self.process_delete_events();
        self.process_delete_finalize_events();
        self.process_duplicate_scan_events();
        self.process_result_store_load_events();
        self.process_memory_release_events();
        self.process_queued_delete();
        self.maybe_refresh_memory_status();
        self.maybe_auto_release_memory();
        if !self.advanced_tools_enabled && matches!(self.page, Page::Errors | Page::Diagnostics) {
            self.page = Page::Dashboard;
        }
        self.apply_theme(ctx);
        let delete_active = self.delete_active();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "DirOtter {}",
            self.status_text()
        )));
        if self.scan_active()
            || delete_active
            || self.system_memory_release_active()
            || self.duplicate_scan_session.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        egui::TopBottomPanel::top("top_bar")
            .exact_height(TOOLBAR_HEIGHT)
            .show_separator_line(false)
            .frame(toolbar_frame(ctx))
            .show(ctx, |ui| ui_shell::ui_toolbar(self, ui));

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(STATUSBAR_HEIGHT)
            .show_separator_line(false)
            .frame(statusbar_frame(ctx))
            .show(ctx, |ui| ui_shell::ui_statusbar(self, ui));

        egui::SidePanel::left("nav")
            .exact_width(NAV_WIDTH)
            .resizable(false)
            .show_separator_line(false)
            .frame(panel_frame(ctx))
            .show(ctx, |ui| ui_shell::ui_nav(self, ui));

        egui::SidePanel::right("inspector")
            .exact_width(INSPECTOR_WIDTH)
            .resizable(true)
            .show_separator_line(false)
            .frame(panel_frame(ctx))
            .show(ctx, |ui| ui_shell::ui_inspector(self, ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(ctx.style().visuals.window_fill)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::same(24.0)),
            )
            .show(ctx, |ui| {
                if delete_active {
                    self.ui_delete_activity_banner(ui);
                    ui.add_space(12.0);
                }
                match self.page {
                    Page::Dashboard => {
                        with_scrollable_page_width(ui, DASHBOARD_PAGE_MAX_WIDTH, |ui| {
                            self.ui_dashboard(ui)
                        })
                    }
                    Page::CurrentScan => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 40.0, |ui| {
                            self.ui_current_scan(ui)
                        })
                    }
                    Page::Duplicates => {
                        with_page_width_fill_height(ui, DUPLICATES_PAGE_MAX_WIDTH, |ui| {
                            self.ui_duplicates(ui)
                        })
                    }
                    Page::Errors => with_page_width_fill_height(ui, PAGE_MAX_WIDTH + 20.0, |ui| {
                        self.ui_errors(ui)
                    }),
                    Page::Diagnostics => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 20.0, |ui| {
                            self.ui_diagnostics(ui)
                        })
                    }
                    Page::Settings => {
                        with_scrollable_page_width(ui, SETTINGS_PAGE_MAX_WIDTH, |ui| {
                            self.ui_settings(ui, ctx)
                        })
                    }
                }
            });

        self.ui_cleanup_details_window(ctx);
        self.ui_execution_failure_details_dialog(ctx);
        self.ui_cleanup_delete_confirm_dialog(ctx);
        self.ui_duplicate_delete_confirm_dialog(ctx);
        self.ui_delete_confirm_dialog(ctx);
    }
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
    for (font_name, data) in load_system_font_fallbacks() {
        fonts
            .font_data
            .insert(font_name.clone(), egui::FontData::from_owned(data));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push(font_name.clone());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push(font_name);
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

fn load_system_font_fallbacks() -> Vec<(String, Vec<u8>)> {
    let candidates: &[(&str, &str)] = if cfg!(target_os = "windows") {
        &[
            ("cjk-fallback-msyh", "C:\\Windows\\Fonts\\msyh.ttc"),
            ("indic-fallback-nirmala", "C:\\Windows\\Fonts\\Nirmala.ttf"),
            (
                "thai-fallback-leelawadee",
                "C:\\Windows\\Fonts\\LeelawUI.ttf",
            ),
        ]
    } else if cfg!(target_os = "macos") {
        &[
            (
                "cjk-fallback-pingfang",
                "/System/Library/Fonts/PingFang.ttc",
            ),
            (
                "cjk-fallback-hiragino",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
            ),
            (
                "kr-fallback-applegothic",
                "/System/Library/Fonts/AppleSDGothicNeo.ttc",
            ),
            (
                "thai-fallback-thonburi",
                "/System/Library/Fonts/Thonburi.ttc",
            ),
            (
                "indic-fallback-kohinoor",
                "/System/Library/Fonts/Kohinoor.ttc",
            ),
        ]
    } else {
        &[
            (
                "cjk-fallback-noto",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            ),
            (
                "cjk-fallback-noto-ttc",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            ),
            (
                "cjk-fallback-noto-otf",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.otf",
            ),
            (
                "arabic-fallback-noto",
                "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
            ),
            (
                "hebrew-fallback-noto",
                "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
            ),
            (
                "indic-fallback-noto",
                "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
            ),
            (
                "thai-fallback-noto",
                "/usr/share/fonts/truetype/noto/NotoSansThai-Regular.ttf",
            ),
        ]
    };

    candidates
        .iter()
        .filter_map(|(font_name, path)| {
            fs::read(path)
                .ok()
                .map(|data| ((*font_name).to_string(), data))
        })
        .collect()
}

pub fn river_teal() -> egui::Color32 {
    egui::Color32::from_rgb(0x2F, 0x7F, 0x86)
}

pub fn river_teal_hover() -> egui::Color32 {
    egui::Color32::from_rgb(0x27, 0x6D, 0x73)
}

pub fn river_teal_active() -> egui::Color32 {
    egui::Color32::from_rgb(0x1F, 0x5C, 0x61)
}

pub fn danger_red() -> egui::Color32 {
    egui::Color32::from_rgb(0xD5, 0x4E, 0x56)
}

pub fn success_green() -> egui::Color32 {
    egui::Color32::from_rgb(0x2E, 0x8B, 0x57)
}

pub fn warning_amber() -> egui::Color32 {
    egui::Color32::from_rgb(0xC9, 0x8B, 0x2E)
}

pub fn info_blue() -> egui::Color32 {
    egui::Color32::from_rgb(0x4B, 0x7B, 0xEC)
}

// Frame builder functions
pub fn panel_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::default()
        .fill(ctx.style().visuals.panel_fill)
        .stroke(egui::Stroke::new(
            1.0,
            ctx.style().visuals.widgets.noninteractive.bg_fill,
        ))
        .rounding(egui::Rounding::same(0.0))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
}

pub fn toolbar_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::default()
        .fill(ctx.style().visuals.widgets.noninteractive.bg_fill)
        .stroke(egui::Stroke::new(
            1.0,
            ctx.style().visuals.widgets.noninteractive.bg_fill,
        ))
        .rounding(egui::Rounding::same(0.0))
        .inner_margin(egui::Margin::symmetric(16.0, 8.0))
}

pub fn statusbar_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::default()
        .fill(ctx.style().visuals.widgets.noninteractive.bg_fill)
        .stroke(egui::Stroke::new(
            1.0,
            ctx.style().visuals.widgets.noninteractive.bg_fill,
        ))
        .rounding(egui::Rounding::same(0.0))
        .inner_margin(egui::Margin::symmetric(16.0, 4.0))
}

pub fn surface_panel<R>(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::default()
        .inner_margin(egui::Margin::same(24.0))
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_fill,
        ))
        .rounding(egui::Rounding::same(0.0))
        .fill(ui.visuals().widgets.inactive.bg_fill)
        .show(ui, f)
        .inner
}

// build_dark_visuals() 已移至 theme.rs 中的 ColorPalette::dark()

// Layout helper functions
pub fn with_scrollable_page_width<R>(
    ui: &mut egui::Ui,
    width: f32,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available = ui.available_width();
    let content_width = available.min(width).max(320.0);
    let half_margin = ((available - content_width).max(0.0) * 0.5).floor();

    egui::ScrollArea::vertical()
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(half_margin);
                ui.vertical(|ui| {
                    ui.set_width(content_width);
                    f(ui)
                })
            })
            .inner
        })
        .inner
        .inner
}

pub fn with_page_width_fill_height<R>(
    ui: &mut egui::Ui,
    width: f32,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available = ui.available_width();
    let content_width = available.min(width).max(320.0);
    let half_margin = ((available - content_width).max(0.0) * 0.5).floor();

    ui.horizontal(|ui| {
        ui.add_space(half_margin);
        ui.vertical(|ui| {
            ui.set_width(content_width);
            f(ui)
        })
    })
    .inner
    .inner
}

// build_light_visuals() 已移至 theme.rs 中的 ColorPalette::light()

pub fn surface_frame(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::default()
        .fill(visuals.faint_bg_color)
        .outer_margin(egui::Margin::same(0.0))
        .inner_margin(egui::Margin::same(CARD_PADDING))
        .rounding(egui::Rounding::same(0.0))
        .stroke(egui::Stroke::new(CARD_STROKE_WIDTH, border_color(visuals)))
}

fn show_frame_with_relaxed_clip<R>(
    ui: &mut egui::Ui,
    frame: egui::Frame,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let clip_rect = ui.clip_rect();
    ui.scope(|ui| {
        ui.set_clip_rect(clip_rect.expand(6.0));
        frame.show(ui, add_contents)
    })
    .inner
}

fn border_color(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.dark_mode {
        egui::Color32::from_rgb(0x2B, 0x38, 0x3E)
    } else {
        egui::Color32::from_rgb(0xC8, 0xD0, 0xCE)
    }
}

pub fn with_page_width<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available = ui.available_width();
    let width = (available - PAGE_SIDE_GUTTER).max(320.0).min(max_width);
    ui.allocate_ui_with_layout(
        egui::vec2(available, 0.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_width(width);
            ui.set_max_width(width);
            add_contents(ui)
        },
    )
    .inner
}

fn page_header(ui: &mut egui::Ui, eyebrow: &str, title: &str, subtitle: &str) {
    ui.add_space(16.0);
    ui.label(
        egui::RichText::new(eyebrow)
            .text_style(egui::TextStyle::Small)
            .color(river_teal()),
    );
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(title)
            .text_style(egui::TextStyle::Heading)
            .strong(),
    );
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(subtitle)
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(20.0);
}

fn settings_section(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    surface_panel(ui, |ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new(title).text_style(egui::TextStyle::Name("title".into())));
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(20.0);
        add_contents(ui);
        ui.add_space(12.0);
    });
}

fn dashboard_panel<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let mut frame = surface_frame(ui);
    frame.outer_margin = egui::Margin::same(0.0);
    frame.inner_margin = egui::Margin::same(CARD_PADDING + 8.0);
    frame.show(ui, add_contents)
}

fn dashboard_split(
    ui: &mut egui::Ui,
    min_column_width: f32,
    gap: f32,
    left: impl FnOnce(&mut egui::Ui),
    right: impl FnOnce(&mut egui::Ui),
) {
    let width = ui.available_width();
    if width < min_column_width * 2.0 + gap {
        left(ui);
        ui.add_space(gap);
        right(ui);
        return;
    }

    let left_width = ((width - gap) / 2.0).floor();
    let right_width = (width - gap - left_width).max(min_column_width);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(left_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            left,
        );
        ui.add_space(gap);
        ui.allocate_ui_with_layout(
            egui::vec2(right_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            right,
        );
    });
}

fn dashboard_metric_tile(
    ui: &mut egui::Ui,
    title: &str,
    value: &str,
    subtitle: &str,
    accent: egui::Color32,
) {
    dashboard_panel(ui, |ui| {
        let width = ui.available_width();
        ui.set_min_width(width);
        ui.set_max_width(width);
        ui.colored_label(accent, egui::RichText::new(title).strong());
        ui.add_space(10.0);
        ui.label(egui::RichText::new(value).size(28.0).strong());
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
    });
}

fn dashboard_metric_row(ui: &mut egui::Ui, cards: &[(&str, String, String, egui::Color32)]) {
    let gap = 18.0;
    let width = ui.available_width();
    let card_width =
        ((width - gap * (cards.len().saturating_sub(1) as f32)) / cards.len() as f32).max(160.0);
    ui.horizontal_top(|ui| {
        for (idx, card) in cards.iter().enumerate() {
            ui.allocate_ui_with_layout(
                egui::vec2(card_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| dashboard_metric_tile(ui, card.0, &card.1, &card.2, card.3),
            );
            if idx + 1 < cards.len() {
                ui.add_space(gap);
            }
        }
    });
}

fn empty_state_panel(ui: &mut egui::Ui, title: &str, body: &str) {
    let visuals = ui.visuals();
    let frame = egui::Frame::default()
        .fill(if visuals.dark_mode {
            egui::Color32::from_rgb(0x1A, 0x24, 0x29)
        } else {
            egui::Color32::from_rgb(0xEC, 0xF1, 0xEF)
        })
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::same(14.0))
        .stroke(egui::Stroke::new(1.0, border_color(visuals)));
    show_frame_with_relaxed_clip(ui, frame, |ui| {
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

fn settings_row(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_controls: impl FnOnce(&mut egui::Ui),
) {
    let row_width = ui.available_width();
    let label_width = (row_width * 0.34).clamp(180.0, 280.0);
    let control_width = (row_width - label_width - 18.0).max(240.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.label(egui::RichText::new(title).strong());
                ui.label(
                    egui::RichText::new(subtitle)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
            },
        );
        ui.add_space(18.0);
        ui.allocate_ui_with_layout(
            egui::vec2(control_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            add_controls,
        );
    });
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str, accent: egui::Color32) {
    surface_panel(ui, |ui| {
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

#[allow(clippy::too_many_arguments)]
fn render_ranked_size_list(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    empty_body: &str,
    items: &[dirotter_scan::RankedPath],
    total: u64,
    selection: &mut SelectionState,
    execution_report: &mut Option<ExecutionReport>,
) {
    surface_panel(ui, |ui| {
        ui.push_id(("ranked-panel", title), |ui| {
            ui.label(egui::RichText::new(title).text_style(egui::TextStyle::Name("title".into())));
            ui.label(
                egui::RichText::new(subtitle)
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);

            if items.is_empty() {
                empty_state_panel(ui, title, empty_body);
                return;
            }

            let denom = total.max(items.iter().map(|(_, size)| *size).max().unwrap_or(1));
            let visuals = ui.visuals().clone();
            let active_bg = if visuals.dark_mode {
                egui::Color32::from_rgb(0x22, 0x2D, 0x33)
            } else {
                egui::Color32::from_rgb(0xF4, 0xF8, 0xF7)
            };
            let progress_bg = if visuals.dark_mode {
                egui::Color32::from_rgb(0x17, 0x1F, 0x24)
            } else {
                egui::Color32::from_rgb(0xE1, 0xE8, 0xE6)
            };
            let accent = river_teal();
            let border = border_color(&visuals);

            for (idx, (path, size)) in items.iter().enumerate() {
                let selected = selection.selected_path.as_deref() == Some(path.as_ref());
                let bg = if selected {
                    visuals.selection.bg_fill
                } else {
                    active_bg
                };
                let response = egui::Frame::default()
                    .fill(bg)
                    .stroke(egui::Stroke::new(1.0, border))
                    .rounding(egui::Rounding::same(0.0))
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                    .show(ui, |ui| {
                        ui.set_min_height(58.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}. {}",
                                    idx + 1,
                                    truncate_middle(path.as_ref(), 54)
                                ))
                                .strong(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new(format_bytes(*size)).strong());
                                },
                            );
                        });
                        ui.add_space(7.0);
                        let ratio = (*size as f32 / denom as f32).clamp(0.0, 1.0);
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 14.0),
                            egui::Sense::hover(),
                        );
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 0.0, progress_bg);
                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                rect.min,
                                egui::vec2((rect.width() * ratio).max(2.0), rect.height()),
                            ),
                            0.0,
                            accent,
                        );
                        painter.text(
                            rect.right_center() - egui::vec2(4.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            format!("{:.0}%", ratio * 100.0),
                            egui::FontId::proportional(11.0),
                            visuals.strong_text_color(),
                        );
                    })
                    .response;
                if response
                    .interact(egui::Sense::click())
                    .on_hover_text(path.as_ref())
                    .clicked()
                {
                    selection.selected_path = Some(path.to_string());
                    selection.source = Some(SelectionSource::Table);
                    selection.selected_node = None;
                    *execution_report = None;
                }
                ui.add_space(10.0);
            }
        });
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

pub fn stat_row(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str) {
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

pub fn stacked_stat_block(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str) {
    ui.label(egui::RichText::new(title).strong());
    ui.add_space(4.0);
    ui.label(egui::RichText::new(value).size(20.0).strong());
    if !subtitle.is_empty() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
    }
}

// Formatting helper functions
pub fn truncate_middle(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let half = max_chars / 2;
    let start = input.chars().take(half).collect::<String>();
    let end = input
        .chars()
        .rev()
        .take(max_chars - half - 1)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{}…{}", start, end)
}

pub fn format_count(value: u64) -> String {
    let mut result = String::new();
    let digits = value.to_string().chars().rev().collect::<Vec<_>>();
    for (i, digit) in digits.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(*digit);
    }
    result.chars().rev().collect()
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[(&str, u64)] = &[
        ("PB", 1 << 50),
        ("TB", 1 << 40),
        ("GB", 1 << 30),
        ("MB", 1 << 20),
        ("KB", 1 << 10),
    ];
    for &(unit, threshold) in UNITS {
        if bytes >= threshold {
            return format!("{:.1} {}", bytes as f64 / threshold as f64, unit);
        }
    }
    format!("{} B", bytes)
}

// UI component helper functions
pub fn status_badge(ui: &mut egui::Ui, status: &str, active: bool) {
    let fill = if active {
        river_teal()
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let text_color = if active {
        egui::Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    let mut button = egui::Button::new(egui::RichText::new(status).color(text_color));
    button = button
        .fill(fill)
        .min_size(egui::vec2(140.0, STATUS_BADGE_HEIGHT));
    ui.add(button);
}

pub fn compact_stat_chip(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new(value).text_style(egui::TextStyle::Name("title".into())));
    });
}

pub fn sized_selectable(
    ui: &mut egui::Ui,
    width: f32,
    selected: bool,
    label: &str,
) -> egui::Response {
    let fill = if selected {
        river_teal()
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let text_color = if selected {
        egui::Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    ui.add_sized(
        [width, CONTROL_HEIGHT],
        egui::Button::new(egui::RichText::new(label).color(text_color))
            .fill(fill)
            .stroke(ui.visuals().widgets.inactive.bg_stroke),
    )
}

pub fn sized_button(ui: &mut egui::Ui, width: f32, label: &str) -> egui::Response {
    ui.add_sized(
        [width, CONTROL_HEIGHT],
        egui::Button::new(egui::RichText::new(label).color(ui.visuals().text_color()))
            .fill(ui.visuals().widgets.inactive.bg_fill)
            .stroke(ui.visuals().widgets.inactive.bg_stroke),
    )
}

pub fn sized_primary_button(ui: &mut egui::Ui, width: f32, label: &str) -> egui::Response {
    ui.add_sized(
        [width, CONTROL_HEIGHT],
        egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE))
            .fill(river_teal()),
    )
}

pub fn sized_danger_button(ui: &mut egui::Ui, width: f32, label: &str) -> egui::Response {
    ui.add_sized(
        [width, CONTROL_HEIGHT],
        egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE))
            .fill(danger_red()),
    )
}

pub fn tone_banner(ui: &mut egui::Ui, title: &str, body: &str) {
    surface_panel(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(title).text_style(egui::TextStyle::Name("title".into())));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(body)
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
        });
    });
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

fn short_volume_label(volume: &dirotter_platform::VolumeInfo) -> String {
    #[cfg(target_os = "windows")]
    {
        volume.mount_point.trim_end_matches(['\\', '/']).to_string()
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

#[cfg(target_os = "windows")]
fn detect_system_locale_name() -> Option<String> {
    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(locale_name: *mut u16, cch_locale_name: i32) -> i32;
        fn GetUserPreferredUILanguages(
            flags: u32,
            num_languages: *mut u32,
            languages_buffer: *mut u16,
            buffer_length: *mut u32,
        ) -> i32;
    }

    const MUI_LANGUAGE_NAME: u32 = 0x8;
    const LOCALE_NAME_MAX_LENGTH: usize = 85;

    unsafe {
        let mut num_languages = 0u32;
        let mut buffer_length = 0u32;
        if GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut num_languages,
            std::ptr::null_mut(),
            &mut buffer_length,
        ) != 0
            && buffer_length > 1
        {
            let mut buffer = vec![0u16; buffer_length as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut num_languages,
                buffer.as_mut_ptr(),
                &mut buffer_length,
            ) != 0
            {
                let end = buffer
                    .iter()
                    .position(|&ch| ch == 0)
                    .unwrap_or(buffer.len());
                if end > 0 {
                    if let Ok(locale) = String::from_utf16(&buffer[..end]) {
                        return Some(locale);
                    }
                }
            }
        }

        let mut buffer = vec![0u16; LOCALE_NAME_MAX_LENGTH];
        let len = GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32);
        if len > 1 {
            return String::from_utf16(&buffer[..(len - 1) as usize]).ok();
        }
    }

    None
}

#[cfg(not(target_os = "windows"))]
fn detect_system_locale_name() -> Option<String> {
    std::env::var("LC_ALL")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("LANGUAGE")
                .ok()
                .filter(|value| !value.is_empty())
        })
        .or_else(|| std::env::var("LANG").ok().filter(|value| !value.is_empty()))
}

fn detect_lang() -> Lang {
    let locale = detect_system_locale_name()
        .unwrap_or_default()
        .to_lowercase();

    detect_lang_from_locale(&locale)
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use dirotter_core::{NodeId, NodeKind, NodeStore};

    fn make_test_app() -> DirOtterNativeApp {
        DirOtterNativeApp {
            egui_ctx: egui::Context::default(),
            page: Page::Dashboard,
            available_volumes: Vec::new(),
            root_input: "d:\\".into(),
            status: AppStatus::Idle,
            summary: ScanSummary::default(),
            store: None,
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
            delete_finalize_session: None,
            duplicate_scan_session: None,
            result_store_load_session: None,
            memory_release_session: None,
            scan_mode: ScanMode::Quick,
            scan_current_path: None,
            scan_last_event_at: None,
            scan_cancel_requested: false,
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
            cleanup: CleanupPanelState::default(),
            duplicates: DuplicatePanelState {
                sort: Some(DuplicateSort::Waste),
                ..DuplicatePanelState::default()
            },
            duplicate_prep: DuplicatePrepState::default(),
            execution_report: None,
            pending_delete_confirmation: None,
            queued_delete: None,
            explorer_feedback: None,
            maintenance_feedback: None,
            last_system_memory_release: None,
            process_memory: None,
            system_memory: None,
            last_memory_status_refresh: None,
            last_user_activity: Instant::now(),
            last_auto_memory_release_at: None,
            errors: Vec::new(),
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::for_tests().expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
            missing_result_store_root: None,
        }
    }

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
    fn ranked_items_do_not_depend_on_live_filesystem_metadata() {
        let items = vec![
            ("z:\\snapshot-only\\folder".into(), 2048),
            ("z:\\snapshot-only\\file.bin".into(), 1024),
        ];

        assert_eq!(
            DirOtterNativeApp::materialize_ranked_items(&items, 8, true),
            items
        );
    }

    #[test]
    fn quick_duplicate_mode_keeps_actionable_thresholds_reasonable() {
        let mut app = make_test_app();
        app.duplicates.review_mode = DuplicateReviewMode::Quick;

        let cfg = app.duplicate_dup_config();

        assert_eq!(cfg.min_candidate_size, 1024 * 1024);
        assert_eq!(cfg.min_candidate_total_waste, 8 * 1024 * 1024);
        assert!(cfg.quick_actionable_only);
    }

    #[test]
    fn cleanup_rules_include_common_low_risk_cache_roots() {
        for path in [
            "c:\\Users\\Carter\\AppData\\Local\\Microsoft\\Windows\\INetCache",
            "c:\\Users\\Carter\\AppData\\Local\\Packages\\app\\LocalCache",
            "c:\\repo\\.cache\\webpack",
            "c:\\repo\\pkg\\__pycache__",
        ] {
            assert_eq!(
                cleanup::cleanup_category_for_path(path, NodeKind::Dir),
                CleanupCategory::Cache
            );
            assert_eq!(
                cleanup::cleanup_risk_for_path(path, CleanupCategory::Cache),
                RiskLevel::Low
            );
        }
    }

    #[test]
    fn light_theme_resets_visual_mode_and_control_colors() {
        let mut visuals = egui::Visuals::dark();
        theme::apply_theme(
            &mut visuals,
            theme::ThemeMode::Light,
            &theme::ColorPalette::light(),
        );

        assert!(!visuals.dark_mode);
        assert_eq!(
            visuals.widgets.inactive.bg_fill,
            theme::ColorPalette::light().widget_inactive_fill
        );
        assert_eq!(
            visuals.widgets.inactive.fg_stroke.color,
            theme::ColorPalette::light().text
        );
    }

    #[test]
    fn locale_detection_supports_all_shipped_languages() {
        assert_eq!(detect_lang_from_locale("ar_SA"), Lang::Ar);
        assert_eq!(detect_lang_from_locale("de_DE"), Lang::De);
        assert_eq!(detect_lang_from_locale("fr_FR"), Lang::Fr);
        assert_eq!(detect_lang_from_locale("es_ES.UTF-8"), Lang::Es);
        assert_eq!(detect_lang_from_locale("he_IL"), Lang::He);
        assert_eq!(detect_lang_from_locale("hi_IN"), Lang::Hi);
        assert_eq!(detect_lang_from_locale("id_ID"), Lang::Id);
        assert_eq!(detect_lang_from_locale("it_IT"), Lang::It);
        assert_eq!(detect_lang_from_locale("ja_JP"), Lang::Ja);
        assert_eq!(detect_lang_from_locale("ko_KR"), Lang::Ko);
        assert_eq!(detect_lang_from_locale("nl_NL"), Lang::Nl);
        assert_eq!(detect_lang_from_locale("pl_PL"), Lang::Pl);
        assert_eq!(detect_lang_from_locale("ru_RU"), Lang::Ru);
        assert_eq!(detect_lang_from_locale("th_TH"), Lang::Th);
        assert_eq!(detect_lang_from_locale("tr_TR"), Lang::Tr);
        assert_eq!(detect_lang_from_locale("uk_UA"), Lang::Uk);
        assert_eq!(detect_lang_from_locale("vi_VN"), Lang::Vi);
        assert_eq!(detect_lang_from_locale("zh_CN"), Lang::Zh);
        assert_eq!(detect_lang_from_locale("en_US"), Lang::En);
    }

    #[test]
    fn live_snapshot_updates_rankings_without_building_store_on_ui_thread() {
        let mut app = make_test_app();
        app.status = AppStatus::Scanning;
        app.scan_session = Some(ScanSession {
            cancel: Arc::new(AtomicBool::new(false)),
            relay: Arc::new(Mutex::new(ScanRelayState {
                latest_progress: None,
                pending_batches: VecDeque::new(),
                latest_snapshot: Some((
                    SnapshotDelta {
                        changed_nodes: vec![NodeId(0)],
                        summary: ScanSummary::default(),
                        top_files_delta: Vec::new(),
                        top_dirs_delta: Vec::new(),
                    },
                    dirotter_scan::SnapshotView::Live(dirotter_scan::LiveSnapshotView {
                        changed_node_count: 1,
                        top_files: vec![("d:\\huge.bin".into(), 64)],
                        top_dirs: vec![("d:\\Users".into(), 128)],
                        selection: dirotter_scan::SelectionState {
                            focused: None,
                            expanded: Vec::new(),
                        },
                    }),
                )),
                finished: None,
                last_event_at: Instant::now(),
                dropped_batches: 0,
                dropped_snapshots: 0,
                dropped_progress: 0,
            })),
        });

        app.process_scan_events();

        assert!(app.store.is_none());
        assert_eq!(app.live_top_files, vec![("d:\\huge.bin".into(), 64)]);
        assert_eq!(app.live_top_dirs, vec![("d:\\Users".into(), 128)]);
    }

    #[test]
    fn result_store_reloads_only_current_session_results() {
        let mut app = make_test_app();
        assert!(!app.can_reload_result_store_from_cache());

        app.summary.bytes_observed = 42;
        assert!(app.can_reload_result_store_from_cache());

        app.summary.bytes_observed = 0;
        app.completed_top_files.push(("d:\\sdk.zip".into(), 42));
        assert!(app.can_reload_result_store_from_cache());

        app.store = Some(NodeStore::default());
        assert!(!app.can_reload_result_store_from_cache());
    }

    #[test]
    fn language_settings_round_trip_for_all_supported_languages() {
        assert_eq!(parse_lang_setting("ar"), Some(Lang::Ar));
        assert_eq!(parse_lang_setting("de"), Some(Lang::De));
        assert_eq!(parse_lang_setting("en"), Some(Lang::En));
        assert_eq!(parse_lang_setting("he"), Some(Lang::He));
        assert_eq!(parse_lang_setting("hi"), Some(Lang::Hi));
        assert_eq!(parse_lang_setting("id"), Some(Lang::Id));
        assert_eq!(parse_lang_setting("it"), Some(Lang::It));
        assert_eq!(parse_lang_setting("ja"), Some(Lang::Ja));
        assert_eq!(parse_lang_setting("ko"), Some(Lang::Ko));
        assert_eq!(parse_lang_setting("nl"), Some(Lang::Nl));
        assert_eq!(parse_lang_setting("pl"), Some(Lang::Pl));
        assert_eq!(parse_lang_setting("ru"), Some(Lang::Ru));
        assert_eq!(parse_lang_setting("zh"), Some(Lang::Zh));
        assert_eq!(parse_lang_setting("fr"), Some(Lang::Fr));
        assert_eq!(parse_lang_setting("es"), Some(Lang::Es));
        assert_eq!(parse_lang_setting("th"), Some(Lang::Th));
        assert_eq!(parse_lang_setting("tr"), Some(Lang::Tr));
        assert_eq!(parse_lang_setting("uk"), Some(Lang::Uk));
        assert_eq!(parse_lang_setting("vi"), Some(Lang::Vi));
        assert_eq!(lang_setting_value(Lang::Ar), "ar");
        assert_eq!(lang_setting_value(Lang::De), "de");
        assert_eq!(lang_setting_value(Lang::Fr), "fr");
        assert_eq!(lang_setting_value(Lang::Es), "es");
        assert_eq!(lang_setting_value(Lang::Vi), "vi");
        assert_eq!(supported_languages().len(), 19);
    }

    #[test]
    fn missing_key_patch_does_not_override_chinese_source_text() {
        assert_eq!(
            translate_ui(
                Lang::Zh,
                "尚未选择任何文件或目录。可以从实时列表、重复文件或错误中心点选对象。",
                "No file or folder is selected yet. Pick one from the live list, duplicate review, or errors.",
            ),
            "尚未选择任何文件或目录。可以从实时列表、重复文件或错误中心点选对象。"
        );
    }

    #[test]
    fn recent_multilingual_patch_keys_do_not_fall_back_to_english() {
        let keys = [
            "Scan Strategy",
            "Default strategy is enough for normal cleanup. Open advanced pacing only for huge folders, external drives, or stress testing.",
            "Advanced scan pacing",
            "Start a scan to see which directories consume the most space.",
            "Start a scan to surface the largest files worth reviewing first.",
            "Deletion has finished. Cleanup suggestions and duplicate data are synchronizing in the background and will refresh automatically.",
            "Synchronizing cleanup suggestions and duplicate data after deletion",
            "Select a file or folder from the live list, duplicate review, or errors first.",
        ];

        for &lang in supported_languages() {
            if matches!(lang, Lang::Zh | Lang::En) {
                continue;
            }
            for key in keys {
                assert_ne!(
                    translate_ui(lang, "中文占位", key),
                    key,
                    "{lang:?} fell back to English for {key}"
                );
            }
        }
    }

    #[test]
    fn french_and_spanish_translations_cover_primary_actions() {
        assert_eq!(translate_fr("Start Scan"), "Démarrer l'analyse");
        assert_eq!(translate_es("Start Scan"), "Iniciar escaneo");
        assert_eq!(translate_fr("Open File Location"), "Ouvrir l'emplacement");
        assert_eq!(translate_es("Open File Location"), "Abrir ubicación");
        assert_eq!(translate_fr("Failure Details"), "Détails des échecs");
        assert_eq!(translate_es("Failure Details"), "Detalles del fallo");
        assert_eq!(
            translate_fr("failed, view details"),
            "échecs, voir le détail"
        );
        assert_eq!(translate_es("failed, view details"), "fallos, ver detalles");
    }

    fn extract_english_translation_keys(source: &str) -> Vec<String> {
        let bytes = source.as_bytes();
        let needle = b"self.t(";
        let mut keys = Vec::new();
        let mut i = 0usize;

        while i + needle.len() <= bytes.len() {
            if &bytes[i..i + needle.len()] != needle {
                i += 1;
                continue;
            }

            let start = i + needle.len();
            let mut j = start;
            let mut depth = 1usize;
            let mut in_string = false;
            let mut escape = false;

            while j < bytes.len() && depth > 0 {
                let b = bytes[j];
                if in_string {
                    if escape {
                        escape = false;
                    } else if b == b'\\' {
                        escape = true;
                    } else if b == b'"' {
                        in_string = false;
                    }
                } else if b == b'"' {
                    in_string = true;
                } else if b == b'(' {
                    depth += 1;
                } else if b == b')' {
                    depth -= 1;
                }
                j += 1;
            }

            let inner = &source[start..j.saturating_sub(1)];
            let mut literals = Vec::new();
            let inner_bytes = inner.as_bytes();
            let mut k = 0usize;

            while k < inner_bytes.len() {
                if inner_bytes[k] != b'"' {
                    k += 1;
                    continue;
                }

                k += 1;
                let mut literal = String::new();
                let mut inner_escape = false;

                while k < inner_bytes.len() {
                    let b = inner_bytes[k];
                    if inner_escape {
                        match b {
                            b'n' => literal.push('\n'),
                            b'r' => literal.push('\r'),
                            b't' => literal.push('\t'),
                            b'\\' => literal.push('\\'),
                            b'"' => literal.push('"'),
                            _ => literal.push(b as char),
                        }
                        inner_escape = false;
                    } else if b == b'\\' {
                        inner_escape = true;
                    } else if b == b'"' {
                        break;
                    } else {
                        literal.push(b as char);
                    }
                    k += 1;
                }

                literals.push(literal);
                k += 1;
            }

            if let Some(en) = literals.last() {
                if !keys.iter().any(|existing| existing == en) {
                    keys.push(en.clone());
                }
            }

            i = j;
        }

        keys
    }

    fn assert_translations_cover_source(source: &str, label: &str) {
        let keys = extract_english_translation_keys(source);
        for &lang in supported_languages() {
            let missing: Vec<_> = keys
                .iter()
                .filter(|key| !has_translation(lang, key))
                .cloned()
                .collect();
            assert!(
                missing.is_empty(),
                "missing {label} translations for {lang:?}: {missing:?}"
            );
        }
    }

    #[test]
    fn shipped_translations_cover_all_current_ui_english_keys() {
        let source = include_str!("lib.rs");
        let source = source
            .split("mod ui_tests")
            .next()
            .expect("source before tests");
        assert_translations_cover_source(source, "lib");
    }

    #[test]
    fn all_supported_languages_cover_view_models_failure_detail_keys() {
        assert_translations_cover_source(include_str!("view_models.rs"), "view-model");
    }

    #[test]
    fn all_supported_languages_cover_result_pages_keys() {
        assert_translations_cover_source(include_str!("result_pages.rs"), "result-page");
    }

    #[test]
    fn all_supported_languages_cover_dashboard_keys() {
        assert_translations_cover_source(include_str!("dashboard_impl.rs"), "dashboard");
    }

    #[test]
    fn all_supported_languages_cover_duplicates_pages_keys() {
        assert_translations_cover_source(include_str!("duplicates_pages.rs"), "duplicates-page");
    }

    #[test]
    fn all_supported_languages_cover_settings_pages_keys() {
        assert_translations_cover_source(include_str!("settings_pages.rs"), "settings-page");
    }

    #[test]
    fn truncate_middle_keeps_ends() {
        let truncated = truncate_middle("very-long-file-name.iso", 10);
        assert!(truncated.starts_with("very"));
        assert!(truncated.ends_with(".iso"));
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
            node_id: None,
            name: "drop".into(),
            path: "e:\\drop".into(),
            size_bytes: 20,
            kind: NodeKind::Dir,
            file_count: 1,
            dir_count: 1,
        };

        let rebuilt = DirOtterNativeApp::rebuild_store_without_targets(&store, &[target])
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
            status: AppStatus::Completed,
            summary: ScanSummary::default(),
            store: Some(store),
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
            delete_finalize_session: None,
            duplicate_scan_session: None,
            result_store_load_session: None,
            memory_release_session: None,
            scan_mode: ScanMode::Quick,
            scan_current_path: None,
            scan_last_event_at: None,
            scan_cancel_requested: false,
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
            cleanup: CleanupPanelState::default(),
            duplicates: DuplicatePanelState {
                sort: Some(DuplicateSort::Waste),
                ..DuplicatePanelState::default()
            },
            duplicate_prep: DuplicatePrepState::default(),
            execution_report: None,
            pending_delete_confirmation: None,
            queued_delete: None,
            explorer_feedback: None,
            maintenance_feedback: None,
            last_system_memory_release: None,
            process_memory: None,
            system_memory: None,
            last_memory_status_refresh: None,
            last_user_activity: Instant::now(),
            last_auto_memory_release_at: None,
            errors: Vec::new(),
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::for_tests().expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState {
                selected_node: Some(sdk),
                selected_path: Some("d:\\appdata\\local\\sdk".into()),
                source: Some(SelectionSource::Table),
            },
            error_filter: ErrorFilter::All,
            missing_result_store_root: None,
        };

        let (_, _, files) = app.contextual_ranked_files_panel(8);
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .all(|(path, _)| path.as_ref().starts_with("d:\\appdata\\local\\sdk\\")));
        assert_eq!(files[0].0.as_ref(), "d:\\appdata\\local\\sdk\\system.img");
    }

    #[test]
    fn select_path_replaces_previous_node_selection_for_error_items() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "c:\\".into(), "c:\\".into(), NodeKind::Dir, 0);
        let windows = store.add_node(
            Some(root),
            "Windows".into(),
            "c:\\Windows".into(),
            NodeKind::Dir,
            0,
        );
        let servicing = store.add_node(
            Some(windows),
            "servicing".into(),
            "c:\\Windows\\servicing".into(),
            NodeKind::Dir,
            0,
        );
        store.rollup();

        let mut app = DirOtterNativeApp {
            egui_ctx: egui::Context::default(),
            page: Page::Errors,
            available_volumes: Vec::new(),
            root_input: "c:\\".into(),
            status: AppStatus::Completed,
            summary: ScanSummary::default(),
            store: Some(store),
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
            delete_finalize_session: None,
            duplicate_scan_session: None,
            result_store_load_session: None,
            memory_release_session: None,
            scan_mode: ScanMode::Quick,
            scan_current_path: None,
            scan_last_event_at: None,
            scan_cancel_requested: false,
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
            cleanup: CleanupPanelState::default(),
            duplicates: DuplicatePanelState {
                sort: Some(DuplicateSort::Waste),
                ..DuplicatePanelState::default()
            },
            duplicate_prep: DuplicatePrepState::default(),
            execution_report: None,
            pending_delete_confirmation: None,
            queued_delete: None,
            explorer_feedback: None,
            maintenance_feedback: None,
            last_system_memory_release: None,
            process_memory: None,
            system_memory: None,
            last_memory_status_refresh: None,
            last_user_activity: Instant::now(),
            last_auto_memory_release_at: None,
            errors: Vec::new(),
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::for_tests().expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState {
                selected_node: Some(servicing),
                selected_path: Some("c:\\Windows\\servicing".into()),
                source: Some(SelectionSource::Table),
            },
            error_filter: ErrorFilter::All,
            missing_result_store_root: None,
        };

        app.select_path("c:\\$Recycle.Bin\\S-1-5-18", SelectionSource::Error);

        let target = app.selected_target().expect("selected target");
        assert!(matches!(app.selection.source, Some(SelectionSource::Error)));
        assert_eq!(app.selection.selected_node, None);
        assert_eq!(target.path.as_ref(), "c:\\$Recycle.Bin\\S-1-5-18");
        assert_eq!(target.name.as_ref(), "S-1-5-18");
    }

    #[test]
    fn inspector_fast_cleanup_only_appears_for_low_risk_cache_targets() {
        let app = make_test_app();
        let cache_target = SelectedTarget {
            node_id: None,
            name: Arc::from("Cache"),
            path: Arc::from("c:\\users\\carter\\appdata\\local\\temp\\edge\\cache"),
            size_bytes: 1024,
            kind: NodeKind::Dir,
            file_count: 0,
            dir_count: 1,
        };
        let system_target = SelectedTarget {
            node_id: None,
            name: Arc::from("EdgeCore"),
            path: Arc::from("c:\\program files (x86)\\microsoft\\edgecore"),
            size_bytes: 1024,
            kind: NodeKind::Dir,
            file_count: 0,
            dir_count: 1,
        };

        let cache_actions = app.inspector_actions_view_model(Some(&cache_target));
        assert!(cache_actions.show_fast_cleanup);
        assert!(cache_actions.can_fast_cleanup);
        assert!(cache_actions.info_message.is_none());

        let system_actions = app.inspector_actions_view_model(Some(&system_target));
        assert!(!system_actions.show_fast_cleanup);
        assert!(!system_actions.can_fast_cleanup);
        assert!(system_actions.info_message.is_some());
    }

    #[test]
    fn released_store_can_reload_from_snapshot() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "d:\\".into(), "d:\\".into(), NodeKind::Dir, 0);
        store.add_node(
            Some(root),
            "sdk.zip".into(),
            "d:\\sdk.zip".into(),
            NodeKind::File,
            42,
        );
        store.rollup();

        let mut app = DirOtterNativeApp {
            egui_ctx: egui::Context::default(),
            page: Page::Dashboard,
            available_volumes: Vec::new(),
            root_input: "d:\\".into(),
            status: AppStatus::Completed,
            summary: ScanSummary::default(),
            store: Some(store),
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
            delete_finalize_session: None,
            duplicate_scan_session: None,
            result_store_load_session: None,
            memory_release_session: None,
            scan_mode: ScanMode::Quick,
            scan_current_path: None,
            scan_last_event_at: None,
            scan_cancel_requested: false,
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
            cleanup: CleanupPanelState::default(),
            duplicates: DuplicatePanelState {
                sort: Some(DuplicateSort::Waste),
                ..DuplicatePanelState::default()
            },
            duplicate_prep: DuplicatePrepState::default(),
            execution_report: None,
            pending_delete_confirmation: None,
            queued_delete: None,
            explorer_feedback: None,
            maintenance_feedback: None,
            last_system_memory_release: None,
            process_memory: None,
            system_memory: None,
            last_memory_status_refresh: None,
            last_user_activity: Instant::now(),
            last_auto_memory_release_at: None,
            errors: Vec::new(),
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::for_tests().expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
            missing_result_store_root: None,
        };
        app.sync_summary_from_store();
        app.sync_rankings_from_store();
        app.refresh_cleanup_analysis();

        assert!(app.release_result_store_to_snapshot());
        assert!(app.store.is_none());
        assert_eq!(app.summary.bytes_observed, 42);

        assert!(app.ensure_store_loaded_from_cache());
        assert!(app.store.is_some());
        assert_eq!(app.summary.bytes_observed, 42);
        assert!(app
            .completed_top_files
            .iter()
            .any(|(path, size)| path.as_ref() == "d:\\sdk.zip" && *size == 42));
    }

    #[test]
    fn cleanup_analysis_groups_cache_downloads_and_blocked_system_files() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "c:\\".into(), "c:\\".into(), NodeKind::Dir, 0);
        let temp = store.add_node(
            Some(root),
            "Temp".into(),
            "c:\\Users\\Carter\\AppData\\Local\\Temp".into(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(temp),
            "cache.bin".into(),
            "c:\\Users\\Carter\\AppData\\Local\\Temp\\cache.bin".into(),
            NodeKind::File,
            512 * 1024 * 1024,
        );
        let downloads = store.add_node(
            Some(root),
            "Downloads".into(),
            "c:\\Users\\Carter\\Downloads".into(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(downloads),
            "setup.exe".into(),
            "c:\\Users\\Carter\\Downloads\\setup.exe".into(),
            NodeKind::File,
            2 * 1024 * 1024 * 1024,
        );
        let windows = store.add_node(
            Some(root),
            "Windows".into(),
            "c:\\Windows".into(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(windows),
            "system.dll".into(),
            "c:\\Windows\\System32\\system.dll".into(),
            NodeKind::File,
            900 * 1024 * 1024,
        );
        store.rollup();

        let analysis = DirOtterNativeApp::build_cleanup_analysis(&store);
        assert!(analysis.reclaimable_bytes >= (512 + 2048) * 1024 * 1024);
        assert_eq!(analysis.quick_clean_bytes, 512 * 1024 * 1024);
        assert!(analysis
            .categories
            .iter()
            .any(|category| category.category == CleanupCategory::Cache));
        assert!(analysis
            .categories
            .iter()
            .any(|category| category.category == CleanupCategory::Downloads));
        let system = analysis
            .categories
            .iter()
            .find(|category| category.category == CleanupCategory::System)
            .expect("system category");
        assert_eq!(system.reclaimable_bytes, 0);
        assert_eq!(system.blocked_bytes, 900 * 1024 * 1024);
    }

    #[test]
    fn cleanup_risk_rules_keep_cache_safe_but_appdata_non_cache_warning() {
        let cache_category = DirOtterNativeApp::cleanup_category_for_path(
            "c:\\Users\\Carter\\AppData\\Local\\Temp\\cache.bin",
            NodeKind::File,
        );
        let cache_risk = DirOtterNativeApp::cleanup_risk_for_path(
            "c:\\Users\\Carter\\AppData\\Local\\Temp\\cache.bin",
            cache_category,
        );
        assert_eq!(cache_category, CleanupCategory::Cache);
        assert_eq!(cache_risk, RiskLevel::Low);

        let installer_category = DirOtterNativeApp::cleanup_category_for_path(
            "c:\\Users\\Carter\\AppData\\Roaming\\installer.msi",
            NodeKind::File,
        );
        let installer_risk = DirOtterNativeApp::cleanup_risk_for_path(
            "c:\\Users\\Carter\\AppData\\Roaming\\installer.msi",
            installer_category,
        );
        assert_eq!(installer_category, CleanupCategory::Installer);
        assert_eq!(installer_risk, RiskLevel::Medium);
    }

    #[test]
    fn cleanup_confirmation_lists_all_targets_with_full_paths() {
        let app = make_test_app();
        let targets: Vec<_> = (0..7)
            .map(|idx| SelectedTarget {
                node_id: None,
                name: format!("cache-{idx}.tmp").into(),
                path: format!("d:\\cache\\nested\\item-{idx}\\cache-{idx}.tmp").into(),
                size_bytes: ((idx + 1) as u64) * 1024,
                kind: NodeKind::File,
                file_count: 1,
                dir_count: 0,
            })
            .collect();
        let request = CleanupDeleteRequest {
            label: "Fast cleanup".into(),
            targets,
            estimated_bytes: 28 * 1024,
            mode: ExecutionMode::FastPurge,
        };

        let view_model = app.cleanup_delete_confirmation_view_model(&request);

        assert_eq!(view_model.preview_items.len(), 7);
        assert_eq!(
            view_model.preview_items[6].path_value,
            "d:\\cache\\nested\\item-6\\cache-6.tmp"
        );
        assert!(view_model.preview_hint.contains("full paths"));
    }

    #[test]
    fn execution_report_view_model_exposes_failure_details_action() {
        let mut app = make_test_app();
        app.execution_report = Some(ExecutionReport {
            mode: ExecutionMode::FastPurge,
            attempted: 3,
            succeeded: 1,
            failed: 2,
            items: vec![
                dirotter_actions::ExecutionResultItem {
                    path: "d:\\cache\\ok.tmp".into(),
                    success: true,
                    message: "staged for background purge".into(),
                    failure_kind: None,
                    retries: 0,
                    platform_kind: None,
                    io_kind: None,
                    path_kind: Some("file".into()),
                },
                dirotter_actions::ExecutionResultItem {
                    path: "d:\\cache\\locked.tmp".into(),
                    success: false,
                    message: "execute failed after 3 attempt(s): Io".into(),
                    failure_kind: Some(ActionFailureKind::Io),
                    retries: 2,
                    platform_kind: None,
                    io_kind: Some("permission_denied".into()),
                    path_kind: Some("file".into()),
                },
                dirotter_actions::ExecutionResultItem {
                    path: "d:\\cache\\protected.tmp".into(),
                    success: false,
                    message: "validation failed: Protected".into(),
                    failure_kind: Some(ActionFailureKind::Protected),
                    retries: 0,
                    platform_kind: None,
                    io_kind: None,
                    path_kind: Some("file".into()),
                },
            ],
        });

        let view_model = app
            .inspector_execution_report_view_model()
            .expect("execution report view model");

        assert_eq!(view_model.summary_value, "Fast cleanup");
        assert_eq!(
            view_model.failure_detail_label.as_deref(),
            Some("2 failed, view details")
        );
    }

    #[test]
    fn delete_task_view_model_shows_background_result_sync_phase() {
        let mut app = make_test_app();
        app.delete_finalize_session = Some(controller::DeleteFinalizeSession {
            relay: Arc::new(Mutex::new(controller::DeleteFinalizeRelayState {
                finished: None,
                snapshot: Some(controller::DeleteFinalizeState {
                    started_at: Instant::now(),
                    label: "Quick Cache Cleanup".into(),
                    target_count: 22,
                    mode: ExecutionMode::FastPurge,
                    succeeded_count: 15,
                    failed_count: 7,
                }),
            })),
        });

        let view_model = app
            .delete_task_view_model()
            .expect("delete task view model");

        assert_eq!(view_model.title, "Background Task: Sync Results");
        assert_eq!(view_model.progress_value, "Syncing in background");
        assert!(view_model
            .description
            .contains("synchronizing in the background"));
        assert!(view_model.progress_hint.contains("15 succeeded / 7 failed"));
    }

    #[test]
    fn result_store_load_is_deferred_while_delete_sync_is_active() {
        let mut app = make_test_app();
        app.summary.bytes_observed = 42;
        app.delete_finalize_session = Some(controller::DeleteFinalizeSession {
            relay: Arc::new(Mutex::new(controller::DeleteFinalizeRelayState {
                finished: None,
                snapshot: Some(controller::DeleteFinalizeState {
                    started_at: Instant::now(),
                    label: "Quick Cache Cleanup".into(),
                    target_count: 3,
                    mode: ExecutionMode::FastPurge,
                    succeeded_count: 1,
                    failed_count: 2,
                }),
            })),
        });

        app.begin_result_store_load_if_needed();

        assert!(app.result_store_load_session.is_none());
    }

    #[test]
    fn result_store_load_uses_background_session_instead_of_sync_cache_load() {
        let mut app = make_test_app();
        app.summary.bytes_observed = 42;

        app.begin_result_store_load_if_needed();

        assert!(app.store.is_none());
        assert!(app.result_store_load_session.is_some());
    }

    #[test]
    fn completed_zero_duplicate_review_does_not_restart() {
        let mut app = make_test_app();
        app.store = Some(NodeStore::default());

        app.apply_duplicate_groups(Vec::new());
        app.start_duplicate_scan_if_needed();

        assert!(app.duplicates.review_completed);
        assert!(app.duplicate_scan_session.is_none());
        assert!(app.duplicates.groups.is_empty());
    }

    #[test]
    fn auto_release_skips_duplicate_page_even_under_memory_pressure() {
        let mut app = make_test_app();
        let mut store = NodeStore::default();
        let root = store.add_node(None, "d:\\".into(), "d:\\".into(), NodeKind::Dir, 0);
        store.add_node(
            Some(root),
            "archive.zip".into(),
            "d:\\archive.zip".into(),
            NodeKind::File,
            42,
        );
        store.rollup();
        app.store = Some(store);
        app.status = AppStatus::Completed;
        app.page = Page::Duplicates;
        app.last_user_activity = Instant::now() - Duration::from_secs(IDLE_MEMORY_RELEASE_SECS + 5);
        app.system_memory = Some(dirotter_platform::SystemMemoryStats {
            memory_load_percent: HIGH_MEMORY_LOAD_PERCENT,
            total_phys_bytes: 16,
            available_phys_bytes: 1,
            low_memory_signal: Some(true),
        });

        app.maybe_auto_release_memory();

        assert!(app.store.is_some());
    }

    #[test]
    fn auto_release_skips_while_duplicate_verification_is_running() {
        let mut app = make_test_app();
        let mut store = NodeStore::default();
        let root = store.add_node(None, "d:\\".into(), "d:\\".into(), NodeKind::Dir, 0);
        store.add_node(
            Some(root),
            "archive.zip".into(),
            "d:\\archive.zip".into(),
            NodeKind::File,
            42,
        );
        store.rollup();
        app.store = Some(store);
        app.status = AppStatus::Completed;
        app.last_user_activity = Instant::now() - Duration::from_secs(IDLE_MEMORY_RELEASE_SECS + 5);
        app.system_memory = Some(dirotter_platform::SystemMemoryStats {
            memory_load_percent: HIGH_MEMORY_LOAD_PERCENT,
            total_phys_bytes: 16,
            available_phys_bytes: 1,
            low_memory_signal: Some(true),
        });
        app.duplicate_scan_session = Some(controller::DuplicateScanSession {
            relay: Arc::new(Mutex::new(controller::DuplicateScanRelayState::default())),
        });

        app.maybe_auto_release_memory();

        assert!(app.store.is_some());
        assert!(app.duplicate_scan_session.is_some());
    }

    #[test]
    fn execution_failure_details_include_full_path_reason_and_suggestion() {
        let mut app = make_test_app();
        app.execution_report = Some(ExecutionReport {
            mode: ExecutionMode::Permanent,
            attempted: 1,
            succeeded: 0,
            failed: 1,
            items: vec![dirotter_actions::ExecutionResultItem {
                path: "d:\\very\\long\\path\\that\\should\\stay\\fully\\visible\\locked.tmp".into(),
                success: false,
                message: "execute failed after 3 attempt(s): Io".into(),
                failure_kind: Some(ActionFailureKind::Io),
                retries: 2,
                platform_kind: None,
                io_kind: Some("would_block".into()),
                path_kind: Some("file".into()),
            }],
        });

        let view_model = app
            .execution_failure_details_view_model()
            .expect("failure details view model");

        assert_eq!(view_model.items.len(), 1);
        assert_eq!(
            view_model.items[0].path_value,
            "d:\\very\\long\\path\\that\\should\\stay\\fully\\visible\\locked.tmp"
        );
        assert_eq!(
            view_model.items[0].failure_title,
            "Still Failed After Retries"
        );
        assert_eq!(view_model.open_location_label, "Open File Location");
        assert!(view_model.items[0]
            .failure_body
            .contains("retried this operation"));
        assert!(view_model.items[0]
            .suggestion_value
            .contains("may be in use"));
        assert_eq!(
            view_model.items[0].technical_detail_value.as_deref(),
            Some("execute failed after 3 attempt(s): Io")
        );
    }

    #[test]
    fn delete_feedback_banner_hides_raw_failure_reason_text() {
        let mut app = make_test_app();
        app.execution_report = Some(ExecutionReport {
            mode: ExecutionMode::FastPurge,
            attempted: 1,
            succeeded: 0,
            failed: 1,
            items: vec![dirotter_actions::ExecutionResultItem {
                path: "d:\\cache\\locked.tmp".into(),
                success: false,
                message: "execute failed after 3 attempt(s): Io".into(),
                failure_kind: Some(ActionFailureKind::Io),
                retries: 2,
                platform_kind: None,
                io_kind: Some("would_block".into()),
                path_kind: Some("file".into()),
            }],
        });

        let (_, hint, success) = app.delete_feedback_message().expect("delete feedback");

        assert!(!success);
        assert!(!hint.contains("execute failed after 3 attempt(s): Io"));
        assert!(hint.contains("Delete action failed"));
    }
}
