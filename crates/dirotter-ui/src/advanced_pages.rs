use super::*;

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
