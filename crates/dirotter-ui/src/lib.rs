mod advanced_pages;
mod cleanup;
mod controller;
mod dashboard;
mod i18n;
mod result_pages;
mod settings_pages;
mod view_models;

use dirotter_actions::{
    build_deletion_plan_with_origin, ActionFailureKind, ExecutionMode, ExecutionReport,
    SelectionOrigin,
};
use dirotter_cache::{CacheStore, HistoryRecord};
use dirotter_core::{
    ErrorKind, NodeId, NodeKind, NodeStore, RiskLevel, ScanErrorRecord, ScanSummary, SnapshotDelta,
};
use dirotter_report::{
    default_manifest, export_diagnostics_archive, export_diagnostics_bundle, export_errors_csv,
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
use std::time::{Duration, Instant};

use cleanup::{CleanupAnalysis, CleanupCandidate, CleanupCategory};
use controller::{
    start_delete_session, start_memory_release_session, take_finished_delete,
    take_finished_memory_release, DeleteSession, MemoryReleaseSession, QueuedDeleteRequest,
};

const MAX_PENDING_BATCH_EVENTS: usize = 32;
const MAX_PENDING_SNAPSHOTS: usize = 8;
const MAX_LIVE_FILES: usize = 20_000;
const MAX_TREEMAP_CHILDREN: usize = 2_000;
const MAX_CLEANUP_DETAIL_ITEMS: usize = 48;
const MAX_CLEANUP_ITEMS_PER_CATEGORY: usize = 24;
const MAX_BLOCKED_ITEMS_PER_CATEGORY: usize = 12;
const MAX_CLEANUP_TOTAL_ITEMS: usize = 120;
const MIN_CLEANUP_BYTES: u64 = 64 * 1024 * 1024;
const MIN_CACHE_DIR_BYTES: u64 = 16 * 1024 * 1024;
const MEMORY_STATUS_REFRESH_MS: u64 = 2_000;
const IDLE_MEMORY_RELEASE_SECS: u64 = 45;
const AUTO_MEMORY_RELEASE_COOLDOWN_SECS: u64 = 120;
const HIGH_MEMORY_LOAD_PERCENT: u32 = 85;
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
const DASHBOARD_PAGE_MAX_WIDTH: f32 = 1160.0;
const SETTINGS_PAGE_MAX_WIDTH: f32 = 1040.0;
const PAGE_SIDE_GUTTER: f32 = 64.0;

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
    Treemap,
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
struct SelectedTarget {
    node_id: Option<NodeId>,
    name: Arc<str>,
    path: Arc<str>,
    size_bytes: u64,
    kind: NodeKind,
    file_count: u64,
    dir_count: u64,
}

#[derive(Clone)]
struct TreemapEntry {
    node_id: NodeId,
    name: Arc<str>,
    path: Arc<str>,
    size_bytes: u64,
    kind: NodeKind,
    file_count: u64,
    dir_count: u64,
}

#[derive(Clone)]
struct DeleteRequestScope {
    label: String,
    targets: Vec<SelectedTarget>,
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
    treemap_focus_path: Option<Arc<str>>,
    live_files: Vec<dirotter_scan::RankedPath>,
    live_top_files: Vec<dirotter_scan::RankedPath>,
    live_top_dirs: Vec<dirotter_scan::RankedPath>,
    completed_top_files: Vec<dirotter_scan::RankedPath>,
    completed_top_dirs: Vec<dirotter_scan::RankedPath>,
    last_coalesce_commit: Instant,
    cleanup: CleanupPanelState,

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

    history: Vec<HistoryRecord>,
    errors: Vec<ScanErrorRecord>,
    selected_history_id: Option<i64>,

    language: Lang,
    theme_dark: bool,
    advanced_tools_enabled: bool,
    cache: CacheStore,

    perf: PerfMetrics,
    diagnostics_json: String,
    selection: SelectionState,
    error_filter: ErrorFilter,
}

impl DirOtterNativeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        let cache = CacheStore::new("dirotter.db").expect("open sqlite cache");
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
            treemap_focus_path: None,
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            cleanup: CleanupPanelState::default(),
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
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language,
            theme_dark,
            advanced_tools_enabled,
            cache,
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
        };

        if app.advanced_tools_enabled {
            let _ = app.reload_history();
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
        if enabled {
            let _ = self.reload_history();
            self.refresh_diagnostics();
        } else {
            self.history.clear();
            self.errors.clear();
            if matches!(self.page, Page::History | Page::Errors | Page::Diagnostics) {
                self.page = Page::Dashboard;
            }
        }
    }

    fn selected_scan_config(&self) -> ScanConfig {
        ScanConfig::for_mode(self.scan_mode)
    }

    fn scan_mode_title(&self, mode: ScanMode) -> &'static str {
        match mode {
            ScanMode::Quick => self.t("快速扫描（推荐）", "Quick Scan (Recommended)"),
            ScanMode::Deep => self.t("深度扫描", "Deep Scan"),
            ScanMode::LargeDisk => self.t("超大硬盘模式", "Large Disk Mode"),
        }
    }

    fn scan_mode_description(&self, mode: ScanMode) -> &'static str {
        match mode {
            ScanMode::Quick => self.t(
                "更快进入可操作结果，适合日常整理和大多数本地磁盘。",
                "Reach actionable results faster. Best for routine cleanup and most local disks.",
            ),
            ScanMode::Deep => self.t(
                "用更稳的节奏持续展开复杂目录，适合首次全面排查。",
                "Use a steadier cadence for complex directory trees and first-pass investigations.",
            ),
            ScanMode::LargeDisk => self.t(
                "降低界面刷新压力，适合超大硬盘、外置盘和文件数极多的目录。",
                "Reduce UI refresh pressure for very large drives, external disks, or extremely dense folders.",
            ),
        }
    }

    fn scan_mode_note(&self) -> &'static str {
        self.t(
            "所有模式都会完整扫描当前范围，差异只在扫描节奏和界面刷新方式。",
            "All modes scan the same scope. The only difference is pacing and UI update cadence.",
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
        style.visuals = if self.theme_dark {
            build_dark_visuals()
        } else {
            build_light_visuals()
        };
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

    fn root_node_id(&self) -> Option<NodeId> {
        self.store
            .as_ref()?
            .nodes
            .iter()
            .find(|node| node.parent.is_none())
            .map(|node| node.id)
    }

    fn treemap_focus_target(&self) -> Option<SelectedTarget> {
        let store = self.store.as_ref()?;

        if let Some(path) = self.treemap_focus_path.as_deref() {
            if let Some(node_id) = store.path_index.get(path).copied() {
                if let Some(target) = self.target_from_node_id(node_id) {
                    if matches!(target.kind, NodeKind::Dir) {
                        return Some(target);
                    }
                }
            }
        }

        self.root_node_id()
            .and_then(|node_id| self.target_from_node_id(node_id))
    }

    fn selected_directory_target(&self) -> Option<SelectedTarget> {
        let target = self.selected_target()?;
        if matches!(target.kind, NodeKind::Dir) {
            Some(target)
        } else {
            None
        }
    }

    fn delete_request_for_target(target: SelectedTarget) -> DeleteRequestScope {
        DeleteRequestScope {
            label: target.path.to_string(),
            targets: vec![target],
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

    fn selection_matches_treemap_entry(&self, entry: &TreemapEntry) -> bool {
        self.selection.selected_node == Some(entry.node_id)
            || self.selection_matches_path(entry.path.as_ref())
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

    fn treemap_entries(&self, scope_path: &str, limit: usize) -> Vec<TreemapEntry> {
        let Some(store) = self.store.as_ref() else {
            return Vec::new();
        };
        let Some(scope_id) = store.path_index.get(scope_path).copied() else {
            return Vec::new();
        };
        let Some(children) = store.children.get(&scope_id) else {
            return Vec::new();
        };

        let mut entries: Vec<TreemapEntry> = children
            .iter()
            .filter_map(|child_id| store.nodes.get(child_id.0))
            .map(|node| TreemapEntry {
                node_id: node.id,
                name: store
                    .resolve_string_arc(node.name_id)
                    .unwrap_or_else(|| Arc::from("")),
                path: node.path.clone(),
                size_bytes: node.size_subtree.max(node.size_self),
                kind: node.kind,
                file_count: node.file_count,
                dir_count: node.dir_count,
            })
            .collect();

        entries.sort_by(|a, b| {
            b.size_bytes
                .cmp(&a.size_bytes)
                .then_with(|| a.name.cmp(&b.name))
        });
        entries.truncate(limit);
        entries
    }

    fn focus_treemap_path(&mut self, path: Arc<str>) {
        self.treemap_focus_path = Some(path.clone());
        self.select_path(path.as_ref(), SelectionSource::Treemap);
    }

    fn focus_treemap_node(&mut self, node_id: NodeId) {
        let Some(target) = self.target_from_node_id(node_id) else {
            return;
        };
        self.treemap_focus_path = Some(target.path.clone());
        self.select_node(node_id, SelectionSource::Treemap);
    }

    fn focus_treemap_parent(&mut self) {
        let Some(current) = self.treemap_focus_target() else {
            return;
        };
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let Some(node_id) = store.path_index.get(current.path.as_ref()).copied() else {
            self.treemap_focus_path = None;
            return;
        };
        let Some(node) = store.nodes.get(node_id.0) else {
            self.treemap_focus_path = None;
            return;
        };
        if let Some(parent_id) = node.parent {
            if let Some(parent) = self.target_from_node_id(parent_id) {
                self.focus_treemap_path(parent.path);
                return;
            }
        }
        self.treemap_focus_path = None;
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
            format!("{} {}", item.message, hint),
            false,
        ))
    }

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

    fn path_matches_any_target(path: &str, targets: &[SelectedTarget]) -> bool {
        targets
            .iter()
            .any(|target| Self::path_matches_target(path, target))
    }

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

    fn prune_deleted_targets(&mut self, targets: &[SelectedTarget]) {
        if targets.is_empty() {
            return;
        }

        let matches_target = |path: &str| -> bool { Self::path_matches_any_target(path, targets) };
        if self
            .treemap_focus_path
            .as_deref()
            .is_some_and(matches_target)
        {
            self.treemap_focus_path = None;
        }

        self.live_files
            .retain(|(path, _)| !matches_target(path.as_ref()));
        self.live_top_files
            .retain(|(path, _)| !matches_target(path.as_ref()));
        self.live_top_dirs
            .retain(|(path, _)| !matches_target(path.as_ref()));
        self.completed_top_files
            .retain(|(path, _)| !matches_target(path.as_ref()));
        self.completed_top_dirs
            .retain(|(path, _)| !matches_target(path.as_ref()));
        self.errors.retain(|error| !matches_target(&error.path));
        if let Some(store) = self.store.take() {
            self.store = Self::rebuild_store_without_targets(&store, targets);
            self.sync_summary_from_store();
            self.sync_rankings_from_store();
        } else {
            let released_bytes = targets.iter().map(|target| target.size_bytes).sum();
            let released_files = targets.iter().map(|target| target.file_count).sum();
            let released_dirs = targets.iter().map(|target| target.dir_count).sum();
            self.summary.bytes_observed =
                self.summary.bytes_observed.saturating_sub(released_bytes);
            self.summary.scanned_files = self.summary.scanned_files.saturating_sub(released_files);
            self.summary.scanned_dirs = self.summary.scanned_dirs.saturating_sub(released_dirs);
        }
        self.refresh_cleanup_analysis();
        self.selection = SelectionState::default();
    }

    fn start_system_memory_release(&mut self) {
        if self.memory_release_session.is_some() {
            return;
        }

        self.trim_transient_runtime_memory();
        self.maintenance_feedback = None;
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
        self.queue_delete_request(Self::delete_request_for_target(target), mode);
    }

    fn queue_delete_request(&mut self, request: DeleteRequestScope, mode: ExecutionMode) {
        self.pending_delete_confirmation = None;
        self.execution_report = None;
        self.queued_delete = Some(QueuedDeleteRequest { request, mode });
        self.egui_ctx.request_repaint();
    }

    fn process_queued_delete(&mut self) {
        if self.delete_session.is_some() {
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
            self.selection_origin(),
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
        self.delete_session.is_some()
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

        let Some(payload) = take_finished_delete(session) else {
            return;
        };

        let report = payload.report;
        let audit_payload = serde_json::json!({
            "label": payload.request.label,
            "targets": payload.request.targets.len(),
            "mode": format!("{:?}", report.mode),
            "attempted": report.attempted,
            "succeeded": report.succeeded,
            "failed": report.failed,
        })
        .to_string();
        let _ = self.cache.add_audit_event("delete_execute", &audit_payload);
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
            self.prune_deleted_targets(&succeeded_targets);
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
            SelectionSource::Table => self.t("列表", "Table"),
            SelectionSource::Treemap => self.t("结果视图", "Result View"),
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
        self.summary = payload.summary;
        self.errors = payload.errors;
        self.sync_rankings_from_store();
        self.apply_cleanup_analysis(Some(payload.cleanup_analysis));
        self.status = AppStatus::Completed;
        self.scan_current_path = None;
        self.scan_last_event_at = None;
        self.scan_cancel_requested = false;
        self.execution_report = None;
        self.treemap_focus_path = None;
        self.scan_finalize_session = None;
        self.refresh_diagnostics();
    }

    fn refresh_diagnostics(&mut self) {
        self.refresh_memory_status();
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
            "process_memory": self.process_memory,
            "system_memory": self.system_memory,
            "last_system_memory_release": self.last_system_memory_release,
            "result_store_resident": self.store.is_some(),
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
        if !self.advanced_tools_enabled {
            self.history.clear();
            self.history.shrink_to_fit();
            self.errors.clear();
            self.errors.shrink_to_fit();
        }
    }

    fn save_snapshot_before_memory_release(&mut self) -> bool {
        let Some(store) = self.store.as_ref() else {
            return false;
        };
        self.cache.save_snapshot(&self.root_input, store).is_ok()
    }

    fn release_result_store_to_snapshot(&mut self) -> bool {
        if self.store.is_none() {
            return false;
        }
        if !self.save_snapshot_before_memory_release() {
            return false;
        }
        self.store = None;
        self.selection = SelectionState::default();
        self.treemap_focus_path = None;
        true
    }

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

    fn maybe_auto_release_memory(&mut self) {
        if self.scan_active() || self.delete_active() {
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
        if self.page != Page::Treemap && matches!(self.status, AppStatus::Completed) {
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

    fn save_current_snapshot_manually(&mut self) {
        let Some(store) = self.store.as_ref() else {
            self.set_maintenance_feedback(
                self.t(
                    "当前没有可保存的扫描结果。",
                    "There is no scan result to save yet.",
                )
                .to_string(),
                false,
            );
            return;
        };
        match self.cache.save_snapshot(&self.root_input, store) {
            Ok(()) => self.set_maintenance_feedback(
                self.t(
                    "已手动保存当前快照。",
                    "Saved the current snapshot manually.",
                )
                .to_string(),
                true,
            ),
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t("保存快照失败", "Failed to save snapshot"),
                    err
                ),
                false,
            ),
        }
    }

    fn record_current_history_manually(&mut self) {
        match self.cache.record_scan_history(
            &self.root_input,
            self.summary.scanned_files,
            self.summary.scanned_dirs,
            self.summary.bytes_observed,
            self.summary.error_count,
            &self.errors,
        ) {
            Ok(id) => {
                let _ = self.reload_history();
                self.selected_history_id = Some(id);
                self.set_maintenance_feedback(
                    self.t(
                        "已手动记录当前扫描摘要。",
                        "Recorded the current scan summary manually.",
                    )
                    .to_string(),
                    true,
                );
            }
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t("记录扫描历史失败", "Failed to record scan history"),
                    err
                ),
                false,
            ),
        }
    }

    fn export_errors_csv_manually(&mut self) {
        match export_errors_csv(&self.errors, "dirotter_errors.csv") {
            Ok(()) => self.set_maintenance_feedback(
                self.t("已导出错误 CSV。", "Exported the errors CSV.")
                    .to_string(),
                true,
            ),
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t("导出错误 CSV 失败", "Failed to export errors CSV"),
                    err
                ),
                false,
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
        self.history.clear();
        self.history.shrink_to_fit();
        self.errors.clear();
        self.errors.shrink_to_fit();
        self.selected_history_id = None;
        let snapshot_saved = self.save_snapshot_before_memory_release();
        self.store = None;
        self.summary = ScanSummary::default();
        self.scan_current_path = None;
        self.scan_last_event_at = None;
        self.status = AppStatus::Idle;
        self.treemap_focus_path = None;
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
                        "已先写入磁盘快照，可在需要时重新载入结果。",
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
        self.treemap_focus_path = None;
        self.live_files.clear();
        self.live_top_files.clear();
        self.live_top_dirs.clear();
        self.completed_top_files.clear();
        self.completed_top_dirs.clear();
        self.store = None;
        self.cleanup = CleanupPanelState::default();
        self.delete_session = None;
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
            (Page::Treemap, "结果视图", "Result View"),
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

        if self.advanced_tools_enabled {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(self.t("高级工具", "Advanced Tools"))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(6.0);
            for (p, label_zh, label_en) in [
                (Page::History, "历史记录", "History"),
                (Page::Errors, "错误中心", "Errors"),
                (Page::Diagnostics, "诊断导出", "Diagnostics"),
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
                    if matches!(p, Page::History) {
                        let _ = self.reload_history();
                    }
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

    fn ui_treemap(&mut self, ui: &mut egui::Ui) {
        result_pages::ui_treemap(self, ui);
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        advanced_pages::ui_history(self, ui);
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
        let workspace_context_view = self.inspector_workspace_context_view_model();
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
                    "尚未选择任何文件或目录。可以从实时列表、结果视图或其他页面点选对象。",
                    "No file or folder is selected yet. Pick one from the live list, result view, or another page.",
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
                    self.t("已耗时", "Elapsed"),
                    &snapshot.elapsed_value,
                    snapshot.elapsed_hint,
                );
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
                        match dirotter_platform::select_in_explorer(target.path.as_ref()) {
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
                let permanent =
                    egui::Button::new(&inspector_actions_view.permanent_label).fill(danger_red());
                if ui
                    .add_enabled_ui(inspector_actions_view.can_permanent_delete, |ui| {
                        ui.add_sized([ui.available_width(), CONTROL_HEIGHT], permanent)
                    })
                    .inner
                    .clicked()
                {
                    if let Some(target) = selected_target.clone() {
                        self.pending_delete_confirmation = Some(PendingDeleteConfirmation {
                            request: Self::delete_request_for_target(target.clone()),
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
                if let Some(message) = report.detail_message.as_ref() {
                    ui.label(
                        egui::RichText::new(message)
                            .text_style(egui::TextStyle::Small)
                            .color(if report.detail_success {
                                ui.visuals().text_color()
                            } else {
                                danger_red()
                            }),
                    );
                }
            }
        });

        ui.add_space(10.0);
        surface_panel(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("工作上下文", "Workspace Context"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            stat_row(
                ui,
                self.t("根目录", "Root"),
                &workspace_context_view.root_value,
                workspace_context_view.root_hint,
            );
            stat_row(
                ui,
                self.t("来源", "Source"),
                &workspace_context_view.source_value,
                workspace_context_view.source_hint,
            );
        });
    }

    fn ui_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_delete_confirmation.clone() else {
            return;
        };
        let Some(view_model) = self.delete_confirmation_view_model(&pending) else {
            self.pending_delete_confirmation = None;
            return;
        };

        let mut keep_open = true;
        let mut confirmed_delete: Option<DeleteRequestScope> = None;
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
                        confirmed_delete = Some(pending.request.clone());
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
        if let Some(request) = confirmed_delete {
            self.queue_delete_request(request, ExecutionMode::Permanent);
        }
    }

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
        match dirotter_platform::select_in_explorer(target.path.as_ref()) {
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

    fn ui_cleanup_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.cleanup.pending_delete.clone() else {
            return;
        };
        let view_model = self.cleanup_delete_confirmation_view_model(&request);

        let mut keep_open = true;
        let mut confirmed = false;
        egui::Window::new(self.t("一键清理确认", "Confirm Cleanup"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
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
                ui.add_space(8.0);
                for item in &view_model.preview_items {
                    ui.label(item);
                }
                if let Some(label) = view_model.more_items_label.as_ref() {
                    ui.label(
                        egui::RichText::new(label)
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(!self.delete_active(), |ui| {
                            sized_primary_button(ui, 150.0, view_model.confirm_label)
                        })
                        .inner
                        .clicked()
                    {
                        confirmed = true;
                        keep_open = false;
                    }
                    if ui.button(self.t("取消", "Cancel")).clicked() {
                        keep_open = false;
                    }
                });
            });

        if !keep_open {
            self.cleanup.pending_delete = None;
        }
        if confirmed {
            self.queue_delete_request(
                DeleteRequestScope {
                    label: request.label,
                    targets: request.targets,
                },
                request.mode,
            );
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
            if let Some(process_memory) = self.process_memory {
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "DirOtter {}",
                        format_bytes(process_memory.working_set_bytes)
                    ))
                    .text_style(egui::TextStyle::Small),
                );
            }
            if let Some(system_memory) = self.system_memory {
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}  |  {} {}%",
                        format_bytes(system_memory.available_phys_bytes),
                        self.t("系统可用内存", "system free"),
                        self.t("内存负载", "load"),
                        system_memory.memory_load_percent
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(if self.system_memory_pressure_active() {
                        ui.visuals().warn_fg_color
                    } else {
                        ui.visuals().weak_text_color()
                    }),
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
            if let Some((message, success)) = self.maintenance_feedback.as_ref() {
                ui.add_space(8.0);
                tone_banner(
                    ui,
                    if *success {
                        self.t("维护完成", "Maintenance Done")
                    } else {
                        self.t("维护失败", "Maintenance Failed")
                    },
                    message,
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
                ExecutionMode::FastPurge => {
                    self.t("正在后台释放空间", "Reclaiming Space in Background")
                }
                ExecutionMode::Permanent => {
                    self.t("正在后台永久删除", "Deleting Permanently in Background")
                }
            },
            &format!(
                "{}  |  {} {}  |  {} {:.1}s  |  {}",
                truncate_middle(&snapshot.label, 56),
                format_count(snapshot.target_count as u64),
                self.t("项", "items"),
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
        if ctx.input(|i| !i.events.is_empty() || i.pointer.delta() != egui::Vec2::ZERO) {
            self.last_user_activity = Instant::now();
        }
        self.process_scan_events();
        self.process_scan_finalize_events();
        self.process_delete_events();
        self.process_memory_release_events();
        self.process_queued_delete();
        self.maybe_refresh_memory_status();
        self.maybe_auto_release_memory();
        if !self.advanced_tools_enabled
            && matches!(self.page, Page::History | Page::Errors | Page::Diagnostics)
        {
            self.page = Page::Dashboard;
        }
        self.apply_theme(ctx);
        let delete_active = self.delete_active();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "DirOtter {}",
            self.status_text()
        )));
        if self.scan_active() || delete_active || self.system_memory_release_active() {
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
                        with_scrollable_page_width(ui, DASHBOARD_PAGE_MAX_WIDTH, |ui| {
                            self.ui_dashboard(ui)
                        })
                    }
                    Page::CurrentScan => {
                        with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 40.0, |ui| {
                            self.ui_current_scan(ui)
                        })
                    }
                    Page::Treemap => {
                        with_page_width_fill_height(ui, PAGE_MAX_WIDTH, |ui| self.ui_treemap(ui))
                    }
                    Page::History => with_scrollable_page_width(ui, PAGE_MAX_WIDTH + 20.0, |ui| {
                        self.ui_history(ui)
                    }),
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
        self.ui_cleanup_delete_confirm_dialog(ctx);
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
            ("cjk-fallback-deng", "C:\\Windows\\Fonts\\Deng.ttf"),
            ("jp-fallback-yugothic", "C:\\Windows\\Fonts\\YuGothM.ttc"),
            ("kr-fallback-malgun", "C:\\Windows\\Fonts\\malgun.ttf"),
            ("indic-fallback-nirmala", "C:\\Windows\\Fonts\\Nirmala.ttf"),
            (
                "thai-fallback-leelawadee",
                "C:\\Windows\\Fonts\\LeelawUI.ttf",
            ),
            ("legacy-cjk-simhei", "C:\\Windows\\Fonts\\simhei.ttf"),
            ("legacy-cjk-simsun", "C:\\Windows\\Fonts\\simsun.ttc"),
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
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, river_teal());
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
        .outer_margin(egui::Margin::same(2.0))
        .inner_margin(egui::Margin::same(CARD_PADDING))
        .rounding(egui::Rounding::same(CARD_RADIUS as f32))
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

fn surface_panel<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    show_frame_with_relaxed_clip(ui, surface_frame(ui), add_contents)
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

fn with_scrollable_page_width<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            with_page_width(ui, max_width, |ui| {
                let inner = add_contents(ui);
                ui.add_space(28.0);
                inner
            })
        })
        .inner
}

