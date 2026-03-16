use dirforge_actions::{
    build_deletion_plan_with_origin, execute_plan_simulated, DeletionPlan, ExecutionMode,
    ExecutionReport, SelectionOrigin,
};
use dirforge_cache::{CacheStore, HistoryRecord};
use dirforge_core::{
    ErrorKind, Node, NodeId, NodeKind, NodeStore, RiskLevel, ScanErrorRecord, ScanProfile,
    ScanSummary, SnapshotDelta,
};
use dirforge_dup::{detect_duplicates, DupConfig, DuplicateGroup};
use dirforge_report::{
    default_manifest, export_diagnostics_archive, export_diagnostics_bundle, export_duplicates_csv,
    export_errors_csv, export_summary_json, export_text_report,
};
use dirforge_scan::{start_scan, BatchEntry, ScanConfig, ScanEvent, ScanHandle};
use dirforge_telemetry as telemetry;
use eframe::egui;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const MAX_PENDING_BATCH_EVENTS: usize = 32;
const MAX_PENDING_SNAPSHOTS: usize = 8;
const MAX_LIVE_FILES: usize = 20_000;
const NAV_WIDTH: f32 = 188.0;
const INSPECTOR_WIDTH: f32 = 300.0;
const TOOLBAR_HEIGHT: f32 = 56.0;
const STATUSBAR_HEIGHT: f32 = 28.0;
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
    Operations,
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

pub struct DirForgeNativeApp {
    page: Page,
    root_input: String,
    status: String,
    summary: ScanSummary,
    store: Option<NodeStore>,
    scan_handle: Option<ScanHandle>,
    scan_profile: ScanProfile,
    snapshot_interval_ms: u64,
    event_batch_size: usize,

    pending_batch_events: VecDeque<Vec<BatchEntry>>,
    pending_snapshots: VecDeque<SnapshotDelta>,
    live_files: Vec<(String, u64)>,
    live_top_files: Vec<(String, u64)>,
    live_top_dirs: Vec<(String, u64)>,
    last_coalesce_commit: Instant,

    duplicates: Vec<DuplicateGroup>,
    deletion_plan: Option<DeletionPlan>,
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
            page: Page::Dashboard,
            root_input: ".".into(),
            status: "Idle".into(),
            summary: ScanSummary::default(),
            store: None,
            scan_handle: None,
            scan_profile: ScanProfile::Ssd,
            snapshot_interval_ms: 75,
            event_batch_size: 256,
            pending_batch_events: VecDeque::new(),
            pending_snapshots: VecDeque::new(),
            live_files: Vec::new(),
            live_top_files: Vec::new(),
            live_top_dirs: Vec::new(),
            last_coalesce_commit: Instant::now(),
            duplicates: Vec::new(),
            deletion_plan: None,
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

