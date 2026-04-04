use super::*;

pub(super) fn ui_current_scan(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    page_header(
            ui,
            app.t("DirOtter 工作台", "DirOtter Workspace"),
            app.t("实时扫描", "Live Scan"),
            app.t(
                "这里展示的是“扫描中已发现的最大项”，不是最终结果。内部性能指标已移到诊断页。",
                "This page shows the largest items discovered so far, not the final result. Internal performance counters have been moved to Diagnostics.",
            ),
        );
    ui.add_space(8.0);
    if app.scan_finalizing() {
        tone_banner(
                ui,
                app.t("正在整理最终结果", "Finalizing Final Results"),
                app.t(
                    "目录遍历已经结束，当前正在后台保存快照、写入历史并生成清理建议。界面应保持可操作，完成后会自动切到正常完成态。",
                    "Directory traversal has finished. DirOtter is now saving the snapshot, writing history, and preparing cleanup suggestions in the background. The UI should stay responsive and will switch to the normal completed state automatically.",
                ),
            );
        ui.add_space(10.0);
    } else if app.scan_active() {
        let current_path = app
            .scan_current_path
            .as_deref()
            .map(|path| truncate_middle(path, 84))
            .unwrap_or_else(|| {
                app.t("正在准备扫描路径…", "Preparing scan path...")
                    .to_string()
            });
        tone_banner(
                ui,
                app.t("这是实时增量视图", "This Is a Live Incremental View"),
                &format!(
                    "{} {}\n{}",
                    app.t(
                        "当前结果会持续更新，最终结论请以扫描完成后的概览页为准。正在处理：",
                        "Results keep updating while the scan runs. Use Overview after completion for the final summary. Working on:",
                    ),
                    current_path,
                    app.scan_health_summary()
                ),
            );
        ui.add_space(10.0);
    }

    ui.columns(5, |columns| {
        let cards = app.summary_cards();
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
    let ranked_dirs = app.current_ranked_dirs(12);
    let (live_files_title, live_files_subtitle, ranked_files) =
        app.contextual_ranked_files_panel(12);
    let live_folders_title = app
        .t("当前最大的文件夹", "Largest Folders Found So Far")
        .to_string();
    let live_folders_subtitle = app
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
            app.summary.bytes_observed,
            &mut app.selection,
            &mut app.execution_report,
        );
        render_ranked_size_list(
            &mut columns[1],
            &live_files_title,
            &live_files_subtitle,
            &ranked_files,
            app.summary.bytes_observed,
            &mut app.selection,
            &mut app.execution_report,
        );
    });

    ui.add_space(12.0);
    surface_panel(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(app.t("最近扫描到的文件", "Recently Scanned Files"))
                    .text_style(egui::TextStyle::Name("title".into())),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}",
                        format_count(app.live_files.len() as u64),
                        app.t("条", "rows")
                    ))
                    .color(ui.visuals().weak_text_color()),
                );
            });
        });
        ui.add_space(6.0);
        let rows = app.live_files.len();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show_rows(ui, 28.0, rows, |ui, row_range| {
                for row in row_range {
                    if let Some((path, size)) = app.live_files.get(row).cloned() {
                        let row_width = (ui.available_width() - 120.0).max(120.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [row_width, 24.0],
                                    egui::SelectableLabel::new(
                                        app.selection_matches_path(path.as_ref()),
                                        truncate_middle(path.as_ref(), 92),
                                    ),
                                )
                                .clicked()
                            {
                                app.select_path(path.as_ref(), SelectionSource::Table);
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

pub(super) fn ui_treemap(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    page_header(
            ui,
            app.t("DirOtter 工作台", "DirOtter Workspace"),
            app.t("结果视图", "Result View"),
            app.t(
                "这里只展示扫描完成后的结果，不跟实时扫描绑定。每次只看当前目录的直接子项，再按需逐层进入。",
                "This page only shows completed scan results. It is not bound to live scanning. Inspect one directory level at a time and drill in only when needed.",
            ),
        );
    ui.add_space(8.0);

    if app.scan_active() {
        tone_banner(
                ui,
                app.t("Treemap 不参与实时刷新", "Treemap Stays Out of Live Updates"),
                app.t(
                    "扫描完成后再生成结果视图，避免 UI 线程、海量节点和布局开销叠加卡顿。",
                    "The result view is generated only after scan completion, avoiding UI thread churn, huge node counts, and layout overhead piling up together.",
                ),
            );
        return;
    }

    if app.can_reload_result_store_from_cache() {
        app.ensure_store_loaded_from_cache();
    }

    let Some(scope) = app.treemap_focus_target() else {
        tone_banner(
                ui,
                app.t("还没有可用结果", "No Completed Result Yet"),
                app.t(
                    "先完成一次扫描后再使用这个结果视图。DirOtter 不会在这里自动载入旧缓存，避免把界面拖慢。",
                    "Complete a scan first before using this result view. DirOtter does not auto-load old cached results here, so the UI stays responsive.",
                ),
            );
        return;
    };

    let entries = app.treemap_entries(&scope.path, MAX_TREEMAP_CHILDREN);
    let selected_dir = app.selected_directory_target();
    let root_target = app
        .root_node_id()
        .and_then(|node_id| app.target_from_node_id(node_id));
    let scope_total = scope.size_bytes.max(1);

    tone_banner(
        ui,
        app.t("轻量结果视图", "Lightweight Result View"),
        &format!(
            "{} {}\n{}",
            app.t("当前目录：", "Current directory:"),
            truncate_middle(&scope.path, 88),
            app.t(
                "只展示直接子项，不递归整树，不做实时布局。",
                "Only direct children are shown. No whole-tree recursion and no live layout work.",
            )
        ),
    );
    ui.add_space(10.0);

    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                app.treemap_focus_path.is_some(),
                egui::Button::new(app.t("返回上级", "Up One Level")),
            )
            .clicked()
        {
            app.focus_treemap_parent();
        }

        if let Some(root) = root_target.clone() {
            if scope.path != root.path && ui.button(app.t("回到根目录", "Back to Root")).clicked()
            {
                app.treemap_focus_path = None;
                app.select_path(&root.path, SelectionSource::Treemap);
            }
        }

        if let Some(target) = selected_dir {
            if target.path != scope.path
                && ui
                    .button(app.t("跳到当前选中目录", "Use Selected Directory"))
                    .clicked()
            {
                app.focus_treemap_path(target.path);
            }
        }
    });

    ui.add_space(10.0);
    ui.columns(3, |columns| {
        compact_metric_block(
            &mut columns[0],
            app.t("当前层级体积", "Current Level Size"),
            &format_bytes(scope.size_bytes),
            app.t("作为本层占比基准", "Used as the local baseline"),
        );
        compact_metric_block(
            &mut columns[1],
            app.t("直接子项", "Direct Children"),
            &format_count(entries.len() as u64),
            app.t("只统计当前目录下一层", "Current directory only"),
        );
        compact_metric_block(
            &mut columns[2],
            app.t("显示上限", "Display Cap"),
            &format_count(MAX_TREEMAP_CHILDREN as u64),
            app.t("避免大目录压垮结果视图", "Keeps large folders responsive"),
        );
    });

    ui.add_space(12.0);
    if entries.is_empty() {
        tone_banner(
                ui,
                app.t("这一层没有可展示的子项", "No Children to Show at This Level"),
                app.t(
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
            egui::RichText::new(app.t("目录结果条形图", "Directory Result Bars"))
                .text_style(egui::TextStyle::Name("title".into())),
        );
        ui.label(
            egui::RichText::new(app.t(
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
                            let selected = app.selection_matches_treemap_entry(entry);
                            let label = format!(
                                "{} {}",
                                if matches!(entry.kind, NodeKind::Dir) {
                                    app.t("目录", "DIR")
                                } else {
                                    app.t("文件", "FILE")
                                },
                                truncate_middle(entry.name.as_ref(), 56)
                            );
                            let subtitle = match entry.kind {
                                NodeKind::Dir => format!(
                                    "{} {}  |  {} {}",
                                    format_count(entry.file_count),
                                    app.t("文件", "files"),
                                    format_count(entry.dir_count.saturating_sub(1)),
                                    app.t("子目录", "subdirs")
                                ),
                                NodeKind::File => app.t("文件项", "File item").to_string(),
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
                                        app.select_node(entry.node_id, SelectionSource::Treemap);
                                    }
                                    if matches!(entry.kind, NodeKind::Dir)
                                        && ui.button(app.t("进入下一层", "Open Level")).clicked()
                                    {
                                        app.focus_treemap_node(entry.node_id);
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
