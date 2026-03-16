use dirforge_actions::{
    build_deletion_plan_with_origin, execute_plan_simulated, DeletionPlan, ExecutionMode,
    ExecutionReport, SelectionOrigin,
};
use dirforge_cache::{CacheStore, HistoryRecord};
use dirforge_core::{
    ErrorKind, NodeId, NodeStore, RiskLevel, ScanErrorRecord, ScanProfile, ScanSummary,
    SnapshotDelta,
};
use dirforge_dup::{detect_duplicates, DupConfig, DuplicateGroup};
use dirforge_report::{
    default_manifest, export_diagnostics_archive, export_diagnostics_bundle, export_duplicates_csv,
    export_errors_csv, export_summary_json, export_text_report,
};
use dirforge_scan::{start_scan, BatchEntry, ScanConfig, ScanEvent, ScanHandle};
use dirforge_telemetry as telemetry;
use eframe::egui;
use rayon::prelude::*;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const MAX_PENDING_BATCH_EVENTS: usize = 32;
const MAX_PENDING_SNAPSHOTS: usize = 8;
const MAX_LIVE_FILES: usize = 20_000;

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
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
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
        app.refresh_diagnostics();
        app
    }

    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        match self.language {
            Lang::Zh => zh,
            Lang::En => en,
        }
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
        ui.heading(self.t("导航", "Navigation"));
        for (p, label_zh, label_en) in [
            (Page::Dashboard, "首页", "Dashboard"),
            (Page::CurrentScan, "当前扫描", "Current Scan"),
            (Page::Treemap, "Treemap", "Treemap"),
            (Page::History, "历史快照", "History"),
            (Page::Errors, "错误中心", "Errors"),
            (Page::Operations, "操作中心", "Operations"),
            (Page::Diagnostics, "诊断", "Diagnostics"),
            (Page::Settings, "设置", "Settings"),
        ] {
            if ui
                .selectable_label(self.page == p, self.t(label_zh, label_en))
                .clicked()
            {
                self.page = p;
            }
        }
    }

    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("DirForge 首页", "DirForge Dashboard"));
        ui.horizontal(|ui| {
            ui.label(self.t("路径", "Root"));
            ui.text_edit_singleline(&mut self.root_input);
            if ui.button(self.t("开始扫描", "Start scan")).clicked() {
                self.start_scan();
            }
        });
        ui.horizontal(|ui| {
            ui.label(self.t("扫描配置", "Scan profile"));
            ui.selectable_value(&mut self.scan_profile, ScanProfile::Ssd, "SSD");
            ui.selectable_value(&mut self.scan_profile, ScanProfile::Hdd, "HDD");
            ui.selectable_value(&mut self.scan_profile, ScanProfile::Network, "Network");
        });
        ui.horizontal(|ui| {
            ui.label("batch");
            ui.add(egui::DragValue::new(&mut self.event_batch_size).range(32..=4096));
            ui.label("snapshot(ms)");
            ui.add(egui::DragValue::new(&mut self.snapshot_interval_ms).range(50..=1000));
        });

        ui.separator();
        ui.label(format!("{}: {}", self.t("状态", "Status"), self.status));
        ui.label(format!(
            "files={} dirs={} bytes={} errors={}",
            self.summary.scanned_files,
            self.summary.scanned_dirs,
            self.summary.bytes_observed,
            self.summary.error_count
        ));
    }

    fn ui_current_scan(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("当前扫描", "Current Scan"));
        ui.label(format!(
            "frame_ms={:.2} queue_depth={} avg_snapshot_commit_ms={} avg_scan_batch_size={}",
            self.perf.frame_ms,
            self.perf.snapshot_queue_depth,
            self.perf.avg_snapshot_commit_ms,
            self.perf.avg_scan_batch_size
        ));

        ui.label(self.t("扫描中 Top Files", "Top Files During Scan"));
        for (path, size) in self.live_top_files.iter().take(10) {
            ui.label(format!("{} ({})", path, size));
        }
        ui.separator();
        ui.label(self.t("扫描中 Top Dirs", "Top Dirs During Scan"));
        for (path, size) in self.live_top_dirs.iter().take(10) {
            ui.label(format!("{} ({})", path, size));
        }
        ui.separator();

        let rows = self.live_files.len();
        egui::ScrollArea::vertical().show_rows(ui, 22.0, rows, |ui, row_range| {
            for row in row_range {
                if let Some((path, size)) = self.live_files.get(row) {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                self.selection.selected_path.as_deref() == Some(path),
                                path,
                            )
                            .clicked()
                        {
                            self.selection.selected_path = Some(path.clone());
                            self.selection.source = Some(SelectionSource::Table);
                        }
                        ui.separator();
                        ui.label(size.to_string());
                    });
                }
            }
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

        let dirs = store.largest_dirs(40);
        let mut tiles = Vec::new();
        layout_treemap_recursive(viewport, &dirs, &mut tiles);
        self.treemap_cache = TreemapViewportCache {
            key: Some(key),
            tiles: tiles.clone(),
        };
        tiles
    }

    fn ui_treemap(&mut self, ui: &mut egui::Ui) {
        ui.heading("Treemap");
        let desired = egui::vec2(ui.available_width(), ui.available_height() - 20.0);
        let (rect, _response) = ui.allocate_exact_size(desired, egui::Sense::hover());
        if let Some(store) = self.store.clone() {
            let painter = ui.painter_at(rect);
            let tiles = self.treemap_tiles_for_viewport(&store, rect);

            for tile in &tiles {
                let resp = ui.interact(
                    tile.rect,
                    ui.make_persistent_id(("treemap", tile.node_id.0)),
                    egui::Sense::click(),
                );
                let mut color = egui::Color32::from_rgb(
                    (tile.node_id.0 as u8).wrapping_mul(29),
                    (tile.node_id.0 as u8).wrapping_mul(53),
                    140,
                );
                if self.selection.selected_node == Some(tile.node_id) {
                    color = egui::Color32::LIGHT_GREEN;
                }
                if resp.clicked() {
                    self.selection.selected_node = Some(tile.node_id);
                    if let Some(node) = store.nodes.get(tile.node_id.0) {
                        self.selection.selected_path = Some(node.path.clone());
                    }
                    self.selection.source = Some(SelectionSource::Treemap);
                }
                painter.rect_filled(tile.rect, 2.0, color);
                painter.text(
                    tile.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &tile.label,
                    egui::FontId::default(),
                    egui::Color32::WHITE,
                );
            }

            if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                if let Some(hit) = treemap_hit_test(&tiles, pointer_pos) {
                    ui.label(format!("hover: {}", hit.label));
                }
            }

            ui.label(self.t(
                "提示：真正 treemap 布局 + 命中测试 + viewport cache 已启用",
                "Treemap now uses layout engine + hit test + viewport cache",
            ));
        } else {
            ui.label(self.t("暂无数据", "No data"));
        }
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("历史快照", "History"));
        if ui.button(self.t("刷新", "Refresh")).clicked() {
            let _ = self.reload_history();
        }

        let selected = self
            .selected_history_id
            .and_then(|id| self.history.iter().find(|h| h.id == id))
            .cloned();

        if let Some(h) = selected {
            ui.group(|ui| {
                ui.heading(self.t("快照详情", "Snapshot Detail"));
                ui.label(format!("id={} root={}", h.id, h.root));
                ui.label(format!(
                    "files={} dirs={} bytes={} errors={}",
                    h.scanned_files, h.scanned_dirs, h.bytes_observed, h.error_count
                ));
                ui.label(format!("created_at={}", h.created_at));
            });
            ui.separator();
        }

        egui::ScrollArea::vertical().show_rows(ui, 22.0, self.history.len(), |ui, range| {
            for i in range {
                if let Some(h) = self.history.get(i) {
                    let label = format!(
                        "#{} {} files={} dirs={} bytes={} errors={}",
                        h.id,
                        h.root,
                        h.scanned_files,
                        h.scanned_dirs,
                        h.bytes_observed,
                        h.error_count
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
        ui.heading(self.t("错误中心", "Errors"));
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
        ui.label(format!(
            "User={} Transient={} System={}",
            user, transient, system
        ));

        let filter_label = self.t("全部", "All").to_string();
        ui.horizontal(|ui| {
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

        egui::ScrollArea::vertical().show_rows(ui, 24.0, filtered.len(), |ui, range| {
            for i in range {
                if let Some(e) = filtered.get(i) {
                    ui.group(|ui| {
                        if ui
                            .selectable_label(
                                self.selection.selected_path.as_deref() == Some(&e.path),
                                format!("[{:?}] {}", e.kind, e.path),
                            )
                            .clicked()
                        {
                            self.selection.selected_path = Some(e.path.clone());
                            self.selection.source = Some(SelectionSource::Error);
                        }
                        if ui.button(self.t("跳转路径", "Jump to path")).clicked() {
                            self.selection.selected_path = Some(e.path.clone());
                            self.page = Page::Operations;
                        }
                        ui.label(&e.reason);
                    });
                }
            }
        });
    }

    fn ui_operations(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("操作中心", "Operations"));
        if self.deletion_plan.is_none() {
            self.deletion_plan = Some(self.build_deletion_plan_from_duplicates());
        }
        if let Some(plan) = self.deletion_plan.clone() {
            ui.group(|ui| {
                ui.heading(self.t("待执行", "Pending"));
                if let Some(path) = &self.selection.selected_path {
                    ui.label(format!("selected={}", path));
                }
                ui.label(format!(
                    "files={} reclaim={} high_risk={}",
                    plan.files.len(),
                    plan.reclaimable_bytes,
                    plan.high_risk_count
                ));
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
            egui::ScrollArea::vertical().show_rows(ui, 22.0, plan.files.len(), |ui, range| {
                for i in range {
                    if let Some(item) = plan.files.get(i) {
                        ui.label(format!("{:?} {} ({})", item.risk, item.path, item.size));
                    }
                }
            });

            if let Some(report) = self.execution_report.clone() {
                ui.separator();
                ui.group(|ui| {
                    ui.heading(self.t("执行结果", "Result"));
                    ui.label(format!(
                        "mode={:?} attempted={} ok={} failed={}",
                        report.mode, report.attempted, report.succeeded, report.failed
                    ));
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
                egui::ScrollArea::vertical().show_rows(
                    ui,
                    22.0,
                    report.items.len(),
                    |ui, range| {
                        for i in range {
                            if let Some(it) = report.items.get(i) {
                                ui.label(format!(
                                    "{} | success={} | {}",
                                    it.path, it.success, it.message
                                ));
                            }
                        }
                    },
                );
            }
        }
    }

    fn ui_diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("诊断页", "Diagnostics"));
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
        ui.code(&self.diagnostics_json);
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(self.t("设置", "Settings"));
        ui.horizontal(|ui| {
            ui.label(self.t("语言", "Language"));
            let mut zh = self.language == Lang::Zh;
            if ui.checkbox(&mut zh, "中文 / Chinese").changed() {
                self.language = if zh { Lang::Zh } else { Lang::En };
                let _ = self
                    .cache
                    .set_setting("language", if zh { "zh" } else { "en" });
            }
        });

        let dark_label = self.t("深色主题", "Dark theme");
        if ui.checkbox(&mut self.theme_dark, dark_label).changed() {
            if self.theme_dark {
                ctx.set_visuals(egui::Visuals::dark());
                let _ = self.cache.set_setting("theme", "dark");
            } else {
                ctx.set_visuals(egui::Visuals::light());
                let _ = self.cache.set_setting("theme", "light");
            }
        }

        ui.separator();
        ui.label(self.t(
            "默认语言跟随系统语言环境（LC_ALL/LANG）。",
            "Default language follows system locale (LC_ALL/LANG).",
        ));
    }
}

impl eframe::App for DirForgeNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_scan_events();
        if self.theme_dark {
            ctx.set_visuals(egui::Visuals::dark());
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DirForge");
                ui.separator();
                ui.label(format!("{}: {}", self.t("状态", "Status"), self.status));
                if ui.button(self.t("开始扫描", "Start Scan")).clicked() {
                    self.start_scan();
                }
                if ui.button(self.t("取消", "Cancel")).clicked() {
                    if let Some(h) = &self.scan_handle {
                        h.cancel();
                        self.status = self.t("已取消", "Cancelled").to_string();
                    }
                }
            });
        });

        egui::SidePanel::left("nav").show(ctx, |ui| self.ui_nav(ui));

        egui::CentralPanel::default().show(ctx, |ui| match self.page {
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

fn map_nodes_to_rows(store: &NodeStore, ids: &[NodeId]) -> Vec<(String, u64)> {
    ids.par_iter()
        .filter_map(|id| store.nodes.get(id.0))
        .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
        .collect()
}

fn layout_treemap_recursive(
    rect: egui::Rect,
    dirs: &[&dirforge_core::Node],
    out: &mut Vec<TreemapTile>,
) {
    if dirs.is_empty() {
        return;
    }
    if dirs.len() == 1 {
        out.push(TreemapTile {
            node_id: dirs[0].id,
            rect,
            label: dirs[0].name.clone(),
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