fn with_page_width_fill_height<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available_width = ui.available_width();
    let available_height = ui.available_height();
    let width = (available_width - PAGE_SIDE_GUTTER)
        .max(320.0)
        .min(max_width);

    ui.allocate_ui_with_layout(
        egui::vec2(available_width, available_height),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_width(width);
            ui.set_max_width(width);
            ui.set_min_height(available_height);
            add_contents(ui)
        },
    )
    .inner
}

fn page_header(ui: &mut egui::Ui, eyebrow: &str, title: &str, subtitle: &str) {
    ui.label(
        egui::RichText::new(eyebrow)
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

fn settings_section(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    surface_panel(ui, |ui| {
        ui.label(egui::RichText::new(title).text_style(egui::TextStyle::Name("title".into())));
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(12.0);
        add_contents(ui);
    });
}

fn dashboard_panel<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let mut frame = surface_frame(ui);
    frame.outer_margin = egui::Margin::same(0.0);
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
        ui.add_space(6.0);
        ui.label(egui::RichText::new(value).size(22.0).strong());
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
    });
}

fn dashboard_metric_row(ui: &mut egui::Ui, cards: &[(&str, String, String, egui::Color32)]) {
    let gap = 14.0;
    let width = ui.available_width();
    let card_width =
        ((width - gap * (cards.len().saturating_sub(1) as f32)) / cards.len() as f32).max(140.0);
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

fn compact_stat_chip(ui: &mut egui::Ui, label: &str, value: &str) {
    let visuals = ui.visuals().clone();
    egui::Frame::default()
        .fill(visuals.extreme_bg_color)
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .rounding(egui::Rounding::same(CONTROL_RADIUS as f32))
        .stroke(egui::Stroke::new(1.0, border_color(&visuals)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(label)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.label(egui::RichText::new(value).strong());
            });
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
                let label = format!("{}. {}", idx + 1, truncate_middle(path.as_ref(), 52));
                let row_width = (ui.available_width() - 150.0).max(120.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [row_width, 22.0],
                            egui::SelectableLabel::new(
                                selection.selected_path.as_deref() == Some(path.as_ref()),
                                label,
                            ),
                        )
                        .clicked()
                    {
                        selection.selected_path = Some(path.to_string());
                        selection.source = Some(SelectionSource::Table);
                        selection.selected_node = None;
                        *execution_report = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format_bytes(*size));
                    });
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

