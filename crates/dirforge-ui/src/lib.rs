use dirforge_actions::{
    build_deletion_plan_with_origin, execute_plan, ExecutionMode, ExecutionReport, SelectionOrigin,
};
use dirforge_cache::{CacheStore, HistoryRecord};
use dirforge_core::{
    ErrorKind, Node, NodeId, NodeKind, NodeStore, RiskLevel, ScanErrorRecord, ScanProfile,
    ScanSummary, SnapshotDelta,
};
use dirforge_report::{
    default_manifest, export_diagnostics_archive, export_diagnostics_bundle, export_errors_csv,
};
use dirforge_scan::{start_scan, BatchEntry, ScanConfig, ScanEvent};
use dirforge_telemetry as telemetry;
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
const TOOLBAR_HEIGHT: f32 = 44.0;
const STATUSBAR_HEIGHT: f32 = 26.0;
const CARD_RADIUS: u8 = 14;
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
    latest_progress: Option<dirforge_scan::ScanProgress>,
    pending_batches: VecDeque<Vec<BatchEntry>>,
    latest_snapshot: Option<(SnapshotDelta, dirforge_scan::SnapshotView)>,
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

pub struct DirForgeNativeApp {
    egui_ctx: egui::Context,
    page: Page,
    root_input: String,
    status: String,
    summary: ScanSummary,
    store: Option<NodeStore>,
    scan_session: Option<ScanSession>,
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

impl DirForgeNativeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        let cache = CacheStore::new("dirforge.db").expect("open sqlite cache");
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

        let mut app = Self {
            egui_ctx: cc.egui_ctx.clone(),
            page: Page::Dashboard,
            root_input: ".".into(),
            status: "Idle".into(),
            summary: ScanSummary::default(),
            store: None,
            scan_session: None,
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
        let plan = build_deletion_plan_with_origin(
            vec![(
                target.path.clone(),
                target.size_bytes,
                self.risk_for_path(&target.path),
            )],
            self.selection_origin(),
        );
        let report = execute_plan(&plan, mode);
        let payload = serde_json::json!({
            "path": target.path,
            "mode": format!("{:?}", report.mode),
            "attempted": report.attempted,
            "succeeded": report.succeeded,
            "failed": report.failed,
        })
        .to_string();
        let _ = self.cache.add_audit_event("delete_execute", &payload);
        if report.succeeded > 0 {
            self.prune_deleted_target(&target);
            self.status = self.t("删除已执行", "Delete executed").to_string();
        }
        self.execution_report = Some(report);
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
    }

    fn current_volume_info(&self) -> Option<dirforge_platform::VolumeInfo> {
        dirforge_platform::volume_info(&self.root_input).ok()
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

    fn refresh_diagnostics(&mut self) {
        let cache_payload = self
            .cache
            .export_diagnostics_json()
            .unwrap_or_else(|_| "{}".to_string());
        let telemetry_snapshot = telemetry::snapshot();
        let system_snapshot = telemetry::system_snapshot();
        let metrics = telemetry::metric_descriptors();
        let audit = telemetry::action_audit_tail(32);
        let path_access = dirforge_platform::assess_path_access(&self.root_input)
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
        self.status = self.t("扫描中", "Scanning").to_string();
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
            self.status = self.t("完成", "Completed").to_string();
            self.scan_current_path = None;
            self.scan_last_event_at = None;
            self.completed_top_files = finished.top_files;
            self.completed_top_dirs = finished.top_dirs;
            let _ = export_errors_csv(&finished.errors, "dirforge_errors.csv");
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
        ui.heading("DirForge");
        ui.label(
            egui::RichText::new("[relay-1]")
                .text_style(egui::TextStyle::Small)
                .color(egui::Color32::from_rgb(33, 158, 188)),
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
                    [ui.available_width(), 32.0],
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
        ui.add_space(8.0);
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
                self.t("[relay-1] 扫描仍在进行", "[relay-1] Scan Still Running"),
                &format!(
                    "{} {}\n{}",
                    self.t("当前正在处理：", "Currently working on:"),
                    current_path,
                    self.scan_health_summary()
                ),
            );
            ui.add_space(10.0);
        }

        ui.columns(2, |columns| {
            surface_frame(&columns[0]).show(&mut columns[0], |ui| {
                let root_hint = self
                    .t("输入目录，例如 D:\\", "Enter a folder, e.g. D:\\")
                    .to_string();
                ui.label(
                    egui::RichText::new(self.t("扫描目标", "Scan Target"))
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                ui.add_space(6.0);
                ui.label(self.t("根目录", "Root path"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.root_input)
                        .desired_width(f32::INFINITY)
                        .hint_text(root_hint),
                );
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(self.t("扫描策略", "Profile"));
                    ui.selectable_value(&mut self.scan_profile, ScanProfile::Ssd, "SSD");
                    ui.selectable_value(&mut self.scan_profile, ScanProfile::Hdd, "HDD");
                    ui.selectable_value(&mut self.scan_profile, ScanProfile::Network, "Network");
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(self.t("批大小", "Batch size"));
                        ui.add(
                            egui::DragValue::new(&mut self.event_batch_size)
                                .range(32..=4096)
                                .speed(8),
                        );
                    });
                    ui.vertical(|ui| {
                        ui.label(self.t("快照间隔", "Snapshot interval"));
                        ui.add(
                            egui::DragValue::new(&mut self.snapshot_interval_ms)
                                .range(50..=1000)
                                .suffix(" ms")
                                .speed(5),
                        );
                    });
                });
                ui.add_space(14.0);
                let start_label = if self.scan_active() {
                    self.t("扫描进行中", "Scanning")
                } else {
                    self.t("开始扫描", "Start Scan")
                };
                let start_button = egui::Button::new(start_label);
                if ui
                    .add_enabled(!self.scan_active(), start_button)
                    .on_hover_text(self.t(
                        "扫描进行中时请使用右上角的停止按钮。",
                        "Use the top-right stop button while a scan is running.",
                    ))
                    .clicked()
                {
                    self.start_scan();
                }
            });

