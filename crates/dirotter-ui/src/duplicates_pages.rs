use super::*;

pub(super) fn ui_duplicates(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    ui.set_width(ui.available_width());
    ui.set_max_width(ui.available_width());
    page_header(
        ui,
        app.t("DirOtter 工作台", "DirOtter Workspace"),
        app.t("重复文件", "Duplicate Files"),
        app.t(
            "目标不是列出重复文件，而是让你敢于删除它们。每组至少保留一个副本，默认按推荐保留项自动决策。",
            "The goal is not to list duplicates. It is to make deletion safe: every group keeps one copy and the default selection follows the recommended keeper.",
        ),
    );
    ui.add_space(8.0);

    if app.scan_active() {
        tone_banner(
            ui,
            app.t("扫描完成后再审阅重复文件", "Review Duplicates After Scan Completion"),
            app.t(
                "重复文件审阅依赖最终结果快照。请先完成扫描，避免在实时增量阶段做删除决策。",
                "Duplicate review relies on the completed snapshot. Finish the scan first so deletion decisions are not made from live incremental results.",
            ),
        );
        return;
    }

    if app.delete_active() && app.store.is_none() {
        tone_banner(
            ui,
            app.t("重复文件页面等待结果同步", "Duplicate Review Is Waiting For Result Sync"),
            app.t(
                "后台删除或结果同步仍在进行。同步完成后会自动恢复重复文件分组。",
                "Background deletion or result synchronization is still running. Duplicate groups will return automatically after the sync completes.",
            ),
        );
        return;
    }

    if app.can_reload_result_store_from_cache() {
        app.begin_result_store_load_if_needed();
    }

    if app.result_store_load_active() {
        tone_banner(
            ui,
            app.t("正在后台载入结果快照", "Loading Saved Result Snapshot"),
            app.t(
                "重复文件页面会在结果快照准备好之后再开始后台校验。",
                "The duplicate review will begin background verification after the saved result snapshot is ready.",
            ),
        );
        return;
    }

    if app.store.is_none() {
        tone_banner(
            ui,
            app.t("还没有可用结果", "No Completed Result Yet"),
            app.t(
                "先完成一次扫描后再进入重复文件审阅。",
                "Complete a scan first before opening duplicate review.",
            ),
        );
        return;
    }

    app.start_duplicate_scan_if_needed();

    if let Some(session) = app.duplicate_scan_session.as_ref() {
        let snapshot = session.snapshot();
        let progress = if snapshot.candidate_groups_total == 0 {
            app.t("正在整理候选分组…", "Preparing candidate groups...")
                .to_string()
        } else {
            format!(
                "{} / {}  |  {} {}",
                format_count(snapshot.candidate_groups_processed as u64),
                format_count(snapshot.candidate_groups_total as u64),
                format_count(snapshot.groups_found as u64),
                app.t("个重复组已确认", "verified duplicate groups")
            )
        };
        tone_banner(
            ui,
            app.t("后台正在做重复文件校验", "Duplicate Verification Is Running in Background"),
            &format!(
                "{} {}",
                app.t(
                    "先按大小分组，再逐步补算哈希确认完全相同的文件。",
                    "The page groups by size first, then incrementally verifies full matches with hashes.",
                ),
                progress
            ),
        );
        ui.add_space(10.0);
    }

    let (selected_groups, selected_files, selected_bytes) = app.duplicate_delete_totals();
    let total_duplicate_files: usize = app
        .duplicates
        .groups
        .iter()
        .map(|group| group.files.len())
        .sum();
    let total_waste: u64 = app
        .duplicates
        .groups
        .iter()
        .map(|group| group.total_waste)
        .sum();

    surface_panel(ui, |ui| {
        ui.columns(3, |columns| {
            compact_metric_block(
                &mut columns[0],
                app.t("可释放空间", "Reclaimable Space"),
                &format_bytes(total_waste),
                app.t(
                    "只统计每组删去重复副本后可回收的空间",
                    "Waste beyond one keeper per group",
                ),
            );
            compact_metric_block(
                &mut columns[1],
                app.t("重复文件数", "Duplicate Files"),
                &format_count(total_duplicate_files as u64),
                app.t("所有重复副本总数", "All files inside duplicate groups"),
            );
            compact_metric_block(
                &mut columns[2],
                app.t("重复组数", "Duplicate Groups"),
                &format_count(app.duplicates.groups.len() as u64),
                app.t(
                    "按组决策，而不是逐个文件决策",
                    "Operate on groups, not isolated files",
                ),
            );
        });
    });

    ui.add_space(12.0);
    let auto_select_label = app.t("自动选择建议", "Auto Select Suggested");
    let clear_selection_label = app.t("清空选择", "Clear Selection");
    let delete_selected_label = app.t("删除选中", "Delete Selected");
    let large_only_label = app.t("只看大文件", "Large Files Only");
    let sort_label = app.t("排序", "Sort");
    let sort_waste_label = app.t("按可释放空间", "By Reclaimable Space");
    let sort_size_label = app.t("按文件大小", "By File Size");
    let expand_all_label = app.t("展开全部", "Expand All");
    let operate_groups_label = app.t(
        "按组决策，而不是逐个文件决策",
        "Operate on groups, not isolated files",
    );
    let selected_groups_label = app.t("组已加入删除计划", "groups selected");
    let selected_files_label = app.t("个文件待删除", "files to delete");
    let estimated_reclaim_label = app.t("预计释放", "estimated reclaim");
    surface_panel(ui, |ui| {
        dashboard_split(
            ui,
            360.0,
            16.0,
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!app.delete_active(), egui::Button::new(auto_select_label))
                        .clicked()
                    {
                        app.reset_duplicate_selection_to_recommended();
                    }
                    if ui
                        .add_enabled(
                            !app.delete_active(),
                            egui::Button::new(clear_selection_label),
                        )
                        .clicked()
                    {
                        app.clear_duplicate_selection();
                    }
                    if ui
                        .add_enabled(
                            selected_files > 0 && !app.delete_active(),
                            egui::Button::new(delete_selected_label),
                        )
                        .clicked()
                    {
                        app.queue_duplicate_delete_review();
                    }
                });

                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut app.duplicates.show_large_only, large_only_label);

                    let combo_width = 240.0_f32.min(ui.available_width().max(160.0));
                    ui.allocate_ui_with_layout(
                        egui::vec2(combo_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::ComboBox::from_label(sort_label)
                                .width((combo_width - 24.0).max(120.0))
                                .selected_text(
                                    match app.duplicates.sort.unwrap_or(DuplicateSort::Waste) {
                                        DuplicateSort::Waste => sort_waste_label,
                                        DuplicateSort::Size => sort_size_label,
                                    },
                                )
                                .show_ui(ui, |ui| {
                                    let mut changed = false;
                                    let sort =
                                        app.duplicates.sort.get_or_insert(DuplicateSort::Waste);
                                    changed |= ui
                                        .selectable_value(
                                            sort,
                                            DuplicateSort::Waste,
                                            sort_waste_label,
                                        )
                                        .clicked();
                                    changed |= ui
                                        .selectable_value(
                                            sort,
                                            DuplicateSort::Size,
                                            sort_size_label,
                                        )
                                        .clicked();
                                    if changed {
                                        app.sort_duplicate_groups();
                                    }
                                });
                        },
                    );

                    if ui.button(expand_all_label).clicked() {
                        app.duplicates.expanded_group_ids =
                            app.duplicates.groups.iter().map(|group| group.id).collect();
                    }
                });
            },
            |ui| {
                ui.label(
                    egui::RichText::new(operate_groups_label)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    compact_stat_chip(
                        ui,
                        selected_groups_label,
                        &format_count(selected_groups as u64),
                    );
                    compact_stat_chip(
                        ui,
                        selected_files_label,
                        &format_count(selected_files as u64),
                    );
                    compact_stat_chip(ui, estimated_reclaim_label, &format_bytes(selected_bytes));
                });
            },
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!(
                "{} {}  |  {} {}  |  {} {}",
                format_count(selected_groups as u64),
                selected_groups_label,
                format_count(selected_files as u64),
                selected_files_label,
                format_bytes(selected_bytes),
                estimated_reclaim_label
            ))
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
        );
    });

    ui.add_space(12.0);
    let groups: Vec<_> = app
        .duplicates
        .groups
        .iter()
        .filter(|group| !app.duplicates.show_large_only || group.size >= 256 * 1024 * 1024)
        .cloned()
        .collect();

    let list_height = ui.available_height().max(220.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), list_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_min_height(list_height);
            if groups.is_empty() {
                empty_state_panel(
                    ui,
                    app.t("没有重复文件组", "No Duplicate Groups"),
                    app.t(
                        "如果这里没有结果，要么当前快照里没有重复文件，要么后台校验还在进行。",
                        "Either the current snapshot has no duplicates, or the background verification is still running.",
                    ),
                );
                return;
            }

            let visible_count = app.duplicates.visible_groups.min(groups.len()).max(1);
            let mut load_more = false;
            egui::ScrollArea::vertical()
                .max_height(list_height)
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for (index, group) in groups.iter().take(visible_count).enumerate() {
                        render_duplicate_group_card(app, ui, group);
                        ui.add_space(10.0);

                        if index + 1 == visible_count && visible_count < groups.len() {
                            load_more = true;
                        }
                    }
                });

            if load_more && app.duplicates.visible_groups < groups.len() {
                app.duplicates.visible_groups =
                    (app.duplicates.visible_groups + 20).min(groups.len());
                app.egui_ctx.request_repaint();
            }
        },
    );
}

