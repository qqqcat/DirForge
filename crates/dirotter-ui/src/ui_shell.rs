use crate::{
    compact_stat_chip, danger_red, format_bytes, format_count, river_teal, sized_button,
    sized_danger_button, sized_primary_button, sized_selectable, stacked_stat_block, stat_row,
    status_badge, surface_panel, tone_banner, truncate_middle, AppStatus, CleanupCategory,
    CleanupDeleteConfirmAction, CleanupDeleteRequest, CleanupDetailsAction, DeleteConfirmAction,
    DeleteRequestScope, DirOtterNativeApp, DuplicateDeleteConfirmAction, Page,
    PendingDeleteConfirmation, SelectionOrigin, SelectionSource,
};
use dirotter_actions::ExecutionMode;
use eframe::egui;
use std::sync::atomic::Ordering;

pub(super) fn ui_nav(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.heading("DirOtter");
    ui.add_space(18.0);

    ui.label(
        egui::RichText::new(app.t("导航", "Navigation"))
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(6.0);

    for (p, label_zh, label_en) in [
        (Page::Dashboard, "概览", "Overview"),
        (Page::CurrentScan, "扫描进行中", "Live Scan"),
        (Page::Treemap, "结果视图", "Result View"),
        (Page::Duplicates, "重复文件", "Duplicate Files"),
        (Page::Settings, "偏好设置", "Settings"),
    ] {
        let selected = app.page == p;
        let text = egui::RichText::new(app.t(label_zh, label_en))
            .size(14.0)
            .strong();
        if ui
            .add_sized(
                [ui.available_width(), super::NAV_ITEM_HEIGHT],
                egui::SelectableLabel::new(selected, text),
            )
            .clicked()
        {
            app.page = p;
        }
    }

    if app.advanced_tools_enabled {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(app.t("高级工具", "Advanced Tools"))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(6.0);
        for (p, label_zh, label_en) in [
            (Page::Errors, "错误中心", "Errors"),
            (Page::Diagnostics, "诊断信息", "Diagnostics"),
        ] {
            let selected = app.page == p;
            let text = egui::RichText::new(app.t(label_zh, label_en))
                .size(14.0)
                .strong();
            if ui
                .add_sized(
                    [ui.available_width(), super::NAV_ITEM_HEIGHT],
                    egui::SelectableLabel::new(selected, text),
                )
                .clicked()
            {
                app.page = p;
            }
        }
    }
}

pub(super) fn ui_toolbar(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("DirOtter")
                .size(22.0)
                .strong()
                .color(ui.visuals().text_color()),
        );
        ui.add_space(10.0);
        let scanning = app.scan_session.is_some();
        let finalizing = app.scan_finalizing();
        status_badge(
            ui,
            app.status_text(),
            scanning || finalizing || app.delete_active(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let active = scanning;
            let deleting = app.delete_active();
            let stop_label = if app.scan_cancel_requested {
                app.t("正在停止", "Stopping")
            } else if active {
                app.t("停止扫描", "Stop Scan")
            } else if finalizing {
                app.t("整理中", "Finalizing")
            } else {
                app.t("取消", "Cancel")
            };
            if ui
                .add_enabled_ui(active && !app.scan_cancel_requested, |ui| {
                    sized_button(ui, 108.0, stop_label)
                })
                .inner
                .clicked()
            {
                if let Some(session) = &app.scan_session {
                    session.cancel.store(true, Ordering::SeqCst);
                    app.scan_cancel_requested = true;
                    app.status = AppStatus::Cancelled;
                    app.scan_current_path = None;
                }
            }
            let start_label = if active {
                app.t("扫描中", "Scanning")
            } else if finalizing {
                app.t("整理中", "Finalizing")
            } else if deleting {
                app.t("删除中", "Deleting")
            } else {
                app.t("开始扫描", "Start Scan")
            };
            if ui
                .add_enabled_ui(!active && !finalizing && !deleting, |ui| {
                    sized_button(ui, 108.0, start_label)
                })
                .inner
                .clicked()
            {
                app.start_scan();
            }
        });
    });
}