            surface_frame(&columns[1]).show(&mut columns[1], |ui| {
                ui.label(
                    egui::RichText::new(self.t("卷空间摘要", "Volume Summary"))
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                ui.add_space(10.0);
                status_badge(ui, &self.status, self.scan_active());
                ui.add_space(12.0);

                if let Some((used, free, total)) = self.volume_numbers() {
                    stat_row(
                        ui,
                        self.t("磁盘已用", "Used"),
                        &format_bytes(used),
                        &format!("{} {}", format_bytes(total), self.t("总容量", "total")),
                    );
                    stat_row(
                        ui,
                        self.t("磁盘可用", "Free"),
                        &format_bytes(free),
                        self.t("系统卷信息", "System volume info"),
                    );
                    stat_row(
                        ui,
                        self.t("已扫描", "Scanned"),
                        &format_bytes(self.summary.bytes_observed),
                        self.t(
                            "本次已遍历到的文件总大小",
                            "Total file bytes scanned so far",
                        ),
                    );

                    ui.add_space(10.0);
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

                ui.add_space(10.0);
                stat_row(
                    ui,
                    self.t("文件数", "Files"),
                    &format_count(self.summary.scanned_files),
                    self.t("当前已统计文件", "Files counted"),
                );
                stat_row(
                    ui,
                    self.t("目录数", "Folders"),
                    &format_count(self.summary.scanned_dirs),
                    self.t("当前已遍历目录", "Folders traversed"),
                );
                stat_row(
                    ui,
                    self.t("错误", "Errors"),
                    &format_count(self.summary.error_count),
                    self.t("无法读取或被跳过的路径", "Unreadable or skipped paths"),
                );
            });
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
        ui.columns(2, |columns| {
            render_ranked_size_list(
                &mut columns[0],
                &folders_title,
                &folders_subtitle,
                &ranked_dirs,
                self.summary.bytes_observed,
                420.0,
                &mut self.selection,
                &mut self.execution_report,
            );
            render_ranked_size_list(
                &mut columns[1],
                &files_title,
                &files_subtitle,
                &ranked_files,
                self.summary.bytes_observed,
                420.0,
                &mut self.selection,
                &mut self.execution_report,
            );
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
                self.t("[relay-1] 这是实时增量视图", "[relay-1] This Is a Live Incremental View"),
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
                egui::Color32::from_rgb(33, 158, 188),
                egui::Color32::from_rgb(61, 133, 198),
                egui::Color32::from_rgb(76, 201, 176),
                egui::Color32::from_rgb(87, 117, 144),
                egui::Color32::from_rgb(231, 111, 81),
            ];
            for (idx, column) in columns.iter_mut().enumerate() {
                if let Some(card) = cards.get(idx) {
                    metric_card(column, &card.0, &card.1, &card.2, accents[idx]);
                }
            }
        });

        ui.add_space(12.0);
        let ranked_dirs = self.current_ranked_dirs(12);
        let ranked_files = self.current_ranked_files(12);
        let live_folders_title = self
            .t("当前最大的文件夹", "Largest Folders Found So Far")
            .to_string();
        let live_folders_subtitle = self
            .t(
                "扫描还未结束时，这里会持续更新。",
                "This keeps updating until the scan finishes.",
            )
            .to_string();
        let live_files_title = self
            .t("当前最大的文件", "Largest Files Found So Far")
            .to_string();
        let live_files_subtitle = self
            .t(
                "先发现的结果不代表最终排序。",
                "Early findings are not yet the final ordering.",
            )
            .to_string();
        ui.columns(2, |columns| {
            render_ranked_size_list(
                &mut columns[0],
                &live_folders_title,
                &live_folders_subtitle,
                &ranked_dirs,
                self.summary.bytes_observed,
                460.0,
                &mut self.selection,
                &mut self.execution_report,
            );
            render_ranked_size_list(
                &mut columns[1],
                &live_files_title,
                &live_files_subtitle,
                &ranked_files,
                self.summary.bytes_observed,
                460.0,
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
                    color = egui::Color32::from_rgb(42, 157, 143);
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
                egui::Color32::from_rgb(244, 162, 97),
            );
            metric_card(
                &mut columns[1],
                "Transient",
                &format_count(transient as u64),
                self.t("可重试的瞬时失败", "Retryable transient failures"),
                egui::Color32::from_rgb(33, 158, 188),
            );
            metric_card(
                &mut columns[2],
                "System",
                &format_count(system as u64),
                self.t("系统级故障", "System-level failures"),
                egui::Color32::from_rgb(231, 111, 81),
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
            manifest.diagnostics_payload_file = "dirforge_diagnostics.json".to_string();
            manifest.summary_report_file = "dirforge_summary.json".to_string();
            manifest.duplicate_report_file = "dirforge_duplicates.csv".to_string();
            manifest.error_report_file = "dirforge_errors.csv".to_string();
            let _ = export_diagnostics_bundle(
                &self.diagnostics_json,
                "dirforge_diagnostics.json",
                &manifest,
            );
            let _ = export_diagnostics_archive(
                &self.diagnostics_json,
                "diagnostics",
                "dirforge",
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
        page_header(
            ui,
            self.t("偏好设置", "Settings"),
            self.t(
                "修复中文字体后，语言选择改成清晰的单选项，不再用含糊复选框。",
                "After fixing CJK font fallback, language selection becomes explicit radio choices instead of an ambiguous checkbox.",
            ),
        );
        ui.add_space(8.0);

        surface_frame(ui).show(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("界面语言", "Interface Language"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.language == Lang::Zh, self.t("中文", "中文"))
                    .clicked()
                {
                    self.language = Lang::Zh;
                    let _ = self.cache.set_setting("language", "zh");
                }
                if ui
                    .selectable_label(self.language == Lang::En, "English")
                    .clicked()
                {
                    self.language = Lang::En;
                    let _ = self.cache.set_setting("language", "en");
                }
            });
            ui.add_space(10.0);
            let dark_label = self.t("深色主题", "Dark theme");
            if ui.checkbox(&mut self.theme_dark, dark_label).changed() {
                self.apply_theme(ctx);
                let _ = self
                    .cache
                    .set_setting("theme", if self.theme_dark { "dark" } else { "light" });
            }
        });

        ui.add_space(10.0);
        surface_frame(ui).show(ui, |ui| {
            ui.label(
                egui::RichText::new(self.t("本地化说明", "Localization Notes"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.label(self.t(
                "应用会优先加载系统中的中文字体回退（Windows 优先 Microsoft YaHei / DengXian），避免中文标题和设置项显示为方框。",
                "The app now prefers CJK-capable system fallback fonts (Windows prioritizes Microsoft YaHei / DengXian) so Chinese labels do not render as tofu boxes.",
            ));
            ui.label(self.t(
                "首次启动默认仍可根据系统语言环境推断中英文，但设置页的手动选择会覆盖自动检测结果。",
                "The first launch can still infer language from the system locale, but manual selection here overrides auto-detection.",
            ));
        });
    }

    fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("DirForge")
                    .size(22.0)
                    .strong()
                    .color(ui.visuals().text_color()),
            );
            ui.add_space(10.0);
            status_badge(ui, &self.status, self.scan_active());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let active = self.scan_active();
                let stop_label = if active {
                    self.t("停止扫描", "Stop Scan")
                } else {
                    self.t("取消", "Cancel")
                };
                if ui
                    .add_enabled(active, egui::Button::new(stop_label))
                    .clicked()
                {
                    if let Some(session) = &self.scan_session {
                        session.cancel.store(true, Ordering::SeqCst);
                        self.status = self.t("已取消", "Cancelled").to_string();
                        self.scan_current_path = None;
                    }
                }
                let start_label = if active {
                    self.t("扫描中", "Scanning")
                } else {
                    self.t("开始扫描", "Start Scan")
                };
                if ui
                    .add_enabled(!active, egui::Button::new(start_label))
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
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        has_selection,
                        egui::Button::new(self.t("移到回收站", "Move to Recycle Bin")),
                    )
                    .clicked()
                {
                    self.execute_selected_delete(ExecutionMode::RecycleBin);
                }
                let permanent = egui::Button::new(self.t("永久删除", "Delete Permanently"))
                    .fill(egui::Color32::from_rgb(157, 53, 53));
                if ui.add_enabled(has_selection, permanent).clicked() {
                    self.execute_selected_delete(ExecutionMode::Permanent);
                }
            });
            if !has_selection {
                ui.label(
                    egui::RichText::new(self.t(
                        "先从列表、树图、历史或错误列表里选中一个文件或文件夹。",
                        "Select a file or folder from a list, treemap, history, or errors first.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
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
                            egui::Color32::from_rgb(231, 111, 81)
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
        });
    }
}

impl eframe::App for DirForgeNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_scan_events();
        self.apply_theme(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "DirForge [relay-1] {}",
            self.status
        )));
        if self.scan_active() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        egui::TopBottomPanel::top("top_bar")
            .exact_height(TOOLBAR_HEIGHT)
            .frame(toolbar_frame(ctx))
            .show(ctx, |ui| self.ui_toolbar(ui));

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(STATUSBAR_HEIGHT)
            .frame(statusbar_frame(ctx))
            .show(ctx, |ui| self.ui_statusbar(ui));

        egui::SidePanel::left("nav")
            .exact_width(NAV_WIDTH)
            .resizable(false)
            .frame(panel_frame(ctx))
            .show(ctx, |ui| self.ui_nav(ui));

        egui::SidePanel::right("inspector")
            .exact_width(INSPECTOR_WIDTH)
            .resizable(true)
            .frame(panel_frame(ctx))
            .show(ctx, |ui| self.ui_inspector(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(ctx.style().visuals.window_fill)
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| match self.page {
                Page::Dashboard => self.ui_dashboard(ui),
                Page::CurrentScan => self.ui_current_scan(ui),
                Page::Treemap => self.ui_treemap(ui),
                Page::History => self.ui_history(ui),
                Page::Errors => self.ui_errors(ui),
                Page::Diagnostics => self.ui_diagnostics(ui),
                Page::Settings => self.ui_settings(ui, ctx),
            });
    }
}

