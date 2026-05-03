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
    let live_folders_empty_body = app
        .t(
            "开始扫描后会显示占用空间最多的目录。",
            "Start a scan to see which directories consume the most space.",
        )
        .to_string();
    let live_files_empty_body = app
        .t(
            "开始扫描后会优先显示最值得检查的大文件。",
            "Start a scan to surface the largest files worth reviewing first.",
        )
        .to_string();
    ui.columns(2, |columns| {
        render_ranked_size_list(
            &mut columns[0],
            &live_folders_title,
            &live_folders_subtitle,
            &live_folders_empty_body,
            &ranked_dirs,
            app.summary.bytes_observed,
            &mut app.selection,
            &mut app.execution_report,
        );
        render_ranked_size_list(
            &mut columns[1],
            &live_files_title,
            &live_files_subtitle,
            &live_files_empty_body,
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