    fn current_ranked_dirs(&self, limit: usize) -> Vec<(String, u64)> {
        if self.scan_handle.is_some() && !self.live_top_dirs.is_empty() {
            return self.live_top_dirs.iter().take(limit).cloned().collect();
        }

        self.store
            .as_ref()
            .map(|store| {
                store
                    .largest_dirs(limit)
                    .into_iter()
                    .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn current_ranked_files(&self, limit: usize) -> Vec<(String, u64)> {
        if self.scan_handle.is_some() && !self.live_top_files.is_empty() {
            return self.live_top_files.iter().take(limit).cloned().collect();
        }

        self.store
            .as_ref()
            .map(|store| {
                store
                    .top_n_largest_files(limit)
                    .into_iter()
                    .map(|node| (node.path.clone(), node.size_self))
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
        self.pending_batch_events.clear();
        self.pending_snapshots.clear();
        self.live_files.clear();
        self.live_top_files.clear();
        self.live_top_dirs.clear();
        self.store = None;
        self.last_coalesce_commit = Instant::now();

        self.scan_handle = Some(start_scan(
            PathBuf::from(self.root_input.clone()),
            ScanConfig {
                profile: self.scan_profile,
                batch_size: self.event_batch_size.max(1),
                snapshot_ms: self.snapshot_interval_ms.max(50),
                metadata_parallelism: 4,
                deep_tasks_throttle: 64,
            },
        ));
        self.page = Page::CurrentScan;
    }

    fn process_scan_events(&mut self) {
        let frame_start = Instant::now();
        let mut finished = None;

        if let Some(handle) = &self.scan_handle {
            while let Ok(event) = handle.events.try_recv() {
                match event {
                    ScanEvent::Progress(p) => {
                        self.summary = p.summary;
                        self.perf.snapshot_queue_depth =
                            p.queue_depth.max(p.metadata_backlog).max(p.publisher_lag);
                    }
                    ScanEvent::Batch(batch) => {
                        self.pending_batch_events.push_back(batch);
                        if self.pending_batch_events.len() > MAX_PENDING_BATCH_EVENTS {
                            let drop_n = self.pending_batch_events.len() - MAX_PENDING_BATCH_EVENTS;
                            self.pending_batch_events.drain(0..drop_n);
                            telemetry::record_ui_backpressure(drop_n as u64, 0);
                        }
                    }
                    ScanEvent::Snapshot { delta, view } => {
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
                    ScanEvent::Finished { summary, errors } => finished = Some((summary, errors)),
                }
            }
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

        if let Some((summary, errors)) = finished {
            self.summary = summary.clone();
            self.status = self.t("完成", "Completed").to_string();
            let store = self.store.clone().unwrap_or_default();
            self.store = Some(store.clone());
            self.duplicates = detect_duplicates(&store, DupConfig::default());
            self.deletion_plan = Some(self.build_deletion_plan_from_duplicates());

            let _ = export_text_report(&store, "dirforge_report.txt");
            let _ = export_summary_json(&store, "dirforge_summary.json");
            let _ = export_duplicates_csv(&self.duplicates, "dirforge_duplicates.csv");
            let _ = export_errors_csv(&errors, "dirforge_errors.csv");
            let _ = self.cache.save_snapshot(&self.root_input, &store);
            let history_id = self
                .cache
                .record_scan_history(
                    &self.root_input,
                    summary.scanned_files,
                    summary.scanned_dirs,
                    summary.bytes_observed,
                    summary.error_count,
                    &errors,
                )
                .ok();

            self.errors = errors;
            if let Some(id) = history_id {
                self.selected_history_id = Some(id);
            }
            let _ = self.reload_history();
            self.refresh_diagnostics();
            self.scan_handle = None;
        }

        let t = telemetry::snapshot();
        self.perf.avg_snapshot_commit_ms = t.avg_snapshot_commit_ms;
        self.perf.avg_scan_batch_size = t.avg_scan_batch_size;
        self.perf.frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.perf.last_update = Some(Instant::now());
        telemetry::record_ui_frame();
    }

    fn build_deletion_plan_from_duplicates(&self) -> DeletionPlan {
        let candidates: Vec<(String, u64, RiskLevel)> = self
            .duplicates
            .iter()
            .flat_map(|g| {
                g.members
                    .iter()
                    .filter(|m| !m.keeper)
                    .map(|m| (m.path.clone(), m.size, g.risk))
                    .collect::<Vec<_>>()
            })
            .collect();
        build_deletion_plan_with_origin(candidates, SelectionOrigin::Duplicates)
    }

    fn ui_nav(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(self.t("空间分析工作台", "Storage Intelligence"))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.heading("DirForge");
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
            (Page::Operations, "操作中心", "Operations"),
            (Page::Diagnostics, "诊断导出", "Diagnostics"),
            (Page::Settings, "偏好设置", "Settings"),
        ] {
            let selected = self.page == p;
            let text = egui::RichText::new(self.t(label_zh, label_en)).size(14.0).strong();
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
                if ui
                    .add_sized(
                        [140.0, 36.0],
                        egui::Button::new(self.t("开始扫描", "Start Scan")),
                    )
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
                status_badge(ui, &self.status, self.scan_handle.is_some());
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
                        self.t("本次已遍历到的文件总大小", "Total file bytes scanned so far"),
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
                            .desired_width(f32::INFINITY),
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
        ui.columns(2, |columns| {
            render_ranked_size_list(
                &mut columns[0],
                self.t("最大文件夹", "Largest Folders"),
                self.t(
                    "优先看哪些目录占空间最多。",
                    "Start with the folders consuming the most space.",
                ),
                &self.current_ranked_dirs(10),
                self.summary.bytes_observed,
                &mut self.selection,
                self.store.as_ref(),
            );
            render_ranked_size_list(
                &mut columns[1],
                self.t("最大文件", "Largest Files"),
                self.t(
                    "这些通常是最直接可处理的空间占用点。",
                    "These are usually the quickest wins for reclaiming space.",
                ),
                &self.current_ranked_files(10),
                self.summary.bytes_observed,
                &mut self.selection,
                self.store.as_ref(),
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
        ui.columns(2, |columns| {
            render_ranked_size_list(
                &mut columns[0],
                self.t("当前最大的文件夹", "Largest Folders Found So Far"),
                self.t(
                    "扫描还未结束时，这里会持续更新。",
                    "This keeps updating until the scan finishes.",
                ),
                &self.current_ranked_dirs(12),
                self.summary.bytes_observed,
                &mut self.selection,
                self.store.as_ref(),
            );
            render_ranked_size_list(
                &mut columns[1],
                self.t("当前最大的文件", "Largest Files Found So Far"),
                self.t(
                    "先发现的结果不代表最终排序。",
                    "Early findings are not yet the final ordering.",
                ),
                &self.current_ranked_files(12),
                self.summary.bytes_observed,
                &mut self.selection,
                self.store.as_ref(),
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
                        if let Some((path, size)) = self.live_files.get(row) {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized(
                                        [ui.available_width() - 120.0, 24.0],
                                        egui::SelectableLabel::new(
                                            self.selection.selected_path.as_deref() == Some(path),
                                            truncate_middle(path, 92),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.select_path(path, SelectionSource::Table);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(format_bytes(*size));
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
                ui.label(self.t("暂无扫描结果，请先执行一次扫描。", "No scan data yet. Start a scan first."));
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
                            self.selection.selected_path = Some(e.path.clone());
                            self.selection.source = Some(SelectionSource::Error);
                        }
                        ui.horizontal(|ui| {
                            if ui.button(self.t("跳转路径", "Jump to path")).clicked() {
                                self.selection.selected_path = Some(e.path.clone());
                                self.page = Page::Operations;
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

    fn ui_operations(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            self.t("操作中心", "Operations"),
            self.t(
                "将建议回收量、风险和模拟执行结果集中到一处，避免关键数字难以辨认。",
                "Consolidate reclaimable size, risk, and simulated execution results into a readable workflow.",
            ),
        );
        ui.add_space(8.0);
        if self.deletion_plan.is_none() {
            self.deletion_plan = Some(self.build_deletion_plan_from_duplicates());
        }
        if let Some(plan) = self.deletion_plan.clone() {
            surface_frame(ui).show(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.t("待执行计划", "Pending Plan"))
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                if let Some(path) = &self.selection.selected_path {
                    ui.label(format!(
                        "{}: {}",
                        self.t("当前选中", "Selected"),
                        truncate_middle(path, 56)
                    ));
                }
                ui.add_space(8.0);
                stat_row(
                    ui,
                    self.t("文件数", "Files"),
                    &format_count(plan.files.len() as u64),
                    self.t("候选删除文件", "Candidate deletions"),
                );
                stat_row(
                    ui,
                    self.t("可回收", "Reclaimable"),
                    &format_bytes(plan.reclaimable_bytes),
                    self.t("预计释放空间", "Expected reclaimed size"),
                );
                stat_row(
                    ui,
                    self.t("高风险", "High Risk"),
                    &format_count(plan.high_risk_count as u64),
                    self.t("需重点复核", "Needs careful review"),
                );
                ui.horizontal(|ui| {
                    if ui
                        .button(self.t("模拟回收站删除", "Simulate recycle delete"))
                        .clicked()
                    {
                        self.execution_report =
                            Some(execute_plan_simulated(&plan, ExecutionMode::RecycleBin));
                    }
                    if ui
                        .button(self.t("模拟永久删除", "Simulate permanent delete"))
                        .clicked()
                    {
                        self.execution_report =
                            Some(execute_plan_simulated(&plan, ExecutionMode::Permanent));
                    }
                });
            });

            ui.separator();
            egui::ScrollArea::vertical().show_rows(ui, 28.0, plan.files.len(), |ui, range| {
                for i in range {
                    if let Some(item) = plan.files.get(i) {
                        data_row(
                            ui,
                            &format!(
                                "{:?}  {}",
                                item.risk,
                                truncate_middle(&item.path, 72)
                            ),
                            &format_bytes(item.size),
                        );
                    }
                }
            });

            if let Some(report) = self.execution_report.clone() {
                ui.separator();
                surface_frame(ui).show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(self.t("执行结果", "Execution Result"))
                            .text_style(egui::TextStyle::Name("title".into())),
                    );
                    ui.add_space(8.0);
                    stat_row(
                        ui,
                        self.t("模式", "Mode"),
                        &format!("{:?}", report.mode),
                        self.t("本次为模拟执行", "Simulation only"),
                    );
                    stat_row(
                        ui,
                        self.t("尝试", "Attempted"),
                        &format_count(report.attempted as u64),
                        &format!(
                            "{} {} / {} {}",
                            format_count(report.succeeded as u64),
                            self.t("成功", "succeeded"),
                            format_count(report.failed as u64),
                            self.t("失败", "failed")
                        ),
                    );
                    if ui
                        .button(self.t("记录批执行审计", "Record execution audit"))
                        .clicked()
                    {
                        let payload = serde_json::json!({
                            "mode": format!("{:?}", report.mode),
                            "attempted": report.attempted,
                            "succeeded": report.succeeded,
                            "failed": report.failed,
                        })
                        .to_string();
                        let _ = self
                            .cache
                            .add_audit_event("delete_execute_simulated", &payload);
                        self.refresh_diagnostics();
                    }
                });
                egui::ScrollArea::vertical().show_rows(ui, 28.0, report.items.len(), |ui, range| {
                        for i in range {
                            if let Some(it) = report.items.get(i) {
                                data_row(
                                    ui,
                                    &format!(
                                        "{}  {}",
                                        if it.success {
                                            self.t("成功", "OK")
                                        } else {
                                            self.t("失败", "Failed")
                                        },
                                        truncate_middle(&it.path, 72)
                                    ),
                                    &it.message,
                                );
                            }
                        }
                    });
            }
        }
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
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("DirForge")
                        .size(24.0)
                        .color(ui.visuals().text_color()),
                );
                ui.label(
                    egui::RichText::new(self.t("磁盘空间可视分析", "Disk Space Visual Analyzer"))
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
            });
            ui.add_space(16.0);
            status_badge(ui, &self.status, self.scan_handle.is_some());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(self.t("取消", "Cancel")).clicked() {
                    if let Some(h) = &self.scan_handle {
                        h.cancel();
                        self.status = self.t("已取消", "Cancelled").to_string();
                    }
                }
                if ui
                    .add(egui::Button::new(self.t("开始扫描", "Start Scan")))
                    .clicked()
                {
                    self.start_scan();
                }
            });
        });

        if self.scan_handle.is_some() {
            ui.add_space(10.0);
            tone_banner(
                ui,
                self.t("正在扫描", "Scan in Progress"),
                self.t(
                    "当前页面展示的是“已发现的部分结果”。扫描完成后，请回到概览页查看更接近 WinDirStat / WizTree 风格的汇总结果。",
                    "This page shows partial results discovered so far. After the scan completes, return to Overview for a more WinDirStat / WizTree style summary.",
                ),
            );
        }
    }

    fn ui_inspector(&mut self, ui: &mut egui::Ui) {
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
            if let Some(node) = self.selected_node() {
                stat_row(
                    ui,
                    self.t("名称", "Name"),
                    &node.name,
                    match node.kind {
                        NodeKind::Dir => self.t("目录", "Directory"),
                        NodeKind::File => self.t("文件", "File"),
                    },
                );
                stat_row(
                    ui,
                    self.t("路径", "Path"),
                    &truncate_middle(&node.path, 34),
                    self.t("完整路径可在悬浮提示中查看", "Full path available on hover"),
                );
                stat_row(
                    ui,
                    self.t("大小", "Size"),
                    &format_bytes(node.size_subtree.max(node.size_self)),
                    &format!(
                        "{} {} / {} {}",
                        format_count(node.file_count),
                        self.t("文件", "files"),
                        format_count(node.dir_count),
                        self.t("目录", "dirs")
                    ),
                );
            } else if let Some(path) = &self.selection.selected_path {
                stat_row(
                    ui,
                    self.t("路径", "Path"),
                    &truncate_middle(path, 34),
                    self.t("来自外部列表选择", "Selected from an external list"),
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
        });
    }
}

impl eframe::App for DirForgeNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_scan_events();
        self.apply_theme(ctx);

        egui::TopBottomPanel::top("top_bar")
            .exact_height(TOOLBAR_HEIGHT)
            .frame(panel_frame(ctx))
            .show(ctx, |ui| self.ui_toolbar(ui));

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(STATUSBAR_HEIGHT)
            .frame(panel_frame(ctx))
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
            Page::Operations => self.ui_operations(ui),
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
    if dirs.is_empty() || rect.width() < MIN_TREEMAP_TILE_EDGE || rect.height() < MIN_TREEMAP_TILE_EDGE
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
        fonts.font_data.insert(
            "cjk-fallback".to_string(),
            egui::FontData::from_owned(data),
        );
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
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 48, 61));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(27, 33, 42);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 55, 70));
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(34, 43, 55);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(74, 144, 164));
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(42, 68, 77);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(96, 191, 171));
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
        ui.label(
            egui::RichText::new(value)
                .size(22.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
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
    let text = egui::RichText::new(status).color(egui::Color32::WHITE).strong();
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

fn data_row(ui: &mut egui::Ui, left: &str, right: &str) {
    ui.horizontal(|ui| {
        ui.label(left);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(right)
                    .color(ui.visuals().weak_text_color()),
            );
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