pub(super) fn ui_inspector(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    let selected_target = app.selected_target();
    let selected_target_view = selected_target
        .as_ref()
        .map(|target| app.inspector_target_view_model(target));
    let delete_task_view = app.delete_task_view_model();
    let inspector_actions_view = app.inspector_actions_view_model(selected_target.as_ref());
    let explorer_feedback_view = app.inspector_explorer_feedback_view_model();
    let delete_feedback_view = app.inspector_delete_feedback_view_model();
    let execution_report_view = app.inspector_execution_report_view_model();
    let memory_status_view = app.inspector_memory_status_view_model();
    let maintenance_feedback_view = app.inspector_maintenance_feedback_view_model();
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(app.t("检查器", "Inspector"))
            .text_style(egui::TextStyle::Name("title".into())),
    );
    ui.label(
        egui::RichText::new(app.t("当前聚焦对象详情", "Details for the current selection"))
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
                        app.t("名称", "Name"),
                        target.name_value.as_ref(),
                        target.name_hint,
                    );
                    stat_row(
                        ui,
                        app.t("路径", "Path"),
                        &target.path_value,
                        target.path_hint,
                    );
                    stat_row(
                        ui,
                        app.t("大小", "Size"),
                        &target.size_value,
                        &target.size_hint,
                    );
                } else {
                    ui.label(app.t(
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
                        app.t("目标", "Target"),
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
                        app.t("已耗时", "Elapsed"),
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
                    egui::RichText::new(app.t("快速操作", "Quick Actions"))
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
                            app.open_path_location(target.path.as_ref());
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
                            app.queue_delete_for_target(target, ExecutionMode::FastPurge);
                        }
                    }
                    if ui
                        .add_enabled_ui(inspector_actions_view.can_recycle, |ui| {
                            sized_button(ui, ui.available_width(), &inspector_actions_view.recycle_label)
                        })
                        .inner
                        .clicked()
                    {
                        app.execute_selected_delete(ExecutionMode::RecycleBin);
                    }
                    if ui
                        .add_enabled_ui(inspector_actions_view.can_permanent_delete, |ui| {
                            sized_danger_button(
                                ui,
                                ui.available_width(),
                                &inspector_actions_view.permanent_label,
                            )
                        })
                        .inner
                        .clicked()
                    {
                        if let Some(target) = selected_target.clone() {
                            app.pending_delete_confirmation = Some(PendingDeleteConfirmation {
                                request: DirOtterNativeApp::delete_request_for_target(
                                    target.clone(),
                                    app.selection_origin(),
                                ),
                                risk: app.risk_for_path(target.path.as_ref()),
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
                        app.start_system_memory_release();
                    }
                });
                if let Some(message) = explorer_feedback_view.as_ref() {
                    ui.add_space(8.0);
                    tone_banner(ui, &message.title, &message.message);
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
                            [ui.available_width(), super::CONTROL_HEIGHT],
                            egui::Button::new(label),
                        );
                        let response = if let Some(hint) = report.failure_detail_hint.as_ref() {
                            response.on_hover_text(hint)
                        } else {
                            response
                        };
                        if response.clicked() {
                            app.set_execution_failure_details_open(true);
                        }
                    }
                }
            });

            ui.add_space(10.0);
            surface_panel(ui, |ui| {
                ui.label(
                    egui::RichText::new(app.t("一键释放系统内存", "Release System Memory"))
                        .text_style(egui::TextStyle::Name("title".into())),
                );
                ui.add_space(10.0);
                if let Some(system_free) = memory_status_view.system_free_value.as_ref() {
                    ui.label(egui::RichText::new(system_free).size(28.0).strong());
                    ui.add_space(8.0);
                }

                if let Some(load_value) = memory_status_view.load_value.as_ref() {
                    stat_row(ui, app.t("内存负载", "load"), load_value, "");
                    ui.add_space(6.0);
                }
                if let Some(process_memory) = memory_status_view.process_working_set_value.as_ref() {
                    stat_row(ui, "DirOtter", process_memory, "");
                }

                if let Some(active_message) = memory_status_view.active_message.as_ref() {
                    ui.add_space(10.0);
                    tone_banner(
                        ui,
                        app.t("一键释放系统内存", "Release System Memory"),
                        active_message,
                    );
                }

                if let Some(delta) = memory_status_view.release_delta_value.as_ref() {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(app.t("最近执行", "Last Action"))
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(6.0);
                    stacked_stat_block(
                        ui,
                        app.t("系统可用内存增加约", "System free memory increased by about"),
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

pub(super) fn ui_delete_confirm_dialog(app: &mut DirOtterNativeApp, ctx: &egui::Context) {
    let Some(pending) = app.pending_delete_confirmation.clone() else {
        return;
    };
    let Some(view_model) = app.delete_confirmation_view_model(&pending) else {
        app.pending_delete_confirmation = None;
        return;
    };

    let mut keep_open = true;
    let mut actions = Vec::new();
    egui::Window::new(app.t("确认永久删除", "Confirm Permanent Delete"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.label(egui::RichText::new(view_model.intro).strong());
            ui.add_space(8.0);
            stat_row(
                ui,
                app.t("目标", "Target"),
                &view_model.target_value,
                view_model.target_hint,
            );
            stat_row(
                ui,
                app.t("大小", "Size"),
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
                if sized_danger_button(ui, 150.0, app.t("确认永久删除", "Delete Permanently"))
                    .clicked()
                {
                    actions.push(DeleteConfirmAction::Confirm);
                }
                if ui.button(app.t("取消", "Cancel")).clicked() {
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
        app.handle_delete_confirm_action(pending.request.clone(), action);
    }
    if !keep_open {
        app.pending_delete_confirmation = None;
    }
}

pub(super) fn ui_cleanup_details_window(app: &mut DirOtterNativeApp, ctx: &egui::Context) {
    let Some(category) = app.cleanup.detail_category else {
        return;
    };
    let items = app.cleanup_items_for_category(category);
    let view_model = app.cleanup_details_window_view_model(category, &items);
    let mut keep_open = true;
    let mut actions = Vec::new();
    let screen_size = ctx.input(|i| i.screen_rect().size());
    egui::Window::new(app.t("清理建议详情", "Cleanup Details"))
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
                    app.t("已选项目", "Selected"),
                    &view_model.selected_count_value,
                );
                compact_stat_chip(
                    ui,
                    app.t("预计释放", "Estimated Reclaim"),
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
                        sized_danger_button(ui, 156.0, &view_model.permanent_label)
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
                                .add_enabled_ui(item.enabled, |ui| ui.checkbox(&mut checked, ""))
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
                                actions
                                    .push(CleanupDetailsAction::FocusTarget(item.target.clone()));
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
                            ui.colored_label(app.cleanup_risk_color(item.risk), "●");
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
        app.handle_cleanup_details_action(category, action);
    }
    if !keep_open {
        app.cleanup.detail_category = None;
    }
}

pub(super) fn handle_cleanup_details_action(
    app: &mut DirOtterNativeApp,
    category: CleanupCategory,
    action: CleanupDetailsAction,
) {
    match action {
        CleanupDetailsAction::SelectCategory(category) => {
            app.cleanup.detail_category = Some(category);
        }
        CleanupDetailsAction::ToggleTarget { path, checked } => {
            if checked {
                app.cleanup.selected_paths.insert(path);
            } else {
                app.cleanup.selected_paths.remove(path.as_ref());
            }
        }
        CleanupDetailsAction::FocusTarget(target) => {
            if let Some(node_id) = target.node_id {
                app.select_node(node_id, SelectionSource::Table);
            } else {
                app.select_path(target.path.as_ref(), SelectionSource::Table);
            }
        }
        CleanupDetailsAction::SelectAllSafe => app.select_all_safe_cleanup_items(category),
        CleanupDetailsAction::ClearSelected => app.clear_selected_cleanup_items(category),
        CleanupDetailsAction::OpenSelectedLocation => {
            app.open_selected_cleanup_target_location();
        }
        CleanupDetailsAction::TriggerPrimary => app.trigger_cleanup_details_primary(category),
        CleanupDetailsAction::TriggerPermanent => {
            app.queue_cleanup_category_delete_with_mode(category, ExecutionMode::Permanent);
        }
        CleanupDetailsAction::Close => {}
    }
}

pub(super) fn handle_delete_confirm_action(
    app: &mut DirOtterNativeApp,
    request: DeleteRequestScope,
    action: DeleteConfirmAction,
) {
    match action {
        DeleteConfirmAction::Confirm => {
            app.queue_delete_request(request, ExecutionMode::Permanent);
        }
        DeleteConfirmAction::Close => {}
    }
}

pub(super) fn handle_cleanup_delete_confirm_action(
    app: &mut DirOtterNativeApp,
    request: CleanupDeleteRequest,
    action: CleanupDeleteConfirmAction,
) {
    match action {
        CleanupDeleteConfirmAction::Confirm => {
            app.queue_delete_request(
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

pub(super) fn ui_cleanup_delete_confirm_dialog(app: &mut DirOtterNativeApp, ctx: &egui::Context) {
    let Some(request) = app.cleanup.pending_delete.clone() else {
        return;
    };
    let view_model = app.cleanup_delete_confirmation_view_model(&request);

    let mut keep_open = true;
    let mut actions = Vec::new();
    let screen_size = ctx.input(|i| i.screen_rect().size());
    egui::Window::new(app.t("一键清理确认", "Confirm Cleanup"))
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
                app.t("任务", "Task"),
                &view_model.task_value,
                view_model.task_hint,
            );
            stat_row(
                ui,
                app.t("项目数", "Items"),
                &view_model.item_count_value,
                view_model.item_count_hint,
            );
            stat_row(
                ui,
                app.t("预计释放", "Estimated Reclaim"),
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
                                egui::Label::new(egui::RichText::new(&item.path_value).monospace())
                                    .wrap(),
                            );
                        });
                        ui.add_space(8.0);
                    }
                });
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled_ui(!app.delete_active(), |ui| {
                        sized_primary_button(ui, 150.0, view_model.confirm_label)
                    })
                    .inner
                    .clicked()
                {
                    actions.push(CleanupDeleteConfirmAction::Confirm);
                }
                if ui.button(app.t("取消", "Cancel")).clicked() {
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
        handle_cleanup_delete_confirm_action(app, request.clone(), action);
    }
    if !keep_open {
        app.cleanup.pending_delete = None;
    }
}

pub(super) fn ui_duplicate_delete_confirm_dialog(app: &mut DirOtterNativeApp, ctx: &egui::Context) {
    let Some(request) = app.duplicates.pending_delete.clone() else {
        return;
    };

    let mut keep_open = true;
    let mut actions = Vec::new();
    egui::Window::new(app.t("一键清理确认", "Confirm Cleanup"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(460.0);
            ui.label(
                egui::RichText::new(app.t(
                    "建议：日常清理优先移到回收站。只有在你非常确定时才使用永久删除。",
                    "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
                ))
                .strong(),
            );
            ui.add_space(10.0);
            stat_row(
                ui,
                app.t("项目数", "Items"),
                &format_count(request.targets.len() as u64),
                "",
            );
            stat_row(
                ui,
                app.t("任务", "Task"),
                &format_count(request.group_count as u64),
                "",
            );
            stat_row(
                ui,
                app.t("预计释放", "Estimated Reclaim"),
                &format_bytes(request.estimated_bytes),
                app.t(
                    "建议：日常清理优先移到回收站。只有在你非常确定时才使用永久删除。",
                    "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
                ),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(app.t(
                    "建议：日常清理优先移到回收站。只有在你非常确定时才使用永久删除。",
                    "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled_ui(!app.delete_active(), |ui| {
                        sized_danger_button(ui, 120.0, app.t("永久删除", "Delete Permanently"))
                    })
                    .inner
                    .clicked()
                {
                    actions.push(DuplicateDeleteConfirmAction::Permanent);
                }
                if ui
                    .add_enabled(
                        !app.delete_active(),
                        egui::Button::new(app.t("移到回收站", "Move to Recycle Bin")),
                    )
                    .clicked()
                {
                    actions.push(DuplicateDeleteConfirmAction::RecycleBin);
                }
                if ui.button(app.t("取消", "Cancel")).clicked() {
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
        app.handle_duplicate_delete_confirm_action(action);
    }
    if !keep_open {
        app.duplicates.pending_delete = None;
    }
}

pub(super) fn ui_execution_failure_details_dialog(
    app: &mut DirOtterNativeApp,
    ctx: &egui::Context,
) {
    if !app.execution_failure_details_open() {
        return;
    }
    let Some(view_model) = app.execution_failure_details_view_model() else {
        app.set_execution_failure_details_open(false);
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
                                            (ui.available_width() - button_width - 8.0).max(180.0);
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
                                                [button_width, super::CONTROL_HEIGHT],
                                                egui::Button::new(&view_model.open_location_label),
                                            )
                                            .clicked()
                                        {
                                            app.open_path_location(&item.path_value);
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
        app.set_execution_failure_details_open(false);
    }
}

pub(super) fn ui_statusbar(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} {}  |  {} {}  |  {} {}  |  {} {}",
                format_count(app.summary.scanned_files),
                app.t("文件", "files"),
                format_count(app.summary.scanned_dirs),
                app.t("目录", "dirs"),
                format_bytes(app.summary.bytes_observed),
                app.t("扫描体积", "scanned"),
                format_count(app.summary.error_count),
                app.t("错误", "errors")
            ))
            .text_style(egui::TextStyle::Small),
        );
        if let Some(volume) = app.current_volume_info() {
            let used = volume.total_bytes.saturating_sub(volume.available_bytes);
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "{} {} / {} {}",
                    format_bytes(used),
                    app.t("已用", "used"),
                    format_bytes(volume.total_bytes),
                    app.t("总量", "total")
                ))
                .text_style(egui::TextStyle::Small),
            );
        }
        if app.scan_active() {
            ui.separator();
            ui.label(
                egui::RichText::new(app.scan_health_short())
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
        }
        if let Some(session) = app.delete_session.as_ref() {
            let snapshot = session.snapshot();
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "{} {:.1}s  |  {}  |  {} {}",
                    app.t("删除中", "Deleting"),
                    snapshot.started_at.elapsed().as_secs_f32(),
                    truncate_middle(&snapshot.label, 32),
                    format_count(snapshot.target_count as u64),
                    app.t("项", "items")
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
        }
    });
}