fn render_duplicate_group_card(
    app: &mut DirOtterNativeApp,
    ui: &mut egui::Ui,
    group: &dirotter_dup::DuplicateGroup,
) {
    let selection = app.duplicate_group_selection(group);
    let expanded = app.duplicates.expanded_group_ids.contains(&group.id);
    let recommended = group.files.get(group.recommended_keep_index).cloned();
    let group_title = format!(
        "{} #{}  |  {} {}",
        app.t("组", "Group"),
        group.id,
        format_bytes(group.total_waste),
        app.t("可释放", "reclaimable")
    );

    surface_panel(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            let disclosure = if expanded { "▼" } else { "▶" };
            if ui.button(disclosure).clicked() {
                if expanded {
                    app.duplicates.expanded_group_ids.remove(&group.id);
                } else {
                    app.duplicates.expanded_group_ids.insert(group.id);
                }
            }
            ui.label(egui::RichText::new(group_title).strong());
            ui.separator();
            risk_chip(
                ui,
                app.cleanup_risk_label(group.risk),
                app.cleanup_risk_color(group.risk),
            );
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "{}  |  {} {}",
                    format_bytes(group.size),
                    format_count(group.files.len() as u64),
                    app.t("个副本", "copies")
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        if let Some(recommended) = recommended.as_ref() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "{} {}",
                        app.t("推荐保留：", "Recommended keep:"),
                        truncate_middle(&recommended.path, 104)
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(river_teal()),
                )
                .wrap(),
            );
            ui.add_space(4.0);
        }

        ui.add(
            egui::Label::new(
                egui::RichText::new(app.t(
                    "每组至少保留一个文件。高风险组默认不自动加入删除计划。",
                    "Each group keeps at least one file. High-risk groups are not auto-selected for deletion.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            )
            .wrap(),
        );

        ui.add_space(8.0);
        let mut enabled = selection.enabled;
        if ui
            .checkbox(
                &mut enabled,
                app.t(
                    "删除本组的非保留副本",
                    "Delete non-keeper files in this group",
                ),
            )
            .changed()
        {
            app.set_duplicate_group_enabled(group.id, enabled);
        }

        if expanded {
            ui.add_space(10.0);
            for file in &group.files {
                render_duplicate_file_row(app, ui, group.id, &selection.keep_path, file);
                ui.add_space(6.0);
            }
        }
    });
}