fn layout_treemap_recursive(
    rect: egui::Rect,
    dirs: &[&dirforge_core::Node],
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
    if let Some(data) = load_system_font_bytes() {
        fonts
            .font_data
            .insert("cjk-fallback".to_string(), egui::FontData::from_owned(data));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "cjk-fallback".to_string());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("cjk-fallback".to_string());
    }
    ctx.set_fonts(fonts);
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

fn build_dark_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = egui::Color32::from_rgb(15, 18, 24);
    visuals.panel_fill = egui::Color32::from_rgb(18, 22, 29);
    visuals.extreme_bg_color = egui::Color32::from_rgb(11, 14, 20);
    visuals.faint_bg_color = egui::Color32::from_rgb(29, 35, 44);
    visuals.code_bg_color = egui::Color32::from_rgb(12, 17, 24);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(18, 22, 29);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 48, 61));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(27, 33, 42);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 55, 70));
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(34, 43, 55);
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(74, 144, 164));
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(42, 68, 77);
    visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(96, 191, 171));
    visuals.selection.bg_fill = egui::Color32::from_rgb(36, 111, 150);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals
}

fn build_light_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::light();
    visuals.window_fill = egui::Color32::from_rgb(245, 247, 250);
    visuals.panel_fill = egui::Color32::from_rgb(252, 253, 255);
    visuals.extreme_bg_color = egui::Color32::from_rgb(234, 239, 244);
    visuals.faint_bg_color = egui::Color32::from_rgb(240, 244, 248);
    visuals.code_bg_color = egui::Color32::from_rgb(238, 244, 248);
    visuals.override_text_color = Some(egui::Color32::from_rgb(31, 41, 55));
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(252, 253, 255);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(210, 218, 230));
    visuals.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 65, 81));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(243, 246, 250);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(196, 206, 219));
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(227, 238, 248);
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(89, 141, 188));
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(206, 225, 242);
    visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(57, 112, 161));
    visuals.selection.bg_fill = egui::Color32::from_rgb(60, 128, 171);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals
}

