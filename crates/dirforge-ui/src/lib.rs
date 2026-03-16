use dirforge_actions::{build_deletion_plan, DeletionPlan};
use dirforge_cache::{CacheStore, HistoryRecord};
use dirforge_core::{
    NodeStore, RiskLevel, ScanErrorRecord, ScanProfile, ScanSummary, SnapshotDelta,
};
use dirforge_dup::{detect_duplicates, DupConfig, DuplicateGroup};
use dirforge_report::{export_diagnostics_bundle, export_text_report};
use dirforge_scan::{start_scan, BatchEntry, ScanConfig, ScanEvent, ScanHandle};
use eframe::egui;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

#[derive(Default)]
struct PerfMetrics {
    frame_ms: f32,
    snapshot_queue_depth: usize,
    last_update: Option<Instant>,
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

    pending_batch_events: Vec<Vec<BatchEntry>>,
    pending_snapshots: Vec<(NodeStore, SnapshotDelta)>,
    last_coalesce_commit: Instant,

    duplicates: Vec<DuplicateGroup>,
    deletion_plan: Option<DeletionPlan>,

    history: Vec<HistoryRecord>,
    errors: Vec<ScanErrorRecord>,
    selected_history_id: Option<i64>,

    language: Lang,
    theme_dark: bool,
    cache: CacheStore,

    perf: PerfMetrics,
    diagnostics_json: String,
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
            pending_batch_events: Vec::new(),
            pending_snapshots: Vec::new(),
            last_coalesce_commit: Instant::now(),
            duplicates: Vec::new(),
            deletion_plan: None,
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language,
            theme_dark,
            cache,
            perf: PerfMetrics::default(),
            diagnostics_json: String::new(),
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
        self.diagnostics_json = self
            .cache
            .export_diagnostics_json()
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
        self.last_coalesce_commit = Instant::now();

        self.scan_handle = Some(start_scan(
            PathBuf::from(self.root_input.clone()),
            ScanConfig {
                profile: self.scan_profile,
                batch_size: self.event_batch_size.max(1),
                snapshot_ms: self.snapshot_interval_ms.max(50),
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
                        self.perf.snapshot_queue_depth = p.queue_depth;
                    }
                    ScanEvent::Batch(batch) => self.pending_batch_events.push(batch),
                    ScanEvent::Snapshot { store, delta } => {
                        self.pending_snapshots.push((store, delta))
                    }
                    ScanEvent::Finished {
                        store,
                        summary,
                        errors,
                    } => finished = Some((store, summary, errors)),
                }
            }
        }

        // Snapshot coalescing: commit once per 50~100ms
        if self.last_coalesce_commit.elapsed()
            >= Duration::from_millis(self.snapshot_interval_ms.max(50))
        {
            if let Some((store, _delta)) = self.pending_snapshots.pop() {
                self.store = Some(store);
                self.pending_snapshots.clear();
            }
            self.pending_batch_events.clear();
            self.last_coalesce_commit = Instant::now();
        }

        if let Some((store, summary, errors)) = finished {
            self.summary = summary.clone();
            self.status = self.t("完成", "Completed").to_string();
            self.duplicates = detect_duplicates(&store, DupConfig::default());
            self.deletion_plan = Some(self.build_deletion_plan_from_duplicates());

            let _ = export_text_report(&store, "dirforge_report.txt");
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

            self.store = Some(store);
            self.errors = errors;
            if let Some(id) = history_id {
                self.selected_history_id = Some(id);
            }
            let _ = self.reload_history();
            self.refresh_diagnostics();
            self.scan_handle = None;
        }

        self.perf.frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.perf.last_update = Some(Instant::now());
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
        build_deletion_plan(candidates)
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
            "frame_ms={:.2} queue_depth={}",
            self.perf.frame_ms, self.perf.snapshot_queue_depth
        ));

        if let Some(store) = &self.store {
            let rows = store
                .nodes
                .iter()
                .filter(|n| n.kind == dirforge_core::NodeKind::File)
                .count();
            egui::ScrollArea::vertical().show_rows(ui, 22.0, rows, |ui, row_range| {
                let files: Vec<_> = store
                    .nodes
                    .iter()
                    .filter(|n| n.kind == dirforge_core::NodeKind::File)
                    .collect();
                for row in row_range {
                    if let Some(n) = files.get(row) {
                        ui.horizontal(|ui| {
                            ui.label(&n.path);
                            ui.separator();
                            ui.label(n.size_self.to_string());
                        });
                    }
                }
            });
        }
    }

    fn ui_treemap(&mut self, ui: &mut egui::Ui) {
        ui.heading("Treemap");
        let desired = egui::vec2(ui.available_width(), ui.available_height() - 20.0);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());
        if let Some(store) = &self.store {
            let dirs = store.largest_dirs(20);
            let total = dirs.iter().map(|d| d.size_subtree).sum::<u64>().max(1);
            let painter = ui.painter_at(rect);
            let mut x = rect.left();
            for d in dirs {
                let w = (d.size_subtree as f32 / total as f32) * rect.width();
                let r = egui::Rect::from_min_size(
                    egui::pos2(x, rect.top()),
                    egui::vec2(w.max(3.0), rect.height()),
                );
                let color = egui::Color32::from_rgb(
                    (d.id.0 as u8).wrapping_mul(29),
                    (d.id.0 as u8).wrapping_mul(53),
                    140,
                );
                painter.rect_filled(r, 2.0, color);
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    &d.name,
                    egui::FontId::default(),
                    egui::Color32::WHITE,
                );
                x += w;
            }
            if response.hovered() {
                ui.label(self.t("提示：点击导航查看明细", "Tip: use navigation for details"));
            }
        } else {
            ui.label(self.t("暂无数据", "No data"));
        }
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("历史快照", "History"));
        if ui.button(self.t("刷新", "Refresh")).clicked() {
            let _ = self.reload_history();
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
                    }
                }
            }
        });
    }

    fn ui_errors(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("错误中心", "Errors"));
        egui::ScrollArea::vertical().show_rows(ui, 24.0, self.errors.len(), |ui, range| {
            for i in range {
                if let Some(e) = self.errors.get(i) {
                    ui.group(|ui| {
                        ui.label(&e.path);
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
            ui.label(format!(
                "files={} reclaim={} high_risk={}",
                plan.files.len(),
                plan.reclaimable_bytes,
                plan.high_risk_count
            ));
            if ui
                .button(self.t("记录操作审计", "Record audit event"))
                .clicked()
            {
                let payload = serde_json::json!({
                    "files": plan.files.len(),
                    "reclaimable": plan.reclaimable_bytes,
                    "high_risk": plan.high_risk_count
                })
                .to_string();
                let _ = self.cache.add_audit_event("delete_plan_preview", &payload);
                self.refresh_diagnostics();
            }
            ui.separator();
            egui::ScrollArea::vertical().show_rows(ui, 22.0, plan.files.len(), |ui, range| {
                for i in range {
                    if let Some(item) = plan.files.get(i) {
                        ui.label(format!("{:?} {} ({})", item.risk, item.path, item.size));
                    }
                }
            });
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
            let _ = export_diagnostics_bundle(&self.diagnostics_json, "dirforge_diagnostics.json");
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