fn render_duplicate_file_row(
    app: &mut DirOtterNativeApp,
    ui: &mut egui::Ui,
    group_id: u64,
    keep_path: &Arc<str>,
    file: &dirotter_dup::DuplicateFileEntry,
) {
    let is_keep = keep_path.as_ref() == file.path;
    let (location_label, location_color) = duplicate_location_badge(app, file.location);
    surface_panel(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            if ui.radio(is_keep, "").clicked() {
                app.set_duplicate_group_keep_path(group_id, Arc::<str>::from(file.path.clone()));
            }

            let action_width = 120.0;
            let size_width = 84.0;
            let path_width = (ui.available_width() - action_width - size_width - 56.0).max(220.0);
            if ui
                .add_sized(
                    [path_width, CONTROL_HEIGHT],
                    egui::SelectableLabel::new(
                        app.selection_matches_path(&file.path),
                        truncate_middle(&file.path, 108),
                    ),
                )
                .clicked()
            {
                app.select_path(&file.path, SelectionSource::Duplicate);
            }

            if ui
                .add_sized(
                    [action_width, CONTROL_HEIGHT],
                    egui::Button::new(app.t("打开所在位置", "Open Location")),
                )
                .clicked()
            {
                app.open_duplicate_file_location(&file.path);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_sized(
                    [size_width, CONTROL_HEIGHT],
                    egui::Label::new(egui::RichText::new(format_bytes(file.size)).strong()),
                );
            });
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            risk_chip(ui, location_label, location_color);
            if file.hidden {
                risk_chip(
                    ui,
                    app.t("隐藏", "Hidden"),
                    egui::Color32::from_rgb(0x7C, 0x86, 0x8D),
                );
            }
            if file.system {
                risk_chip(ui, app.t("系统", "System"), danger_red());
            }
            ui.label(
                egui::RichText::new(format!(
                    "{} {}  |  {} {}",
                    app.t("修改时间", "Modified"),
                    duplicate_modified_label(app, file.modified_unix_secs),
                    app.t("保留评分", "Keep score"),
                    file.keep_score
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
        });
    });
}