fn tone_banner(ui: &mut egui::Ui, title: &str, body: &str) {
    let visuals = ui.visuals();
    let width = ui.available_width();
    let frame = egui::Frame::default()
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
        ));
    show_frame_with_relaxed_clip(ui, frame, |ui| {
        ui.set_min_width(width);
        ui.label(egui::RichText::new(title).strong().color(river_teal()));
        ui.label(body);
    });
}

fn color_note_row(ui: &mut egui::Ui, swatch: egui::Color32, title: &str, body: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::Rounding::same(6.0), swatch);
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
            treemap_focus_path: None,
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            cleanup: CleanupPanelState::default(),
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
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::new(":memory:").expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
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
    fn result_view_only_reloads_cache_for_current_session_results() {
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
    fn french_and_spanish_translations_cover_primary_actions() {
        assert_eq!(translate_fr("Start Scan"), "Démarrer l'analyse");
        assert_eq!(translate_es("Start Scan"), "Iniciar escaneo");
        assert_eq!(translate_fr("Open File Location"), "Ouvrir l'emplacement");
        assert_eq!(translate_es("Open File Location"), "Abrir ubicación");
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

    #[test]
    fn shipped_translations_cover_all_current_ui_english_keys() {
        let source = include_str!("lib.rs");
        let source = source
            .split("mod ui_tests")
            .next()
            .expect("source before tests");
        let keys = extract_english_translation_keys(source);
        for &lang in supported_languages() {
            let missing: Vec<_> = keys
                .iter()
                .filter(|key| !has_translation(lang, key))
                .cloned()
                .collect();
            assert!(
                missing.is_empty(),
                "missing translations for {lang:?}: {missing:?}"
            );
        }
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
            treemap_focus_path: None,
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            cleanup: CleanupPanelState::default(),
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
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::new(":memory:").expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState {
                selected_node: Some(sdk),
                selected_path: Some("d:\\appdata\\local\\sdk".into()),
                source: Some(SelectionSource::Table),
            },
            error_filter: ErrorFilter::All,
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
            treemap_focus_path: None,
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            cleanup: CleanupPanelState::default(),
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
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::new(":memory:").expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState {
                selected_node: Some(servicing),
                selected_path: Some("c:\\Windows\\servicing".into()),
                source: Some(SelectionSource::Table),
            },
            error_filter: ErrorFilter::All,
        };

        app.select_path("c:\\$Recycle.Bin\\S-1-5-18", SelectionSource::Error);

        let target = app.selected_target().expect("selected target");
        assert!(matches!(app.selection.source, Some(SelectionSource::Error)));
        assert_eq!(app.selection.selected_node, None);
        assert_eq!(target.path.as_ref(), "c:\\$Recycle.Bin\\S-1-5-18");
        assert_eq!(target.name.as_ref(), "S-1-5-18");
    }

    #[test]
    fn treemap_entries_only_return_direct_children() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "d:\\".into(), "d:\\".into(), NodeKind::Dir, 0);
        let users = store.add_node(
            Some(root),
            "Users".into(),
            "d:\\Users".into(),
            NodeKind::Dir,
            0,
        );
        store.add_node(
            Some(users),
            "alice.dat".into(),
            "d:\\Users\\alice.dat".into(),
            NodeKind::File,
            12,
        );
        store.add_node(
            Some(root),
            "pagefile.sys".into(),
            "d:\\pagefile.sys".into(),
            NodeKind::File,
            20,
        );
        store.rollup();

        let app = DirOtterNativeApp {
            egui_ctx: egui::Context::default(),
            page: Page::Treemap,
            available_volumes: Vec::new(),
            root_input: "d:\\".into(),
            status: AppStatus::Completed,
            summary: ScanSummary::default(),
            store: Some(store),
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
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
            treemap_focus_path: None,
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            cleanup: CleanupPanelState::default(),
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
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::new(":memory:").expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
        };

        let entries = app.treemap_entries("d:\\", 32);
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.path.as_ref() == "d:\\Users"));
        assert!(entries
            .iter()
            .any(|entry| entry.path.as_ref() == "d:\\pagefile.sys"));
        assert!(!entries
            .iter()
            .any(|entry| entry.path.as_ref() == "d:\\Users\\alice.dat"));
    }

    #[test]
    fn treemap_focus_falls_back_to_root_when_focus_is_missing() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "d:\\".into(), "d:\\".into(), NodeKind::Dir, 0);
        let users = store.add_node(
            Some(root),
            "Users".into(),
            "d:\\Users".into(),
            NodeKind::Dir,
            0,
        );
        store.rollup();

        let app = DirOtterNativeApp {
            egui_ctx: egui::Context::default(),
            page: Page::Treemap,
            available_volumes: Vec::new(),
            root_input: "d:\\".into(),
            status: AppStatus::Completed,
            summary: ScanSummary::default(),
            store: Some(store),
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
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
            treemap_focus_path: Some("d:\\missing".into()),
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            cleanup: CleanupPanelState::default(),
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
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::new(":memory:").expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState {
                selected_node: Some(users),
                selected_path: Some("d:\\Users".into()),
                source: Some(SelectionSource::Table),
            },
            error_filter: ErrorFilter::All,
        };

        let focus = app.treemap_focus_target().expect("focus target");
        assert_eq!(focus.path.as_ref(), "d:\\");
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
            treemap_focus_path: None,
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            completed_top_files: Vec::new(),
            completed_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            cleanup: CleanupPanelState::default(),
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
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: Lang::En,
            theme_dark: true,
            advanced_tools_enabled: false,
            cache: CacheStore::new(":memory:").expect("cache"),
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
            selection: SelectionState::default(),
            error_filter: ErrorFilter::All,
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
}