pub(super) fn ui_delete_activity_banner(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    if let Some(session) = app.delete_session.as_ref() {
        let snapshot = session.snapshot();
        let phase = snapshot.started_at.elapsed().as_secs_f32();
        let pulse = ((phase.sin() + 1.0) * 0.5 * 0.7 + 0.15).clamp(0.08, 0.92);

        tone_banner(
            ui,
            match snapshot.mode {
                ExecutionMode::RecycleBin => {
                    app.t("正在后台移到回收站", "Moving to Recycle Bin in Background")
                }
                ExecutionMode::FastPurge => {
                    app.t("正在后台释放空间", "Reclaiming Space in Background")
                }
                ExecutionMode::Permanent => {
                    app.t("正在后台永久删除", "Deleting Permanently in Background")
                }
            },
            &format!(
                "{}  |  {} / {}  |  {} {} / {} {}  |  {} {:.1}s  |  {}",
                truncate_middle(&snapshot.label, 56),
                format_count(snapshot.completed_count as u64),
                format_count(snapshot.target_count as u64),
                format_count(snapshot.succeeded_count as u64),
                app.t("成功", "succeeded"),
                format_count(snapshot.failed_count as u64),
                app.t("失败", "failed"),
                app.t("已耗时", "Elapsed"),
                phase,
                app.t(
                    "你可以继续浏览扫描结果，删除完成后界面会自动同步。",
                    "You can keep browsing scan results. The UI will synchronize automatically when deletion finishes.",
                )
            ),
        );
        ui.add_space(6.0);
        ui.add(
            egui::ProgressBar::new(pulse)
                .desired_width(ui.available_width().max(220.0))
                .text(app.t(
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

    let Some(snapshot) = app
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
        app.t("正在同步删除结果", "Synchronizing Cleanup Results"),
        &format!(
            "{}  |  {} {} / {} {}  |  {} {:.1}s  |  {}",
            truncate_middle(&snapshot.label, 56),
            format_count(snapshot.succeeded_count as u64),
            app.t("成功", "succeeded"),
            format_count(snapshot.failed_count as u64),
            app.t("失败", "failed"),
            app.t("已耗时", "Elapsed"),
            phase,
            app.t(
                "删除已经完成，正在后台整理结果视图与清理建议。",
                "Deletion finished. The result view and cleanup suggestions are being synchronized in the background.",
            )
        ),
    );
    ui.add_space(6.0);
    ui.add(
        egui::ProgressBar::new(pulse)
            .desired_width(ui.available_width().max(220.0))
            .text(app.t(
                "系统正在同步删除后的结果",
                "System is synchronizing post-delete results",
            )),
    );
}
