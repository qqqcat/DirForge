mod i18n;

use dirotter_actions::{
    build_deletion_plan_with_origin, execute_plan, ActionFailureKind, ExecutionMode,
    ExecutionReport, SelectionOrigin,
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
use i18n::{
    detect_lang_from_locale, lang_native_label, lang_setting_value, parse_lang_setting,
    translate_es, translate_fr,
};
#[cfg(test)]
use i18n::{has_translation_es, has_translation_fr};
use std::collections::{HashMap, HashSet, VecDeque};
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
const MAX_TREEMAP_CHILDREN: usize = 2_000;
const MAX_CLEANUP_DETAIL_ITEMS: usize = 48;
const MAX_CLEANUP_ITEMS_PER_CATEGORY: usize = 24;
const MAX_BLOCKED_ITEMS_PER_CATEGORY: usize = 12;
const MAX_CLEANUP_TOTAL_ITEMS: usize = 120;
const MIN_CLEANUP_BYTES: u64 = 64 * 1024 * 1024;
const MIN_CACHE_DIR_BYTES: u64 = 16 * 1024 * 1024;
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
    En,
    Zh,
    Fr,
    Es,
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
    top_files: Vec<(String, u64)>,
    top_dirs: Vec<(String, u64)>,
}

struct ScanFinalizePayload {
    summary: ScanSummary,
    store: NodeStore,
    errors: Vec<ScanErrorRecord>,
    top_files: Vec<(String, u64)>,
    top_dirs: Vec<(String, u64)>,
    cleanup_analysis: CleanupAnalysis,
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
struct TreemapEntry {
    name: String,
    path: String,
    size_bytes: u64,
    kind: NodeKind,
    file_count: u64,
    dir_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CleanupCategory {
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
struct CleanupCandidate {
    target: SelectedTarget,
    category: CleanupCategory,
    risk: RiskLevel,
    cleanup_score: f32,
    unused_days: Option<u64>,
}

#[derive(Clone)]
struct CleanupCategorySummary {
    category: CleanupCategory,
    total_bytes: u64,
    reclaimable_bytes: u64,
    blocked_bytes: u64,
    item_count: usize,
}

#[derive(Clone, Default)]
struct CleanupAnalysis {
    reclaimable_bytes: u64,
    quick_clean_bytes: u64,
    categories: Vec<CleanupCategorySummary>,
    items: Vec<CleanupCandidate>,
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
    selected_paths: HashSet<String>,
    pending_delete: Option<CleanupDeleteRequest>,
}

#[derive(Clone)]
struct PendingDeleteConfirmation {
    request: DeleteRequestScope,
    risk: RiskLevel,
}

struct DeleteSession {
    relay: Arc<Mutex<DeleteRelayState>>,
}

struct DeleteRelayState {
    started_at: Instant,
    label: String,
    target_count: usize,
    mode: ExecutionMode,
    finished: Option<DeleteFinishedPayload>,
}

struct DeleteFinishedPayload {
    request: DeleteRequestScope,
    report: ExecutionReport,
}

struct QueuedDeleteRequest {
    request: DeleteRequestScope,
    mode: ExecutionMode,
}

impl DeleteRelayState {
    fn new(request: &DeleteRequestScope, mode: ExecutionMode) -> Self {
        Self {
            started_at: Instant::now(),
            label: request.label.clone(),
            target_count: request.targets.len(),
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
            label: relay.label.clone(),
            target_count: relay.target_count,
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
    scan_finalize_session: Option<ScanFinalizeSession>,
    delete_session: Option<DeleteSession>,
    scan_mode: ScanMode,
    scan_current_path: Option<String>,
    scan_last_event_at: Option<Instant>,
    scan_cancel_requested: bool,
    scan_dropped_batches: u64,
    scan_dropped_snapshots: u64,
    scan_dropped_progress: u64,

    pending_batch_events: VecDeque<Vec<BatchEntry>>,
    pending_snapshots: VecDeque<SnapshotDelta>,
    treemap_focus_path: Option<String>,
    live_files: Vec<(String, u64)>,
    live_top_files: Vec<(String, u64)>,
    live_top_dirs: Vec<(String, u64)>,
    completed_top_files: Vec<(String, u64)>,
    completed_top_dirs: Vec<(String, u64)>,
    last_coalesce_commit: Instant,
    cleanup: CleanupPanelState,

    execution_report: Option<ExecutionReport>,
    pending_delete_confirmation: Option<PendingDeleteConfirmation>,
    queued_delete: Option<QueuedDeleteRequest>,
    explorer_feedback: Option<(String, bool)>,
    maintenance_feedback: Option<(String, bool)>,

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
        app.refresh_diagnostics();
        app
    }

    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        match self.language {
            Lang::Zh => zh,
            Lang::En => en,
            Lang::Fr => translate_fr(en),
            Lang::Es => translate_es(en),
        }
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

    fn target_from_node_id(&self, node_id: NodeId) -> Option<SelectedTarget> {
        let store = self.store.as_ref()?;
        let node = store.nodes.get(node_id.0)?;
        Some(SelectedTarget {
            name: node.name.clone(),
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
            label: target.path.clone(),
            targets: vec![target],
        }
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

    fn cleanup_category_for_path(path: &str, kind: NodeKind) -> CleanupCategory {
        let lower = path.to_ascii_lowercase();
        let extension = PathBuf::from(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{}", ext.to_ascii_lowercase()))
            .unwrap_or_default();

        if Self::is_system_path(&lower) {
            return CleanupCategory::System;
        }
        if Self::is_cache_path(&lower, kind) {
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

    fn cleanup_risk_for_path(path: &str, category: CleanupCategory) -> RiskLevel {
        let lower = path.to_ascii_lowercase();
        if Self::is_system_path(&lower)
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
        let limit = Self::cleanup_candidate_limit(candidate.risk);
        let bucket = category_candidates.entry(candidate.category).or_default();
        bucket.push(candidate);
        bucket.sort_by(|a, b| {
            Self::rank_cleanup_candidate(b)
                .cmp(&Self::rank_cleanup_candidate(a))
                .then_with(|| a.target.path.cmp(&b.target.path))
        });
        if bucket.len() > limit {
            bucket.truncate(limit);
        }
    }

    fn build_cleanup_analysis(store: &NodeStore) -> CleanupAnalysis {
        let mut cache_dirs: Vec<&dirotter_core::Node> = store
            .nodes
            .iter()
            .filter(|node| {
                node.kind == NodeKind::Dir
                    && Self::is_cache_path(&node.path.to_ascii_lowercase(), node.kind)
                    && node.size_subtree.max(node.size_self) >= MIN_CACHE_DIR_BYTES
            })
            .collect();
        cache_dirs.sort_by(|a, b| {
            a.path
                .len()
                .cmp(&b.path.len())
                .then_with(|| b.size_subtree.cmp(&a.size_subtree))
        });

        let mut cache_scope_paths: Vec<String> = Vec::new();
        let mut category_candidates: HashMap<CleanupCategory, Vec<CleanupCandidate>> =
            HashMap::new();

        for node in cache_dirs {
            if cache_scope_paths
                .iter()
                .any(|scope| path_within_scope(&node.path, scope))
            {
                continue;
            }
            cache_scope_paths.push(node.path.clone());
            let target = SelectedTarget {
                name: node.name.clone(),
                path: node.path.clone(),
                size_bytes: node.size_subtree.max(node.size_self),
                kind: node.kind,
                file_count: node.file_count,
                dir_count: node.dir_count,
            };
            let unused_days = Self::cleanup_unused_days(&target.path);
            Self::push_ranked_cleanup_candidate(
                &mut category_candidates,
                CleanupCandidate {
                    cleanup_score: Self::cleanup_score(
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
            if cache_scope_paths
                .iter()
                .any(|scope| path_within_scope(&node.path, scope))
            {
                continue;
            }

            let category = Self::cleanup_category_for_path(&node.path, node.kind);
            let risk = Self::cleanup_risk_for_path(&node.path, category);
            if category == CleanupCategory::Other && node.size_self < MIN_CLEANUP_BYTES * 4 {
                continue;
            }
            if category != CleanupCategory::System && node.size_self < MIN_CLEANUP_BYTES {
                continue;
            }

            let unused_days = if risk == RiskLevel::High {
                None
            } else {
                Self::cleanup_unused_days(&node.path)
            };
            let score = Self::cleanup_score(node.size_self, unused_days, category, risk);
            if risk != RiskLevel::High && score < 1.0 {
                continue;
            }

            Self::push_ranked_cleanup_candidate(
                &mut category_candidates,
                CleanupCandidate {
                    target: SelectedTarget {
                        name: node.name.clone(),
                        path: node.path.clone(),
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

        let mut items: Vec<CleanupCandidate> =
            category_candidates.into_values().flatten().collect();
        items.sort_by(|a, b| {
            Self::rank_cleanup_candidate(b)
                .cmp(&Self::rank_cleanup_candidate(a))
                .then_with(|| {
                    Self::cleanup_sort_priority(a.category)
                        .cmp(&Self::cleanup_sort_priority(b.category))
                })
                .then_with(|| a.target.path.cmp(&b.target.path))
        });
        if items.len() > MAX_CLEANUP_TOTAL_ITEMS {
            items.truncate(MAX_CLEANUP_TOTAL_ITEMS);
        }

        let mut category_map: HashMap<CleanupCategory, CleanupCategorySummary> = HashMap::new();
        let mut reclaimable_bytes = 0u64;
        let mut quick_clean_bytes = 0u64;
        for item in &items {
            let summary =
                category_map
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
                summary.blocked_bytes =
                    summary.blocked_bytes.saturating_add(item.target.size_bytes);
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
                .then_with(|| {
                    Self::cleanup_sort_priority(a.category)
                        .cmp(&Self::cleanup_sort_priority(b.category))
                })
        });

        CleanupAnalysis {
            reclaimable_bytes,
            quick_clean_bytes,
            categories,
            items,
        }
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
        if category == CleanupCategory::Cache {
            ExecutionMode::FastPurge
        } else {
            ExecutionMode::RecycleBin
        }
    }

    fn can_fast_purge_path(&self, path: &str) -> bool {
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
        let category = Self::cleanup_category_for_path(path, kind);
        let risk = Self::cleanup_risk_for_path(path, category);
        category == CleanupCategory::Cache && risk == RiskLevel::Low
    }

    fn selected_cleanup_totals(&self, category: CleanupCategory) -> (usize, u64) {
        self.cleanup_items_for_category(category)
            .into_iter()
            .filter(|item| {
                item.risk != RiskLevel::High
                    && self
                        .cleanup
                        .selected_paths
                        .contains(item.target.path.as_str())
            })
            .fold((0usize, 0u64), |(count, bytes), item| {
                (count + 1, bytes.saturating_add(item.target.size_bytes))
            })
    }

    fn select_all_safe_cleanup_items(&mut self, category: CleanupCategory) {
        for item in self.cleanup_items_for_category(category) {
            if item.risk != RiskLevel::High {
                self.cleanup.selected_paths.insert(item.target.path);
            }
        }
    }

    fn clear_selected_cleanup_items(&mut self, category: CleanupCategory) {
        for item in self.cleanup_items_for_category(category) {
            self.cleanup
                .selected_paths
                .remove(item.target.path.as_str());
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
                        .contains(item.target.path.as_str())
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
                name: node.name.clone(),
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

    fn focus_treemap_path(&mut self, path: String) {
        self.treemap_focus_path = Some(path.clone());
        self.select_path(&path, SelectionSource::Treemap);
    }

    fn focus_treemap_parent(&mut self) {
        let Some(current) = self.treemap_focus_target() else {
            return;
        };
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let Some(node_id) = store.path_index.get(&current.path).copied() else {
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
        if self
            .treemap_focus_path
            .as_deref()
            .is_some_and(matches_target)
        {
            self.treemap_focus_path = None;
        }

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
        } else {
            self.summary.bytes_observed = self
                .summary
                .bytes_observed
                .saturating_sub(target.size_bytes);
            self.summary.scanned_files =
                self.summary.scanned_files.saturating_sub(target.file_count);
            self.summary.scanned_dirs = self.summary.scanned_dirs.saturating_sub(target.dir_count);
        }
        self.refresh_cleanup_analysis();
        self.selection = SelectionState::default();
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
                        target.path.clone(),
                        target.size_bytes,
                        self.risk_for_path(&target.path),
                    )
                })
                .collect(),
            self.selection_origin(),
        );
        let relay = Arc::new(Mutex::new(DeleteRelayState::new(&request, mode)));
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
            state.finished = Some(DeleteFinishedPayload { request, report });
            drop(state);
            ctx.request_repaint();
        });
    }

    fn delete_active(&self) -> bool {
        self.delete_session.is_some()
    }

    fn scan_finalizing(&self) -> bool {
        self.scan_finalize_session.is_some()
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
                        .any(|item| item.success && item.path == target.path)
                })
                .cloned()
                .collect();
            succeeded_targets.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
            for target in succeeded_targets {
                self.prune_deleted_target(&target);
            }
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
        self.completed_top_files = payload.top_files;
        self.completed_top_dirs = payload.top_dirs;
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

    fn set_maintenance_feedback(&mut self, message: String, success: bool) {
        self.maintenance_feedback = Some((message, success));
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
        self.pending_batch_events.clear();
        self.pending_snapshots.clear();
        self.live_files.clear();
        self.live_top_files.clear();
        self.live_top_dirs.clear();
        self.completed_top_files.clear();
        self.completed_top_dirs.clear();
        self.cleanup = CleanupPanelState::default();
        self.execution_report = None;
        self.explorer_feedback = None;
        self.selection = SelectionState::default();
        self.history.clear();
        self.errors.clear();
        self.selected_history_id = None;
        self.store = None;
        self.summary = ScanSummary::default();
        self.scan_current_path = None;
        self.scan_last_event_at = None;
        self.status = AppStatus::Idle;
        self.treemap_focus_path = None;
        self.diagnostics_json.clear();
        let trimmed = dirotter_platform::trim_process_memory();
        self.refresh_diagnostics();
        match trimmed {
            Ok(()) => self.set_maintenance_feedback(
                self.t(
                    "已清空当前结果并尝试释放 DirOtter 内存占用。",
                    "Cleared the current result and requested DirOtter memory trimming.",
                )
                .to_string(),
                true,
            ),
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t(
                        "已清空当前结果，但释放内存请求失败",
                        "Cleared current results, but memory trimming failed"
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
                    "已手动清理残留 staging 项。",
                    "Manually cleaned remaining staging items.",
                )
                .to_string(),
                true,
            ),
            Err(err) => self.set_maintenance_feedback(
                format!(
                    "{}: {}",
                    self.t("清理 staging 失败", "Failed to clean staging"),
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
                        top_files,
                        top_dirs,
                    } => {
                        state.finished = Some(FinishedPayload {
                            summary,
                            store,
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
            self.scan_current_path = Some(
                self.t("正在整理最终结果…", "Finalizing final results...")
                    .to_string(),
            );
            self.scan_last_event_at = Some(Instant::now());
            self.completed_top_files = finished.top_files.clone();
            self.completed_top_dirs = finished.top_dirs.clone();
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
                    top_files: finished.top_files,
                    top_dirs: finished.top_dirs,
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
        page_header(
            ui,
            self.t("DirOtter 工作台", "DirOtter Workspace"),
            self.t("磁盘概览", "Drive Overview"),
            self.t(
                "先看结论和动作，再进入扫描设置，以及最大的文件夹和文件。",
                "Start with the conclusion and action, then move into scan setup and the largest folders and files.",
            ),
        );
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
        if !self.scan_active() {
            self.render_overview_hero(ui);
        } else {
            self.render_live_overview_hero(ui);
        }
        ui.add_space(14.0);
        self.render_overview_metrics_strip(ui);
        ui.add_space(18.0);
        self.render_scan_target_card(ui);
        ui.add_space(18.0);
        let wide_layout = ui.available_width() >= 740.0;
        if wide_layout {
            let gap = 20.0;
            let total = ui.available_width();
            let left_width = (total - gap) / 2.0;
            let right_width = total - gap - left_width;
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
                        );
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
                        );
                    },
                );
            });
        } else {
            render_ranked_size_list(
                ui,
                &folders_title,
                &folders_subtitle,
                &ranked_dirs,
                self.summary.bytes_observed,
                &mut self.selection,
                &mut self.execution_report,
            );
            ui.add_space(18.0);
            render_ranked_size_list(
                ui,
                &files_title,
                &files_subtitle,
                &ranked_files,
                self.summary.bytes_observed,
                &mut self.selection,
                &mut self.execution_report,
            );
        }
    }

    fn render_overview_hero(&mut self, ui: &mut egui::Ui) {
        let analysis = self.cleanup.analysis.as_ref();
        let reclaimable = analysis.map(|a| a.reclaimable_bytes).unwrap_or(0);
        let quick_clean = analysis.map(|a| a.quick_clean_bytes).unwrap_or(0);
        let has_items = analysis.is_some_and(|analysis| !analysis.items.is_empty());
        let default_category =
            analysis.and_then(|analysis| analysis.categories.first().map(|entry| entry.category));
        let top_categories: Vec<_> = analysis
            .map(|analysis| {
                analysis
                    .categories
                    .iter()
                    .filter(|category| category.reclaimable_bytes > 0)
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let hero_label = if reclaimable > 0 {
            self.t("清理建议", "Cleanup Suggestions")
        } else if self.summary.bytes_observed > 0 {
            self.t("磁盘概览", "Drive Overview")
        } else {
            self.t("准备开始一次目录巡检", "Ready for a New Pass")
        };
        let hero_value = if reclaimable > 0 {
            format_bytes(reclaimable)
        } else if self.summary.bytes_observed > 0 {
            format_bytes(self.summary.bytes_observed)
        } else {
            self.t("先选一个盘符开始扫描。", "Pick a drive to begin scanning.")
                .to_string()
        };
        let hero_body = if reclaimable > 0 {
            self.t(
                "只统计通过规则筛选后的建议项，先告诉你哪里最值得处理。",
                "Only counts rule-based suggestions so the next action is obvious.",
            )
        } else if self.summary.bytes_observed > 0 {
            self.t(
                "如果当前还没有明确建议，就先从最大文件夹和最大文件开始处理。",
                "If there is no clear cleanup suggestion yet, start from the largest folders and files.",
            )
        } else {
            self.t(
                "从盘符按钮直接开始，或先调整根目录和扫描模式。",
                "Start from a drive button, or adjust the root path and scan mode first.",
            )
        };
        let current_scope = if self.root_input.trim().is_empty() {
            self.t("未设置", "Not set").to_string()
        } else {
            truncate_middle(&self.root_input, 44)
        };
        let scope_mode_title = self.t("当前范围与模式", "Current Scope & Mode").to_string();
        let root_label = self.t("根目录", "Root path").to_string();
        let root_subtitle = self.t("当前扫描目标", "Current scope").to_string();
        let mode_label = self.t("当前模式", "Current Mode").to_string();
        let mode_title = self.scan_mode_title(self.scan_mode).to_string();
        let mode_description = self.scan_mode_description(self.scan_mode).to_string();
        let no_suggestions_title = self.t("还没有建议项", "No Suggestions Yet").to_string();
        let no_suggestions_body = self
            .t(
                "继续完成一次扫描，或直接看下方的最大文件夹和最大文件。",
                "Finish a scan or move straight to the largest folders and files below.",
            )
            .to_string();
        let top_sources_label = self.t("主要来源", "Top Sources").to_string();
        let top_source_rows: Vec<_> = top_categories
            .iter()
            .map(|category| {
                (
                    self.cleanup_category_color(category.category),
                    self.cleanup_category_label(category.category).to_string(),
                    format_bytes(category.reclaimable_bytes),
                )
            })
            .collect();
        let wide_layout = ui.available_width() >= 760.0;

        dashboard_panel(ui, |ui| {
            ui.label(
                egui::RichText::new(hero_label)
                    .text_style(egui::TextStyle::Small)
                    .color(river_teal()),
            );
            ui.add_space(8.0);
            if wide_layout {
                let gap = 20.0;
                let total = ui.available_width();
                let left_width = (((total - gap) * 0.58).floor()).clamp(320.0, total - gap - 280.0);
                let right_width = total - gap - left_width;
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(left_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.label(
                                egui::RichText::new(hero_value)
                                    .size(if reclaimable > 0 || self.summary.bytes_observed > 0 {
                                        36.0
                                    } else {
                                        26.0
                                    })
                                    .strong(),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(hero_body)
                                    .text_style(egui::TextStyle::Small)
                                    .color(ui.visuals().weak_text_color()),
                            );
                            ui.add_space(14.0);
                            ui.horizontal_wrapped(|ui| {
                                if quick_clean > 0 {
                                    if ui
                                        .add_enabled_ui(!self.delete_active(), |ui| {
                                            sized_primary_button(
                                                ui,
                                                220.0,
                                                self.t(
                                                    "一键清理缓存（推荐）",
                                                    "Quick Cache Cleanup",
                                                ),
                                            )
                                        })
                                        .inner
                                        .clicked()
                                    {
                                        self.queue_cleanup_cache_delete();
                                    }
                                } else if ui
                                    .add_enabled_ui(!self.scan_active(), |ui| {
                                        sized_primary_button(
                                            ui,
                                            180.0,
                                            self.t("开始扫描", "Start Scan"),
                                        )
                                    })
                                    .inner
                                    .clicked()
                                {
                                    self.start_scan();
                                }

                                if ui
                                    .add_enabled_ui(has_items, |ui| {
                                        sized_button(
                                            ui,
                                            180.0,
                                            self.t("查看建议详情", "Review Suggestions"),
                                        )
                                    })
                                    .inner
                                    .clicked()
                                {
                                    self.cleanup.detail_category = default_category;
                                }
                            });
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        egui::vec2(right_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.label(egui::RichText::new(&scope_mode_title).strong());
                            ui.add_space(6.0);
                            stat_row(ui, &root_label, &current_scope, &root_subtitle);
                            ui.add_space(8.0);
                            stat_row(ui, &mode_label, &mode_title, &mode_description);
                            ui.add_space(10.0);
                            if top_source_rows.is_empty() {
                                empty_state_panel(ui, &no_suggestions_title, &no_suggestions_body);
                            } else {
                                ui.label(
                                    egui::RichText::new(&top_sources_label)
                                        .text_style(egui::TextStyle::Small)
                                        .color(ui.visuals().weak_text_color()),
                                );
                                ui.add_space(6.0);
                                for (color, label, value) in &top_source_rows {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(*color, "●");
                                        ui.label(egui::RichText::new(label).strong());
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(egui::RichText::new(value).strong());
                                            },
                                        );
                                    });
                                    ui.add_space(6.0);
                                }
                            }
                        },
                    );
                });
            } else {
                ui.label(
                    egui::RichText::new(hero_value)
                        .size(if reclaimable > 0 || self.summary.bytes_observed > 0 {
                            36.0
                        } else {
                            26.0
                        })
                        .strong(),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(hero_body)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(14.0);
                ui.horizontal_wrapped(|ui| {
                    if quick_clean > 0 {
                        if ui
                            .add_enabled_ui(!self.delete_active(), |ui| {
                                sized_primary_button(
                                    ui,
                                    220.0,
                                    self.t("一键清理缓存（推荐）", "Quick Cache Cleanup"),
                                )
                            })
                            .inner
                            .clicked()
                        {
                            self.queue_cleanup_cache_delete();
                        }
                    } else if ui
                        .add_enabled_ui(!self.scan_active(), |ui| {
                            sized_primary_button(ui, 180.0, self.t("开始扫描", "Start Scan"))
                        })
                        .inner
                        .clicked()
                    {
                        self.start_scan();
                    }
                    if ui
                        .add_enabled_ui(has_items, |ui| {
                            sized_button(ui, 180.0, self.t("查看建议详情", "Review Suggestions"))
                        })
                        .inner
                        .clicked()
                    {
                        self.cleanup.detail_category = default_category;
                    }
                });
                ui.add_space(14.0);
                stat_row(ui, &root_label, &current_scope, &root_subtitle);
                ui.add_space(8.0);
                stat_row(ui, &mode_label, &mode_title, &mode_description);
                ui.add_space(10.0);
                if top_source_rows.is_empty() {
                    empty_state_panel(ui, &no_suggestions_title, &no_suggestions_body);
                } else {
                    ui.label(
                        egui::RichText::new(&top_sources_label)
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(6.0);
                    for (color, label, value) in &top_source_rows {
                        ui.horizontal(|ui| {
                            ui.colored_label(*color, "●");
                            ui.label(egui::RichText::new(label).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new(value).strong());
                                },
                            );
                        });
                        ui.add_space(6.0);
                    }
                }
            }
        });
    }

    fn render_live_overview_hero(&mut self, ui: &mut egui::Ui) {
        let current_path = self
            .scan_current_path
            .as_deref()
            .map(|path| truncate_middle(path, 84))
            .unwrap_or_else(|| {
                self.t("正在准备扫描路径…", "Preparing scan path...")
                    .to_string()
            });
        let coverage_label = self
            .scanned_coverage_ratio()
            .map(|ratio| format!("{:.0}%", ratio * 100.0))
            .unwrap_or_else(|| self.t("估算中", "Estimating").to_string());
        dashboard_panel(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("实时总览", "Live Overview"))
                    .text_style(egui::TextStyle::Small)
                    .color(river_teal()),
            );
            ui.add_space(8.0);
            dashboard_split(
                ui,
                320.0,
                20.0,
                |ui| {
                    ui.label(
                        egui::RichText::new(format_bytes(self.summary.bytes_observed))
                            .size(36.0)
                            .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(self.t(
                            "扫描中首页只保留当前态势，不提前给最终结论。",
                            "While scanning, the overview stays focused on current state instead of premature conclusions.",
                        ))
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(12.0);
                    stat_row(
                        ui,
                        self.t("当前处理路径", "Current Path"),
                        &current_path,
                        self.scan_health_summary().as_str(),
                    );
                },
                |ui| {
                    ui.label(egui::RichText::new(self.t("扫描态势", "Scan Status")).strong());
                    ui.add_space(6.0);
                    stat_row(
                        ui,
                        self.t("扫描覆盖率", "Coverage"),
                        &coverage_label,
                        self.t("按卷容量估算", "Estimated against volume size"),
                    );
                    ui.add_space(8.0);
                    stat_row(
                        ui,
                        self.t("错误", "Errors"),
                        &format_count(self.summary.error_count),
                        self.t("当前已累计的问题项", "Issues accumulated so far"),
                    );
                    ui.add_space(8.0);
                    stat_row(
                        ui,
                        self.t("已观察体积", "Observed Bytes"),
                        &format_bytes(self.summary.bytes_observed),
                        self.t(
                            "这是实时增量状态，不是最终结论。",
                            "This is live incremental state, not the final conclusion.",
                        ),
                    );
                },
            );
        });
    }

    fn render_overview_metrics_strip(&self, ui: &mut egui::Ui) {
        let cards = if let Some((used, free, total)) = self.volume_numbers() {
            [
                (
                    self.t("磁盘已用", "Used"),
                    format_bytes(used),
                    format!(
                        "{} {}",
                        format_bytes(total),
                        self.t("总容量", "total capacity")
                    ),
                    river_teal(),
                ),
                (
                    self.t("磁盘可用", "Free"),
                    format_bytes(free),
                    self.t("当前卷剩余可用空间", "Remaining free space on this volume")
                        .to_string(),
                    info_blue(),
                ),
                (
                    self.t("已扫描体积", "Observed"),
                    format_bytes(self.summary.bytes_observed),
                    self.t(
                        "本次扫描已经确认的文件体积",
                        "File bytes already confirmed in this scan",
                    )
                    .to_string(),
                    success_green(),
                ),
                (
                    self.t("错误", "Errors"),
                    format_count(self.summary.error_count),
                    self.t("无法读取或被跳过的路径", "Unreadable or skipped paths")
                        .to_string(),
                    if self.summary.error_count > 0 {
                        danger_red()
                    } else {
                        egui::Color32::from_rgb(0x5F, 0x8D, 0x96)
                    },
                ),
            ]
        } else {
            [
                (
                    self.t("文件", "Files"),
                    format_count(self.summary.scanned_files),
                    self.t("已发现文件数", "Files discovered").to_string(),
                    river_teal(),
                ),
                (
                    self.t("目录", "Folders"),
                    format_count(self.summary.scanned_dirs),
                    self.t("已遍历目录数", "Folders traversed").to_string(),
                    info_blue(),
                ),
                (
                    self.t("已扫描体积", "Observed"),
                    format_bytes(self.summary.bytes_observed),
                    self.t(
                        "本次扫描已经确认的文件体积",
                        "File bytes already confirmed in this scan",
                    )
                    .to_string(),
                    success_green(),
                ),
                (
                    self.t("错误", "Errors"),
                    format_count(self.summary.error_count),
                    self.t("无法读取或被跳过的路径", "Unreadable or skipped paths")
                        .to_string(),
                    if self.summary.error_count > 0 {
                        danger_red()
                    } else {
                        egui::Color32::from_rgb(0x5F, 0x8D, 0x96)
                    },
                ),
            ]
        };
        if ui.available_width() >= 980.0 {
            dashboard_metric_row(ui, &cards);
        } else if ui.available_width() >= 620.0 {
            dashboard_metric_row(ui, &cards[..2]);
            ui.add_space(12.0);
            dashboard_metric_row(ui, &cards[2..]);
        } else {
            for card in cards {
                dashboard_metric_tile(ui, card.0, &card.1, &card.2, card.3);
                ui.add_space(12.0);
            }
        }
    }

    fn render_scan_target_card(&mut self, ui: &mut egui::Ui) {
        dashboard_panel(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("开始扫描", "Start Scan"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );

            ui.add_space(12.0);
            ui.label(egui::RichText::new(self.t("快速盘符", "Quick Drives")).strong());
            ui.label(
                egui::RichText::new(self.t(
                    "优先点击盘符直接开始；只有要扫子目录时再手动输入。",
                    "Start with a drive button first. Only type a manual path when you need a subfolder.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            if self.available_volumes.is_empty() {
                empty_state_panel(
                    ui,
                    self.t("没有检测到卷", "No Volumes Detected"),
                    self.t(
                        "仍可手动输入任意目录作为扫描目标。",
                        "You can still enter any folder manually as the scan target.",
                    ),
                );
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
                                sized_selectable(ui, 156.0, selected, label.clone())
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
            }

            ui.add_space(14.0);
            ui.label(
                egui::RichText::new(self.t("手动目录（可选）", "Manual path (optional)")).strong(),
            );
            ui.add_space(6.0);
            let root_hint = self
                .t("例如 D:\\Projects", "For example D:\\Projects")
                .to_string();
            ui.add_sized(
                [ui.available_width().min(420.0), CONTROL_HEIGHT],
                egui::TextEdit::singleline(&mut self.root_input)
                    .desired_width(420.0)
                    .hint_text(root_hint),
            );

            ui.add_space(14.0);
            ui.label(egui::RichText::new(self.t("扫描模式", "Scan Mode")).strong());
            ui.label(
                egui::RichText::new(self.t(
                    "三种模式都会完整扫描当前范围，差异只在扫描节奏和界面刷新方式。",
                    "All three modes scan the same scope. The only difference is pacing and UI update cadence.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            ui.add_enabled_ui(!self.scan_active(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for mode in [ScanMode::Quick, ScanMode::Deep, ScanMode::LargeDisk] {
                        let response = sized_selectable(
                            ui,
                            190.0,
                            self.scan_mode == mode,
                            self.scan_mode_title(mode),
                        )
                        .on_hover_text(self.scan_mode_description(mode));
                        if response.clicked() {
                            self.set_scan_mode(mode);
                        }
                    }
                });
            });
            ui.add_space(10.0);
            tone_banner(
                ui,
                self.scan_mode_title(self.scan_mode),
                self.scan_mode_description(self.scan_mode),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(self.scan_mode_note())
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );

            if let Some((used, free, _)) = self.volume_numbers() {
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    compact_stat_chip(ui, self.t("磁盘已用", "Used"), &format_bytes(used));
                    compact_stat_chip(ui, self.t("磁盘可用", "Free"), &format_bytes(free));
                    if let Some(ratio) = self.scanned_coverage_ratio() {
                        compact_stat_chip(
                            ui,
                            self.t("扫描覆盖率", "Coverage"),
                            &format!("{:.0}%", ratio * 100.0),
                        );
                    }
                    compact_stat_chip(
                        ui,
                        self.t("文件", "Files"),
                        &format_count(self.summary.scanned_files),
                    );
                });
            }

            ui.add_space(14.0);
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

    fn ui_current_scan(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("DirOtter 工作台", "DirOtter Workspace"),
            self.t("实时扫描", "Live Scan"),
            self.t(
                "这里展示的是“扫描中已发现的最大项”，不是最终结果。内部性能指标已移到诊断页。",
                "This page shows the largest items discovered so far, not the final result. Internal performance counters have been moved to Diagnostics.",
            ),
        );
        ui.add_space(8.0);
        if self.scan_finalizing() {
            tone_banner(
                ui,
                self.t("正在整理最终结果", "Finalizing Final Results"),
                self.t(
                    "目录遍历已经结束，当前正在后台保存快照、写入历史并生成清理建议。界面应保持可操作，完成后会自动切到正常完成态。",
                    "Directory traversal has finished. DirOtter is now saving the snapshot, writing history, and preparing cleanup suggestions in the background. The UI should stay responsive and will switch to the normal completed state automatically.",
                ),
            );
            ui.add_space(10.0);
        } else if self.scan_active() {
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
        surface_panel(ui, |ui| {
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

    fn ui_treemap(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("DirOtter 工作台", "DirOtter Workspace"),
            self.t("结果视图", "Result View"),
            self.t(
                "这里只展示扫描完成后的结果，不跟实时扫描绑定。每次只看当前目录的直接子项，再按需逐层进入。",
                "This page only shows completed scan results. It is not bound to live scanning. Inspect one directory level at a time and drill in only when needed.",
            ),
        );
        ui.add_space(8.0);

        if self.scan_active() {
            tone_banner(
                ui,
                self.t("Treemap 不参与实时刷新", "Treemap Stays Out of Live Updates"),
                self.t(
                    "扫描完成后再生成结果视图，避免 UI 线程、海量节点和布局开销叠加卡顿。",
                    "The result view is generated only after scan completion, avoiding UI thread churn, huge node counts, and layout overhead piling up together.",
                ),
            );
            return;
        }

        let Some(scope) = self.treemap_focus_target() else {
            tone_banner(
                ui,
                self.t("还没有可用结果", "No Completed Result Yet"),
                self.t(
                    "先完成一次扫描，或等待缓存快照加载后，再使用这个结果视图。",
                    "Complete a scan first, or wait until a cached snapshot is loaded before using this result view.",
                ),
            );
            return;
        };

        let entries = self.treemap_entries(&scope.path, MAX_TREEMAP_CHILDREN);
        let selected_dir = self.selected_directory_target();
        let root_target = self
            .root_node_id()
            .and_then(|node_id| self.target_from_node_id(node_id));
        let scope_total = scope.size_bytes.max(1);

        tone_banner(
            ui,
            self.t("轻量结果视图", "Lightweight Result View"),
            &format!(
                "{} {}\n{}",
                self.t("当前目录：", "Current directory:"),
                truncate_middle(&scope.path, 88),
                self.t(
                    "只展示直接子项，不递归整树，不做实时布局。",
                    "Only direct children are shown. No whole-tree recursion and no live layout work.",
                )
            ),
        );
        ui.add_space(10.0);

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.treemap_focus_path.is_some(),
                    egui::Button::new(self.t("返回上级", "Up One Level")),
                )
                .clicked()
            {
                self.focus_treemap_parent();
            }

            if let Some(root) = root_target.clone() {
                if scope.path != root.path
                    && ui.button(self.t("回到根目录", "Back to Root")).clicked()
                {
                    self.treemap_focus_path = None;
                    self.select_path(&root.path, SelectionSource::Treemap);
                }
            }

            if let Some(target) = selected_dir {
                if target.path != scope.path
                    && ui
                        .button(self.t("跳到当前选中目录", "Use Selected Directory"))
                        .clicked()
                {
                    self.focus_treemap_path(target.path);
                }
            }
        });

        ui.add_space(10.0);
        ui.columns(3, |columns| {
            compact_metric_block(
                &mut columns[0],
                self.t("当前层级体积", "Current Level Size"),
                &format_bytes(scope.size_bytes),
                self.t("作为本层占比基准", "Used as the local baseline"),
            );
            compact_metric_block(
                &mut columns[1],
                self.t("直接子项", "Direct Children"),
                &format_count(entries.len() as u64),
                self.t("只统计当前目录下一层", "Current directory only"),
            );
            compact_metric_block(
                &mut columns[2],
                self.t("显示上限", "Display Cap"),
                &format_count(MAX_TREEMAP_CHILDREN as u64),
                self.t("避免大目录压垮结果视图", "Keeps large folders responsive"),
            );
        });

        ui.add_space(12.0);
        if entries.is_empty() {
            tone_banner(
                ui,
                self.t("这一层没有可展示的子项", "No Children to Show at This Level"),
                self.t(
                    "当前目录可能为空，或缓存结果里还没有可用子节点。",
                    "This directory may be empty, or the cached result does not currently have child nodes for it.",
                ),
            );
            return;
        }

        let panel_height = ui.available_height().max(320.0);
        surface_panel(ui, |ui| {
            ui.set_min_height(panel_height);
            ui.label(
                egui::RichText::new(self.t("目录结果条形图", "Directory Result Bars"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.label(
                egui::RichText::new(self.t(
                    "点击条目可联动 Inspector；目录可继续进入下一层。",
                    "Click an item to sync Inspector. Directories can drill into the next level.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);

            let list_height = (panel_height - 84.0).max(220.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), list_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show_rows(ui, 96.0, entries.len(), |ui, row_range| {
                            for row in row_range {
                                let Some(entry) = entries.get(row) else {
                                    continue;
                                };
                                let share =
                                    (entry.size_bytes as f32 / scope_total as f32).clamp(0.0, 1.0);
                                let selected = self.selection.selected_path.as_deref()
                                    == Some(entry.path.as_str());
                                let label = format!(
                                    "{} {}",
                                    if matches!(entry.kind, NodeKind::Dir) {
                                        self.t("目录", "DIR")
                                    } else {
                                        self.t("文件", "FILE")
                                    },
                                    truncate_middle(&entry.name, 56)
                                );
                                let subtitle = match entry.kind {
                                    NodeKind::Dir => format!(
                                        "{} {}  |  {} {}",
                                        format_count(entry.file_count),
                                        self.t("文件", "files"),
                                        format_count(entry.dir_count.saturating_sub(1)),
                                        self.t("子目录", "subdirs")
                                    ),
                                    NodeKind::File => self.t("文件项", "File item").to_string(),
                                };

                                surface_panel(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add_sized(
                                                [
                                                    (ui.available_width() - 220.0).max(160.0),
                                                    CONTROL_HEIGHT,
                                                ],
                                                egui::SelectableLabel::new(selected, label.clone()),
                                            )
                                            .clicked()
                                        {
                                            self.select_path(&entry.path, SelectionSource::Treemap);
                                        }
                                        if matches!(entry.kind, NodeKind::Dir)
                                            && ui
                                                .button(self.t("进入下一层", "Open Level"))
                                                .clicked()
                                        {
                                            self.focus_treemap_path(entry.path.clone());
                                        }
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(format_bytes(entry.size_bytes));
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                    ui.add(
                                        egui::ProgressBar::new(share)
                                            .desired_width(ui.available_width())
                                            .fill(if matches!(entry.kind, NodeKind::Dir) {
                                                river_teal()
                                            } else {
                                                info_blue()
                                            })
                                            .text(format!("{:.1}%", share * 100.0)),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(subtitle)
                                            .text_style(egui::TextStyle::Small)
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                });
                                ui.add_space(8.0);
                            }
                        });
                },
            );
        });
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("DirOtter 工作台", "DirOtter Workspace"),
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
            surface_panel(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.t("快照详情", "Snapshot Detail"))
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                ui.add_space(8.0);
                stat_row(
                    ui,
                    self.t("编号", "ID"),
                    &h.id.to_string(),
                    &truncate_middle(&h.root, 44),
                );
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
            self.t("DirOtter 工作台", "DirOtter Workspace"),
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
                self.t("用户", "User"),
                &format_count(user as u64),
                self.t("用户输入或权限问题", "Input or permission issues"),
                warning_amber(),
            );
            metric_card(
                &mut columns[1],
                self.t("瞬时", "Transient"),
                &format_count(transient as u64),
                self.t("可重试的瞬时失败", "Retryable transient failures"),
                info_blue(),
            );
            metric_card(
                &mut columns[2],
                self.t("系统", "System"),
                &format_count(system as u64),
                self.t("系统级故障", "System-level failures"),
                danger_red(),
            );
        });

        let filter_label = self.t("全部", "All").to_string();
        let user_filter_label = self.t("用户", "User").to_string();
        let transient_filter_label = self.t("瞬时", "Transient").to_string();
        let system_filter_label = self.t("系统", "System").to_string();
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(self.t("过滤", "Filter"));
            ui.selectable_value(&mut self.error_filter, ErrorFilter::All, filter_label);
            ui.selectable_value(&mut self.error_filter, ErrorFilter::User, user_filter_label);
            ui.selectable_value(
                &mut self.error_filter,
                ErrorFilter::Transient,
                transient_filter_label,
            );
            ui.selectable_value(
                &mut self.error_filter,
                ErrorFilter::System,
                system_filter_label,
            );
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
        let list_height = ui.available_height().max(240.0);
        surface_panel(ui, |ui| {
            ui.set_min_height(list_height);
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for e in &filtered {
                        let is_selected = self.selection.selected_path.as_deref() == Some(&e.path);
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let mut frame = surface_frame(ui);
                                if is_selected {
                                    frame = frame.stroke(egui::Stroke::new(1.5, river_teal()));
                                }
                                show_frame_with_relaxed_clip(ui, frame, |ui| {
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 24.0],
                                            egui::SelectableLabel::new(
                                                is_selected,
                                                format!(
                                                    "[{:?}] {}",
                                                    e.kind,
                                                    truncate_middle(&e.path, 68)
                                                ),
                                            ),
                                        )
                                        .clicked()
                                    {
                                        self.select_path(&e.path, SelectionSource::Error);
                                    }
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        if ui.button(self.t("选中查看", "Inspect")).clicked() {
                                            self.select_path(&e.path, SelectionSource::Error);
                                        }
                                    });
                                    ui.add_space(6.0);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&e.reason)
                                                .text_style(egui::TextStyle::Small)
                                                .color(ui.visuals().weak_text_color()),
                                        )
                                        .wrap(),
                                    );
                                });
                            },
                        );
                        ui.add_space(8.0);
                    }
                });
        });
    }

    fn ui_diagnostics(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("DirOtter 工作台", "DirOtter Workspace"),
            self.t("诊断导出", "Diagnostics"),
            self.t(
                "保留结构化 JSON，但给导出动作更明确的位置和说明。",
                "Keep the structured JSON, but surface export actions and explanation more clearly.",
            ),
        );
        ui.add_space(8.0);
        let mut refresh_diag = false;
        let mut export_bundle = false;
        let mut save_snapshot = false;
        let mut record_history = false;
        let mut export_error_csv = false;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(self.t("刷新诊断", "Refresh diagnostics"))
                .clicked()
            {
                refresh_diag = true;
            }
            if ui
                .button(self.t("导出诊断包", "Export diagnostics bundle"))
                .clicked()
            {
                export_bundle = true;
            }
            if ui
                .add_enabled_ui(self.store.is_some(), |ui| {
                    sized_button(
                        ui,
                        170.0,
                        self.t("手动保存当前快照", "Save Current Snapshot"),
                    )
                })
                .inner
                .clicked()
            {
                save_snapshot = true;
            }
            if ui
                .add_enabled_ui(self.summary.bytes_observed > 0, |ui| {
                    sized_button(ui, 188.0, self.t("手动记录扫描摘要", "Record Scan Summary"))
                })
                .inner
                .clicked()
            {
                record_history = true;
            }
            if ui
                .button(self.t("手动导出错误 CSV", "Export Errors CSV"))
                .clicked()
            {
                export_error_csv = true;
            }
        });
        if let Some((message, success)) = self.maintenance_feedback.as_ref() {
            ui.add_space(8.0);
            tone_banner(
                ui,
                if *success {
                    self.t("已完成", "Done")
                } else {
                    self.t("操作失败", "Action Failed")
                },
                message,
            );
        }
        if refresh_diag {
            self.refresh_diagnostics();
        }
        if export_bundle {
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
            self.set_maintenance_feedback(
                self.t("已导出诊断包。", "Exported the diagnostics bundle.")
                    .to_string(),
                true,
            );
        }
        if save_snapshot {
            self.save_current_snapshot_manually();
        }
        if record_history {
            self.record_current_history_manually();
        }
        if export_error_csv {
            self.export_errors_csv_manually();
        }
        ui.separator();
        let panel_width = ui.available_width();
        let viewport_height = ui.ctx().input(|i| i.screen_rect().height());
        let editor_height =
            (viewport_height - TOOLBAR_HEIGHT - STATUSBAR_HEIGHT - 220.0).max(420.0);
        surface_panel(ui, |ui| {
            ui.set_min_width(panel_width);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), editor_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.add_sized(
                                [ui.available_width().max(320.0), editor_height],
                                egui::TextEdit::multiline(&mut self.diagnostics_json)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .code_editor()
                                    .interactive(false),
                            );
                        });
                },
            );
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        page_header(
            ui,
            self.t("DirOtter 工作台", "DirOtter Workspace"),
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
        ui.add_space(14.0);
        settings_section(
            ui,
            self.t("常用设置", "Common Settings"),
            self.t(
                "主流设置页会把高频项放在最上面，并保持分组稳定、可预期。",
                "Mainstream settings pages place high-frequency controls first and keep groups stable and predictable.",
            ),
            |ui| {
                settings_row(
                    ui,
                    self.t("界面语言", "Interface Language"),
                    self.t(
                        "手动选择会覆盖系统语言检测。",
                        "Manual selection overrides automatic locale detection.",
                    ),
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for lang in [Lang::Zh, Lang::En, Lang::Fr, Lang::Es] {
                                if ui
                                    .add_sized(
                                        [110.0, CONTROL_HEIGHT],
                                        egui::SelectableLabel::new(
                                            self.language == lang,
                                            lang_native_label(lang),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.set_language(lang);
                                }
                            }
                        });
                    },
                );
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);
                settings_row(
                    ui,
                    self.t("界面主题", "Interface Theme"),
                    self.t(
                        "深色更适合长时间分析；浅色则保持低对比和柔和明度。",
                        "Dark is better for long analysis sessions; light stays restrained and low contrast.",
                    ),
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_sized(
                                    [132.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(
                                        !self.theme_dark,
                                        self.t("浅色", "Light"),
                                    ),
                                )
                                .clicked()
                            {
                                self.theme_dark = false;
                                self.apply_theme(ctx);
                                let _ = self.cache.set_setting("theme", "light");
                            }
                            if ui
                                .add_sized(
                                    [132.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(
                                        self.theme_dark,
                                        self.t("深色", "Dark"),
                                    ),
                                )
                                .clicked()
                            {
                                self.theme_dark = true;
                                self.apply_theme(ctx);
                                let _ = self.cache.set_setting("theme", "dark");
                            }
                        });
                    },
                );
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);
                settings_row(
                    ui,
                    self.t("高级工具", "Advanced Tools"),
                    self.t(
                        "把历史、错误和诊断页面收进二级入口。普通清理流程默认不需要它们。",
                        "Keeps history, errors, and diagnostics behind a secondary entry. Most cleanup flows do not need them by default.",
                    ),
                    |ui| {
                        let button_width = 168.0;
                        if ui
                            .add_sized(
                                [button_width, CONTROL_HEIGHT],
                                egui::SelectableLabel::new(
                                    self.advanced_tools_enabled,
                                    if self.advanced_tools_enabled {
                                        self.t("已开启", "Enabled")
                                    } else {
                                        self.t("已隐藏", "Hidden")
                                    },
                                ),
                            )
                            .clicked()
                        {
                            self.set_advanced_tools_enabled(!self.advanced_tools_enabled);
                        }
                    },
                );
            },
        );

        ui.add_space(14.0);
        settings_section(
            ui,
            self.t("视觉方向", "Visual Direction"),
            self.t(
                "这一组只保留品牌语义和当前状态，不把说明文字拆成零散卡片。",
                "This section keeps brand semantics and current state together instead of splitting them into disconnected cards.",
            ),
            |ui| {
                color_note_row(
                    ui,
                    river_teal(),
                    self.t("River Teal", "River Teal"),
                    self.t(
                        "主品牌色，用于主按钮、选中与重点数据。",
                        "Primary brand accent for key actions, selection, and emphasis.",
                    ),
                );
                ui.add_space(10.0);
                color_note_row(
                    ui,
                    if self.theme_dark {
                        egui::Color32::from_rgb(0x18, 0x22, 0x27)
                    } else {
                        egui::Color32::from_rgb(0xEE, 0xF1, 0xF0)
                    },
                    self.t("基础面板", "Base Surfaces"),
                    self.t(
                        "保持低对比、长时间查看不刺眼。",
                        "Kept low-contrast so long sessions stay easy on the eyes.",
                    ),
                );
                ui.add_space(10.0);
                color_note_row(
                    ui,
                    sand_accent(),
                    self.t("暖色辅助", "Warm Accent"),
                    self.t(
                        "只做轻微平衡，不大面积出现。",
                        "Used sparingly to soften the palette, not dominate it.",
                    ),
                );
                ui.add_space(14.0);
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
            },
        );

        ui.add_space(14.0);
        settings_section(
            ui,
            self.t("本地化说明", "Localization Notes"),
            self.t(
                "把与语言相关的规则放在一起，减少用户在不同卡片间来回找解释。",
                "Keep language-related rules together so people do not have to hunt across separate cards.",
            ),
            |ui| {
                ui.label(self.t(
                    "应用会优先加载系统中的中文字体回退（Windows 优先 Microsoft YaHei / DengXian），避免中文标题和设置项显示为方框。",
                    "The app now prefers CJK-capable system fallback fonts (Windows prioritizes Microsoft YaHei / DengXian) so Chinese labels do not render as tofu boxes.",
                ));
                ui.add_space(8.0);
                ui.label(self.t(
                    "首次启动仍可根据系统语言环境推断中英文，但这里的手动选择会覆盖自动检测结果。",
                    "The first launch can still infer language from the system locale, but the manual choice here overrides auto-detection.",
                ));
            },
        );

        ui.add_space(14.0);
        settings_section(
            ui,
            self.t("品牌含义", "Why DirOtter"),
            self.t(
                "把品牌语义单独留成一个说明章节，而不是塞进控制区旁边。",
                "Keep brand meaning in its own explanatory section instead of squeezing it beside controls.",
            ),
            |ui| {
                ui.label(self.t(
                    "Dir 指 directory，Otter 借用水獭聪明、灵活、善于整理的联想。它更像一个冷静探索存储结构的分析工具，而不是只会“清理垃圾”的工具。",
                    "Dir points to directories, while Otter brings a clever, tidy, exploratory character. The product should feel like a calm storage analyzer, not a noisy junk cleaner.",
                ));
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
                    "尚未选择任何文件或目录。可以从实时列表、结果视图或其他页面点选对象。",
                    "No file or folder is selected yet. Pick one from the live list, result view, or another page.",
                ));
            }
        });

        if let Some(session) = self.delete_session.as_ref() {
            let snapshot = session.snapshot();
            ui.add_space(10.0);
            surface_panel(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(match snapshot.mode {
                            ExecutionMode::RecycleBin => {
                                self.t("后台任务：移到回收站", "Background Task: Recycle Bin")
                            }
                            ExecutionMode::FastPurge => {
                                self.t("后台任务：快速清理", "Background Task: Fast Cleanup")
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
                    &truncate_middle(&snapshot.label, 34),
                    &format!(
                        "{} {}",
                        format_count(snapshot.target_count as u64),
                        self.t("个项目正在执行", "items in flight")
                    ),
                );
                stat_row(
                    ui,
                    self.t("已耗时", "Elapsed"),
                    &format!("{:.1}s", snapshot.started_at.elapsed().as_secs_f32()),
                    match snapshot.mode {
                        ExecutionMode::RecycleBin => self.t("回收站删除", "Recycle-bin delete"),
                        ExecutionMode::FastPurge => {
                            self.t("秒移走后后台清除", "Instant move, background purge")
                        }
                        ExecutionMode::Permanent => self.t("永久删除", "Permanent delete"),
                    },
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
            let can_fast_purge_selection = selected_target
                .as_ref()
                .map(|target| self.can_fast_purge_path(&target.path))
                .unwrap_or(false);
            ui.vertical(|ui| {
                if ui
                    .add_enabled_ui(has_selection, |ui| {
                        sized_button(
                            ui,
                            ui.available_width(),
                            self.t("打开所在位置", "Open File Location"),
                        )
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
                    .add_enabled_ui(
                        has_selection && can_fast_purge_selection && !delete_active,
                        |ui| {
                            sized_primary_button(
                                ui,
                                ui.available_width(),
                                self.t("快速清理缓存", "Fast Cleanup"),
                            )
                        },
                    )
                    .inner
                    .clicked()
                {
                    if let Some(target) = selected_target.clone() {
                        self.queue_delete_for_target(target, ExecutionMode::FastPurge);
                    }
                }
                if ui
                    .add_enabled_ui(has_selection && !delete_active, |ui| {
                        sized_button(
                            ui,
                            ui.available_width(),
                            self.t("移到回收站", "Move to Recycle Bin"),
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.execute_selected_delete(ExecutionMode::RecycleBin);
                }
                let permanent =
                    egui::Button::new(self.t("永久删除", "Delete Permanently")).fill(danger_red());
                if ui
                    .add_enabled_ui(has_selection && !delete_active, |ui| {
                        ui.add_sized([ui.available_width(), CONTROL_HEIGHT], permanent)
                    })
                    .inner
                    .clicked()
                {
                    if let Some(target) = selected_target.clone() {
                        self.pending_delete_confirmation = Some(PendingDeleteConfirmation {
                            request: Self::delete_request_for_target(target.clone()),
                            risk: self.risk_for_path(&target.path),
                        });
                    }
                }
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                if ui
                    .add_enabled_ui(!self.scan_active() && !delete_active, |ui| {
                        sized_button(
                            ui,
                            ui.available_width(),
                            self.t("释放 DirOtter 内存", "Release DirOtter Memory"),
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.release_dir_otter_memory();
                }
                if ui
                    .add_enabled_ui(!self.scan_active() && !delete_active, |ui| {
                        sized_button(
                            ui,
                            ui.available_width(),
                            self.t("清理残留 staging", "Clean Up Staging"),
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.purge_staging_manually();
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
                        "先从列表、结果视图或其他页面里选中一个文件或文件夹。",
                        "Select a file or folder from a list, result view, or another page first.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
            }
            if let Some((message, success)) = self.explorer_feedback.as_ref() {
                ui.add_space(8.0);
                if *success {
                    tone_banner(ui, self.t("已打开所在位置", "Opened Location"), message);
                } else {
                    tone_banner(ui, self.t("打开位置失败", "Open Location Failed"), message);
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
                        ExecutionMode::FastPurge => self.t("快速清理缓存", "Fast cleanup"),
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
        surface_panel(ui, |ui| {
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
        let mut confirmed_delete: Option<DeleteRequestScope> = None;
        egui::Window::new(self.t("确认永久删除", "Confirm Permanent Delete"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                let target = pending
                    .request
                    .targets
                    .first()
                    .expect("single target delete confirm");
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
                    &truncate_middle(&target.path, 42),
                    match target.kind {
                        NodeKind::Dir => self.t("目录", "Directory"),
                        NodeKind::File => self.t("文件", "File"),
                    },
                );
                stat_row(
                    ui,
                    self.t("大小", "Size"),
                    &format_bytes(target.size_bytes),
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
        let mut keep_open = true;
        let mut request_close = false;
        let mut trigger_clean = false;
        let mut trigger_recycle = false;
        let mut trigger_permanent = false;
        let mut select_all_safe = false;
        let mut clear_selected = false;
        let mut open_selected = false;
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
                        egui::RichText::new(self.t("按分类检查后再决定清理范围。", "Review by category before deciding what to clean."))
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(self.t("关闭", "Close")).clicked() {
                            request_close = true;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    let categories = self
                        .cleanup
                        .analysis
                        .as_ref()
                        .map(|analysis| analysis.categories.clone())
                        .unwrap_or_default();
                    for entry in categories {
                        let selected = self.cleanup.detail_category == Some(entry.category);
                        let label = format!(
                            "{}  {}",
                            self.cleanup_category_label(entry.category),
                            format_bytes(entry.total_bytes)
                        );
                        if sized_selectable(ui, 150.0, selected, label).clicked() {
                            self.cleanup.detail_category = Some(entry.category);
                        }
                    }
                });
                ui.add_space(10.0);
                tone_banner(
                    ui,
                    self.cleanup_category_label(category),
                    self.t(
                        "绿色会默认勾选，黄色默认不勾选；红色项请点击条目后用“打开所选位置”自行确认处理。",
                        "Safe items are selected by default and warning items stay unchecked. For red items, click the row and use Open Selected Location for manual review.",
                    ),
                );
                ui.add_space(10.0);
                let (selected_count, selected_bytes) = self.selected_cleanup_totals(category);
                ui.horizontal_wrapped(|ui| {
                    compact_stat_chip(
                        ui,
                        self.t("已选项目", "Selected"),
                        &format_count(selected_count as u64),
                    );
                    compact_stat_chip(
                        ui,
                        self.t("预计释放", "Estimated Reclaim"),
                        &format_bytes(selected_bytes),
                    );
                    if ui
                        .add_enabled_ui(!self.delete_active(), |ui| {
                            sized_button(ui, 124.0, self.t("全选安全项", "Select Safe"))
                        })
                        .inner
                        .clicked()
                    {
                        select_all_safe = true;
                    }
                    if ui
                        .add_enabled_ui(!self.delete_active(), |ui| {
                            sized_button(ui, 118.0, self.t("清空所选", "Clear Selected"))
                        })
                        .inner
                        .clicked()
                    {
                        clear_selected = true;
                    }
                    if ui
                        .add_enabled_ui(self.selected_target().is_some(), |ui| {
                            sized_button(ui, 124.0, self.t("打开所选位置", "Open Selected"))
                        })
                        .inner
                        .clicked()
                    {
                        open_selected = true;
                    }
                    if ui
                        .add_enabled_ui(selected_count > 0 && !self.delete_active(), |ui| {
                            sized_button(
                                ui,
                                176.0,
                                if category == CleanupCategory::Cache {
                                    self.t("快速清理选中缓存", "Fast Cleanup Selected")
                                } else {
                                    self.t("移到回收站", "Move to Recycle Bin")
                                },
                            )
                        })
                        .inner
                        .clicked()
                    {
                        if category == CleanupCategory::Cache {
                            trigger_clean = true;
                        } else {
                            trigger_recycle = true;
                        }
                    }
                    if ui
                        .add_enabled_ui(selected_count > 0 && !self.delete_active(), |ui| {
                            let button = egui::Button::new(
                                self.t("永久删除", "Delete Permanently"),
                            )
                            .fill(danger_red());
                            ui.add_sized([156.0, CONTROL_HEIGHT], button)
                        })
                        .inner
                        .clicked()
                    {
                        trigger_permanent = true;
                    }
                });
                ui.add_space(10.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for item in &items {
                        surface_panel(ui, |ui| {
                            let size_width = 104.0;
                            let path_width = (ui.available_width() - size_width - 42.0).max(220.0);
                            ui.horizontal(|ui| {
                                let mut checked =
                                    self.cleanup.selected_paths.contains(item.target.path.as_str());
                                let enabled = item.risk != RiskLevel::High;
                                if ui
                                    .add_enabled_ui(enabled, |ui| ui.checkbox(&mut checked, ""))
                                    .inner
                                    .changed()
                                {
                                    if checked {
                                        self.cleanup
                                            .selected_paths
                                            .insert(item.target.path.clone());
                                    } else {
                                        self.cleanup.selected_paths.remove(&item.target.path);
                                    }
                                }
                                if ui
                                    .add_sized(
                                        [path_width, 22.0],
                                        egui::SelectableLabel::new(
                                            self.selection.selected_path.as_deref()
                                                == Some(item.target.path.as_str()),
                                            truncate_middle(&item.target.path, 72),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.select_path(&item.target.path, SelectionSource::Table);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                    ui.add_sized(
                                        [size_width, 20.0],
                                        egui::Label::new(
                                            egui::RichText::new(format_bytes(item.target.size_bytes))
                                                .strong(),
                                        ),
                                        );
                                    },
                                );
                            });
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(self.cleanup_risk_color(item.risk), "●");
                                ui.label(self.cleanup_risk_label(item.risk));
                                ui.label("·");
                                ui.label(self.cleanup_category_label(item.category));
                                if let Some(unused_days) = item.unused_days {
                                    ui.label("·");
                                    ui.label(format!(
                                        "{} {}",
                                        unused_days,
                                        self.t("天未使用", "days unused")
                                    ));
                                }
                                ui.label("·");
                                ui.label(format!(
                                    "{} {:.1}",
                                    self.t("评分", "Score"),
                                    item.cleanup_score
                                ));
                            });
                            ui.label(
                                egui::RichText::new(self.cleanup_reason_text(item))
                                    .text_style(egui::TextStyle::Small)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    }
                });

                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                            .add_enabled_ui(selected_count > 0 && !self.delete_active(), |ui| {
                                sized_primary_button(
                                    ui,
                                    220.0,
                                    if category == CleanupCategory::Cache {
                                        self.t("快速清理选中缓存", "Fast Cleanup Selected")
                                    } else {
                                        self.t("清理选中项", "Clean Selected")
                                    },
                                )
                            })
                            .inner
                        .clicked()
                    {
                        trigger_clean = true;
                    }
                    if ui.button(self.t("关闭", "Close")).clicked() {
                        request_close = true;
                    }
                });
            });

        if request_close {
            keep_open = false;
        }
        if select_all_safe {
            self.select_all_safe_cleanup_items(category);
        }
        if clear_selected {
            self.clear_selected_cleanup_items(category);
        }
        if open_selected {
            if let Some(target) = self.selected_target() {
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
        if !keep_open {
            self.cleanup.detail_category = None;
        }
        if trigger_clean {
            self.queue_cleanup_category_delete(category);
        }
        if trigger_recycle {
            self.queue_cleanup_category_delete_with_mode(category, ExecutionMode::RecycleBin);
        }
        if trigger_permanent {
            self.queue_cleanup_category_delete_with_mode(category, ExecutionMode::Permanent);
        }
    }

    fn ui_cleanup_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.cleanup.pending_delete.clone() else {
            return;
        };

        let mut keep_open = true;
        let mut confirmed = false;
        egui::Window::new(self.t("一键清理确认", "Confirm Cleanup"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                let is_fast_cleanup = request.mode == ExecutionMode::FastPurge;
                ui.label(
                    egui::RichText::new(self.t(
                        if is_fast_cleanup {
                            "将先把建议项快速移出当前目录，再在后台继续释放空间。"
                        } else {
                            "将优先把建议项移到回收站，避免直接永久删除。"
                        },
                        if is_fast_cleanup {
                            "Suggested items will be moved out of the current view first, then reclaimed in the background."
                        } else {
                            "Suggested items will be moved to the recycle bin first instead of being deleted permanently."
                        },
                    ))
                    .strong(),
                );
                ui.add_space(10.0);
                stat_row(
                    ui,
                    self.t("任务", "Task"),
                    &request.label,
                    self.t("规则驱动清理", "Rule-driven cleanup"),
                );
                stat_row(
                    ui,
                    self.t("项目数", "Items"),
                    &format_count(request.targets.len() as u64),
                    if is_fast_cleanup {
                        self.t("会先进入后台清理区", "Will be staged for background cleanup")
                    } else {
                        self.t("将进入系统回收站", "Will move to the system recycle bin")
                    },
                );
                stat_row(
                    ui,
                    self.t("预计释放", "Estimated Reclaim"),
                    &format_bytes(request.estimated_bytes),
                    if is_fast_cleanup {
                        self.t("磁盘空间会在后台逐步释放", "Disk space will continue to be reclaimed in the background")
                    } else {
                        self.t("实际释放量取决于系统删除结果", "Actual reclaim depends on execution results")
                    },
                );
                ui.add_space(8.0);
                for target in request.targets.iter().take(6) {
                    ui.label(format!(
                        "• {}  {}",
                        truncate_middle(&target.path, 52),
                        format_bytes(target.size_bytes)
                    ));
                }
                if request.targets.len() > 6 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            format_count((request.targets.len() - 6) as u64),
                            self.t("项未展开显示", "more items not shown")
                        ))
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                    );
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(!self.delete_active(), |ui| {
                            sized_primary_button(
                                ui,
                                150.0,
                                if is_fast_cleanup {
                                    self.t("立即清理", "Clean Now")
                                } else {
                                    self.t("移到回收站", "Move to Recycle Bin")
                                },
                            )
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
        self.process_scan_events();
        self.process_scan_finalize_events();
        self.process_delete_events();
        self.process_queued_delete();
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
    items: &[(String, u64)],
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

    detect_lang_from_locale(&locale)
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
    fn locale_detection_supports_french_and_spanish() {
        assert_eq!(detect_lang_from_locale("fr_FR"), Lang::Fr);
        assert_eq!(detect_lang_from_locale("es_ES.UTF-8"), Lang::Es);
        assert_eq!(detect_lang_from_locale("zh_CN"), Lang::Zh);
        assert_eq!(detect_lang_from_locale("en_US"), Lang::En);
    }

    #[test]
    fn language_settings_round_trip_for_all_supported_languages() {
        assert_eq!(parse_lang_setting("en"), Some(Lang::En));
        assert_eq!(parse_lang_setting("zh"), Some(Lang::Zh));
        assert_eq!(parse_lang_setting("fr"), Some(Lang::Fr));
        assert_eq!(parse_lang_setting("es"), Some(Lang::Es));
        assert_eq!(lang_setting_value(Lang::Fr), "fr");
        assert_eq!(lang_setting_value(Lang::Es), "es");
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
    fn french_translations_cover_all_current_ui_english_keys() {
        let source = include_str!("lib.rs");
        let source = source
            .split("#[cfg(test)]")
            .next()
            .expect("source before tests");
        let keys = extract_english_translation_keys(source);
        let missing: Vec<_> = keys
            .into_iter()
            .filter(|key| !has_translation_fr(key))
            .collect();
        assert!(
            missing.is_empty(),
            "missing French translations: {missing:?}"
        );
    }

    #[test]
    fn spanish_translations_cover_all_current_ui_english_keys() {
        let source = include_str!("lib.rs");
        let source = source
            .split("#[cfg(test)]")
            .next()
            .expect("source before tests");
        let keys = extract_english_translation_keys(source);
        let missing: Vec<_> = keys
            .into_iter()
            .filter(|key| !has_translation_es(key))
            .collect();
        assert!(
            missing.is_empty(),
            "missing Spanish translations: {missing:?}"
        );
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
            status: AppStatus::Completed,
            summary: ScanSummary::default(),
            store: Some(store),
            scan_session: None,
            scan_finalize_session: None,
            delete_session: None,
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
            .all(|(path, _)| path.starts_with("d:\\appdata\\local\\sdk\\")));
        assert_eq!(files[0].0, "d:\\appdata\\local\\sdk\\system.img");
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
        assert_eq!(target.path, "c:\\$Recycle.Bin\\S-1-5-18");
        assert_eq!(target.name, "S-1-5-18");
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
        assert!(entries.iter().any(|entry| entry.path == "d:\\Users"));
        assert!(entries.iter().any(|entry| entry.path == "d:\\pagefile.sys"));
        assert!(!entries
            .iter()
            .any(|entry| entry.path == "d:\\Users\\alice.dat"));
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
        assert_eq!(focus.path, "d:\\");
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
