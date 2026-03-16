use dirforge_actions::build_deletion_plan;
use dirforge_cache::{CacheStore, HistoryRecord};
use dirforge_core::{NodeStore, ScanErrorRecord, ScanSummary};
use dirforge_dup::detect_duplicates;
use dirforge_report::export_text_report;
use dirforge_scan::{start_scan, ScanEvent, ScanHandle};
use eframe::egui;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Dashboard,
    CurrentScan,
    History,
    Errors,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    En,
    Zh,
}

pub struct DirForgeNativeApp {
    page: Page,
    root_input: String,
    status: String,
    summary: ScanSummary,
    store: Option<NodeStore>,
    dup_summary: String,
    scan_handle: Option<ScanHandle>,
    history: Vec<HistoryRecord>,
    errors: Vec<ScanErrorRecord>,
    selected_history_id: Option<i64>,
    language: Lang,
    theme_dark: bool,
    cache: CacheStore,
}

impl DirForgeNativeApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let cache = CacheStore::new("dirforge.db").expect("open sqlite cache");
        let default_lang = detect_lang();
        let lang = cache
            .get_setting("language")
            .ok()
            .flatten()
            .map(|v| if v == "zh" { Lang::Zh } else { Lang::En })
            .unwrap_or(default_lang);
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
            dup_summary: String::new(),
            scan_handle: None,
            history: Vec::new(),
            errors: Vec::new(),
            selected_history_id: None,
            language: lang,
            theme_dark,
            cache,
        };

        let _ = app.reload_history();
        if let Ok(Some(snapshot)) = app.cache.load_latest_snapshot(&app.root_input) {
            app.store = Some(snapshot);
        }

        app
    }

    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        match self.language {
            Lang::Zh => zh,
            Lang::En => en,
        }
    }

    fn reload_history(&mut self) -> rusqlite::Result<()> {
        self.history = self.cache.list_history(100)?;
        Ok(())
    }

    fn start_scan(&mut self) {
        self.status = self.t("扫描中", "Scanning").to_string();
        self.scan_handle = Some(start_scan(PathBuf::from(self.root_input.clone())));
        self.page = Page::CurrentScan;
    }

    fn poll_events(&mut self) {
        let mut finished = None;
        if let Some(handle) = &self.scan_handle {
            while let Ok(event) = handle.events.try_recv() {
                match event {
                    ScanEvent::Progress(p) => {
                        self.summary = p.summary;
                    }
                    ScanEvent::Snapshot(s) => {
                        self.store = Some(s);
                    }
                    ScanEvent::Finished {
                        store,
                        summary,
                        errors,
                    } => {
                        finished = Some((store, summary, errors));
                    }
                }
            }
        }

        if let Some((store, summary, errors)) = finished {
            self.summary = summary.clone();
            self.status = self.t("完成", "Completed").to_string();
            self.dup_summary = {
                let dups = detect_duplicates(&store);
                let files: Vec<(String, u64)> = dups
                    .iter()
                    .flat_map(|g| g.members.iter().skip(1).map(move |p| (p.clone(), g.size)))
                    .collect();
                let plan = build_deletion_plan(files);
                format!(
                    "{} groups, {} files reclaim {} bytes",
                    dups.len(),
                    plan.files.len(),
                    plan.reclaimable_bytes
                )
            };
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
            self.scan_handle = None;
        }
    }

    fn ui_nav(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("导航", "Navigation"));
        nav_btn(ui, self, Page::Dashboard, self.t("首页", "Dashboard"));
        nav_btn(
            ui,
            self,
            Page::CurrentScan,
            self.t("当前扫描", "Current Scan"),
        );
        nav_btn(ui, self, Page::History, self.t("历史快照", "History"));
        nav_btn(ui, self, Page::Errors, self.t("错误中心", "Error Center"));
        nav_btn(ui, self, Page::Settings, self.t("设置", "Settings"));
    }

    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("DirForge 首页", "DirForge Dashboard"));
        ui.label(format!("{}: {}", self.t("状态", "Status"), self.status));
        ui.horizontal(|ui| {
            ui.label(self.t("扫描根路径", "Scan root"));
            ui.text_edit_singleline(&mut self.root_input);
            if ui.button(self.t("开始扫描", "Start scan")).clicked() {
                self.start_scan();
            }
        });

        ui.separator();
        ui.label(self.t("最近历史", "Recent history"));
        for h in self.history.iter().take(5) {
            ui.label(format!(
                "#{} {} files={} dirs={} errors={}",
                h.id, h.root, h.scanned_files, h.scanned_dirs, h.error_count
            ));
        }
    }

    fn ui_current_scan(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("当前扫描", "Current Scan"));
        ui.label(format!(
            "files={} dirs={} bytes={} errors={}",
            self.summary.scanned_files,
            self.summary.scanned_dirs,
            self.summary.bytes_observed,
            self.summary.error_count
        ));
        if !self.dup_summary.is_empty() {
            ui.label(format!("dup: {}", self.dup_summary));
        }
        ui.separator();
        ui.label(self.t("最大文件 Top 20", "Top 20 Largest Files"));
        if let Some(store) = &self.store {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for node in store.top_n_largest_files(20) {
                    ui.label(format!("{} ({})", node.path, node.size_self));
                }
            });
        }
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("历史快照页", "History Snapshots"));
        if ui.button(self.t("刷新历史", "Refresh History")).clicked() {
            let _ = self.reload_history();
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for h in &self.history {
                if ui
                    .selectable_label(
                        self.selected_history_id == Some(h.id),
                        format!(
                            "#{} {} files={} dirs={} bytes={} ts={}",
                            h.id,
                            h.root,
                            h.scanned_files,
                            h.scanned_dirs,
                            h.bytes_observed,
                            h.created_at
                        ),
                    )
                    .clicked()
                {
                    self.selected_history_id = Some(h.id);
                    if let Ok(e) = self.cache.list_errors_by_history(h.id) {
                        self.errors = e;
                    }
                }
            }
        });
    }

    fn ui_errors(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("错误中心", "Error Center"));
        if self.errors.is_empty() {
            ui.label(self.t("暂无错误记录", "No errors"));
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for e in &self.errors {
                ui.group(|ui| {
                    ui.label(format!("{}: {}", self.t("路径", "Path"), e.path));
                    ui.label(format!("{}: {}", self.t("原因", "Reason"), e.reason));
                });
            }
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(self.t("设置", "Settings"));

        ui.horizontal(|ui| {
            ui.label(self.t("语言", "Language"));
            let mut lang_zh = self.language == Lang::Zh;
            if ui.checkbox(&mut lang_zh, "中文 / Chinese").changed() {
                self.language = if lang_zh { Lang::Zh } else { Lang::En };
                let _ = self.cache.set_setting(
                    "language",
                    if self.language == Lang::Zh {
                        "zh"
                    } else {
                        "en"
                    },
                );
            }
        });

        let dark_label = self.t("深色主题", "Dark theme");
        if ui.checkbox(&mut self.theme_dark, dark_label).changed() {
            if self.theme_dark {
                ctx.set_visuals(egui::Visuals::dark());
            } else {
                ctx.set_visuals(egui::Visuals::light());
            }
            let _ = self
                .cache
                .set_setting("theme", if self.theme_dark { "dark" } else { "light" });
        }

        ui.separator();
        ui.label(self.t(
            "默认语言会跟随系统语言；可在此手动覆盖。",
            "Default language follows system locale; you can override it here.",
        ));
    }
}

impl eframe::App for DirForgeNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
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
                if ui.button(self.t("取消扫描", "Cancel")).clicked() {
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
            Page::History => self.ui_history(ui),
            Page::Errors => self.ui_errors(ui),
            Page::Settings => self.ui_settings(ui, ctx),
        });
    }
}

fn nav_btn(ui: &mut egui::Ui, app: &mut DirForgeNativeApp, page: Page, text: &str) {
    if ui.selectable_label(app.page == page, text).clicked() {
        app.page = page;
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