fn panel_frame(ctx: &egui::Context) -> egui::Frame {
    let visuals = &ctx.style().visuals;
    egui::Frame::default()
        .fill(visuals.panel_fill)
        .inner_margin(egui::Margin::same(12.0))
        .rounding(egui::Rounding::same(CARD_RADIUS as f32))
        .stroke(egui::Stroke::new(1.0, border_color(visuals)))
}

fn toolbar_frame(ctx: &egui::Context) -> egui::Frame {
    let visuals = &ctx.style().visuals;
    egui::Frame::default()
        .fill(visuals.panel_fill)
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .stroke(egui::Stroke::new(1.0, border_color(visuals)))
}

fn statusbar_frame(ctx: &egui::Context) -> egui::Frame {
    let visuals = &ctx.style().visuals;
    egui::Frame::default()
        .fill(visuals.panel_fill)
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .stroke(egui::Stroke::new(1.0, border_color(visuals)))
}

fn surface_frame(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::default()
        .fill(visuals.faint_bg_color)
        .inner_margin(egui::Margin::same(12.0))
        .rounding(egui::Rounding::same(CARD_RADIUS as f32))
        .stroke(egui::Stroke::new(1.0, border_color(visuals)))
}

fn border_color(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.dark_mode {
        egui::Color32::from_rgb(40, 48, 61)
    } else {
        egui::Color32::from_rgb(214, 221, 231)
    }
}

fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(
        egui::RichText::new(title)
            .text_style(egui::TextStyle::Heading)
            .strong(),
    );
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

fn render_ranked_size_list(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    items: &[(String, u64)],
    total: u64,
    max_height: f32,
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
                ui.label("No data");
                return;
            }

            let denom = total.max(items.iter().map(|(_, size)| *size).max().unwrap_or(1));
            egui::ScrollArea::vertical()
                .id_source(("ranked-scroll", title))
                .auto_shrink([false; 2])
                .max_height(max_height)
                .show(ui, |ui| {
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
                        ui.add_space(4.0);
                    }
                });
        });
    });
}

fn tone_banner(ui: &mut egui::Ui, title: &str, body: &str) {
    let visuals = ui.visuals();
    egui::Frame::default()
        .fill(if visuals.dark_mode {
            egui::Color32::from_rgb(22, 33, 43)
        } else {
            egui::Color32::from_rgb(232, 241, 249)
        })
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong());
            ui.label(body);
        });
}

fn status_badge(ui: &mut egui::Ui, status: &str, active: bool) {
    let bg = if active {
        egui::Color32::from_rgb(33, 158, 188)
    } else {
        egui::Color32::from_rgb(99, 102, 111)
    };
    let text = egui::RichText::new(status)
        .color(egui::Color32::WHITE)
        .strong();
    egui::Frame::default()
        .fill(bg)
        .rounding(egui::Rounding::same(999.0))
        .inner_margin(egui::Margin::symmetric(10.0, 5.0))
        .show(ui, |ui: &mut egui::Ui| {
            ui.label(text);
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
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).strong());
        });
    });
}

fn palette_color(seed: usize) -> egui::Color32 {
    let palette = [
        egui::Color32::from_rgb(56, 86, 136),
        egui::Color32::from_rgb(66, 120, 122),
        egui::Color32::from_rgb(115, 92, 161),
        egui::Color32::from_rgb(57, 135, 92),
        egui::Color32::from_rgb(187, 92, 121),
        egui::Color32::from_rgb(143, 117, 61),
        egui::Color32::from_rgb(62, 104, 160),
        egui::Color32::from_rgb(102, 86, 132),
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
}
