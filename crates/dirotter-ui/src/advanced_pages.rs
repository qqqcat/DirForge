use super::*;

pub(super) fn ui_history(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    page_header(
        ui,
        app.t("DirOtter 工作台", "DirOtter Workspace"),
        app.t("历史快照", "History"),
        app.t(
            "按时间回看扫描快照，所有数字都改为适合人读的格式。",
            "Review previous scans with human-friendly formatting and clearer snapshot summaries.",
        ),
    );
    ui.add_space(8.0);
    if ui.button(app.t("刷新列表", "Refresh")).clicked() {
        let _ = app.reload_history();
    }

    let selected = app
        .selected_history_id
        .and_then(|id| app.history.iter().find(|h| h.id == id))
        .cloned();

    if let Some(h) = selected {
        surface_panel(ui, |ui| {
            ui.label(
                egui::RichText::new(app.t("快照详情", "Snapshot Detail"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.add_space(8.0);
            stat_row(
                ui,
                app.t("编号", "ID"),
                &h.id.to_string(),
                &truncate_middle(&h.root, 44),
            );
            stat_row(
                ui,
                app.t("文件", "Files"),
                &format_count(h.scanned_files),
                app.t("扫描到的文件数", "File count"),
            );
            stat_row(
                ui,
                app.t("目录", "Dirs"),
                &format_count(h.scanned_dirs),
                app.t("扫描到的目录数", "Directory count"),
            );
            stat_row(
                ui,
                app.t("体积", "Bytes"),
                &format_bytes(h.bytes_observed),
                app.t("历史扫描到的文件体积", "Historical scanned file size"),
            );
            stat_row(
                ui,
                app.t("错误", "Errors"),
                &format_count(h.error_count),
                &h.created_at.to_string(),
            );
        });
        ui.separator();
    }

    egui::ScrollArea::vertical().show_rows(ui, 22.0, app.history.len(), |ui, range| {
        for i in range {
            if let Some(h) = app.history.get(i) {
                let label = format!(
                    "#{} {}  |  {} {}  |  {} {}  |  {} {}",
                    h.id,
                    truncate_middle(&h.root, 34),
                    format_count(h.scanned_files),
                    app.t("文件", "files"),
                    format_count(h.scanned_dirs),
                    app.t("目录", "dirs"),
                    format_bytes(h.bytes_observed),
                    app.t("扫描体积", "scanned")
                );
                if ui
                    .selectable_label(app.selected_history_id == Some(h.id), label)
                    .clicked()
                {
                    app.selected_history_id = Some(h.id);
                    if let Ok(e) = app.cache.list_errors_by_history(h.id) {
                        app.errors = e;
                    }
                    app.selection.source = Some(SelectionSource::History);
                    app.execution_report = None;
                }
            }
        }
    });
}

pub(super) fn ui_errors(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    page_header(
        ui,
        app.t("DirOtter 工作台", "DirOtter Workspace"),
        app.t("错误中心", "Errors"),
        app.t(
            "保留错误分类与路径跳转，但避免把原始状态直接堆叠成噪声。",
            "Keep error categories and jump actions while reducing raw-text noise.",
        ),
    );
    ui.add_space(8.0);
    let mut user = 0usize;
    let mut transient = 0usize;
    let mut system = 0usize;
    for e in &app.errors {
        match e.kind {
            ErrorKind::User => user += 1,
            ErrorKind::Transient => transient += 1,
            ErrorKind::System => system += 1,
        }
    }
    ui.columns(3, |columns| {
        metric_card(
            &mut columns[0],
            app.t("用户", "User"),
            &format_count(user as u64),
            app.t("用户输入或权限问题", "Input or permission issues"),
            warning_amber(),
        );
        metric_card(
            &mut columns[1],
            app.t("瞬时", "Transient"),
            &format_count(transient as u64),
            app.t("可重试的瞬时失败", "Retryable transient failures"),
            info_blue(),
        );
        metric_card(
            &mut columns[2],
            app.t("系统", "System"),
            &format_count(system as u64),
            app.t("系统级故障", "System-level failures"),
            danger_red(),
        );
    });

    let filter_label = app.t("全部", "All").to_string();
    let user_filter_label = app.t("用户", "User").to_string();
    let transient_filter_label = app.t("瞬时", "Transient").to_string();
    let system_filter_label = app.t("系统", "System").to_string();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(app.t("过滤", "Filter"));
        ui.selectable_value(&mut app.error_filter, ErrorFilter::All, filter_label);
        ui.selectable_value(&mut app.error_filter, ErrorFilter::User, user_filter_label);
        ui.selectable_value(
            &mut app.error_filter,
            ErrorFilter::Transient,
            transient_filter_label,
        );
        ui.selectable_value(
            &mut app.error_filter,
            ErrorFilter::System,
            system_filter_label,
        );
    });

    let filtered: Vec<_> = app
        .errors
        .iter()
        .filter(|e| match app.error_filter {
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
                    let is_selected = app.selection.selected_path.as_deref() == Some(&e.path);
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
                                    app.select_path(&e.path, SelectionSource::Error);
                                }
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    if ui.button(app.t("选中查看", "Inspect")).clicked() {
                                        app.select_path(&e.path, SelectionSource::Error);
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