fn duplicate_location_badge(
    app: &DirOtterNativeApp,
    location: dirotter_dup::DuplicateLocation,
) -> (&'static str, egui::Color32) {
    match location {
        dirotter_dup::DuplicateLocation::Documents => {
            (app.t("Documents", "Documents"), success_green())
        }
        dirotter_dup::DuplicateLocation::Downloads => (
            app.t("Downloads", "Downloads"),
            egui::Color32::from_rgb(0xD9, 0xA4, 0x41),
        ),
        dirotter_dup::DuplicateLocation::Desktop => (
            app.t("Desktop", "Desktop"),
            egui::Color32::from_rgb(0x4D, 0x9C, 0xD3),
        ),
        dirotter_dup::DuplicateLocation::ProgramFiles => {
            (app.t("Program Files", "Program Files"), danger_red())
        }
        dirotter_dup::DuplicateLocation::Windows => (app.t("Windows", "Windows"), danger_red()),
        dirotter_dup::DuplicateLocation::Temp => (
            app.t("Temp", "Temp"),
            egui::Color32::from_rgb(0xAA, 0x7A, 0x39),
        ),
        dirotter_dup::DuplicateLocation::Cache => (app.t("Cache", "Cache"), river_teal()),
        dirotter_dup::DuplicateLocation::AppData => (
            app.t("AppData", "AppData"),
            egui::Color32::from_rgb(0x8E, 0x87, 0xB8),
        ),
        dirotter_dup::DuplicateLocation::UserData => (
            app.t("User Folder", "User Folder"),
            egui::Color32::from_rgb(0x66, 0x9E, 0x7A),
        ),
        dirotter_dup::DuplicateLocation::Other => (
            app.t("Other", "Other"),
            egui::Color32::from_rgb(0x7C, 0x86, 0x8D),
        ),
    }
}

fn duplicate_modified_label(app: &DirOtterNativeApp, modified_unix_secs: Option<u64>) -> String {
    let Some(modified) = modified_unix_secs else {
        return app.t("未知", "Unknown").to_string();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age_days = now.saturating_sub(modified) / 86_400;
    if age_days == 0 {
        app.t("今天", "Today").to_string()
    } else {
        format!("{} {}", age_days, app.t("天前", "days ago"))
    }
}

fn risk_chip(ui: &mut egui::Ui, label: &str, color: egui::Color32) {
    let frame = egui::Frame::default()
        .fill(color.linear_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(8.0, 4.0));
    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(label)
                .text_style(egui::TextStyle::Small)
                .color(color),
        );
    });
}
