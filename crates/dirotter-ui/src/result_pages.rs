use super::*;

struct RankedTreemapEntry {
    path: Arc<str>,
    name: String,
    size_bytes: u64,
    kind: NodeKind,
}

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

    if app.delete_active() && app.store.is_none() {
        tone_banner(
            ui,
            app.t("结果视图等待删除同步完成", "Result View Is Waiting For Cleanup Sync"),
            app.t(
                "后台删除或结果同步仍在进行。DirOtter 会在同步完成后恢复结果视图，避免把快照载入和结果重建压回 UI 主线程。",
                "Background deletion or result synchronization is still running. DirOtter will resume the result view after it finishes so snapshot loading and result rebuilding do not block the UI thread.",
            ),
        );
        return;
    }

    if app.result_store_load_active() {
        tone_banner(
            ui,
            app.t("正在后台载入结果快照", "Loading Saved Result Snapshot"),
            app.t(
                "DirOtter 正在后台载入已保存的结果快照。准备完成后会自动打开轻量结果视图，不会在当前帧里同步解压或重建整棵结果树。",
                "DirOtter is loading the saved result snapshot in the background. The lightweight result view will open automatically when it is ready, without decompressing or rebuilding the whole result tree on the current UI frame.",
            ),
        );
        return;
    }

    if app.store.is_none() {
        ui_lightweight_treemap_from_rankings(app, ui);
        return;
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
            egui::RichText::new(app.t("目录空间 Treemap", "Directory Space Treemap"))
                .text_style(egui::TextStyle::Name("title".into())),
        );
        ui.label(
            egui::RichText::new(app.t(
                "点击矩形可选中对象；点击目录矩形会进入下一层。",
                "Click a rectangle to select an item. Directory rectangles drill into the next level.",
            ))
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(8.0);

        draw_treemap_result_blocks(app, ui, &entries, scope_total);
    });
}

fn ui_lightweight_treemap_from_rankings(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    let mut entries = Vec::new();
    entries.extend(
        app.current_ranked_dirs(48)
            .into_iter()
            .map(|(path, size_bytes)| ranked_treemap_entry(path, size_bytes, NodeKind::Dir)),
    );
    entries.extend(
        app.current_ranked_files(48)
            .into_iter()
            .map(|(path, size_bytes)| ranked_treemap_entry(path, size_bytes, NodeKind::File)),
    );
    entries.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.path.as_ref().cmp(b.path.as_ref()))
    });
    entries.truncate(96);

    if entries.is_empty() {
        tone_banner(
            ui,
            app.t("还没有可用结果", "No Completed Result Yet"),
            app.t(
                "扫描完成后会先保留轻量 Top-N 结果。当前没有可展示的目录或文件。",
                "After a scan finishes, DirOtter keeps a lightweight Top-N result. There are no folders or files to show right now.",
            ),
        );
        return;
    }

    let total = app
        .summary
        .bytes_observed
        .max(entries.iter().map(|entry| entry.size_bytes).sum::<u64>());
    tone_banner(
        ui,
        app.t("轻量 Treemap", "Lightweight Treemap"),
        app.t(
            "这里直接使用扫描完成时保留的 Top-N 结果，不自动载入完整结果树，因此切换页面不会触发大内存整理。",
            "This view uses the Top-N result kept after scan completion and does not auto-load the full result tree, so switching pages does not trigger heavy memory work.",
        ),
    );
    ui.add_space(12.0);
    ui.columns(3, |columns| {
        compact_metric_block(
            &mut columns[0],
            app.t("已扫描体积", "Scanned Size"),
            &format_bytes(app.summary.bytes_observed),
            app.t("完整扫描摘要", "Completed scan summary"),
        );
        compact_metric_block(
            &mut columns[1],
            app.t("显示项目", "Visible Items"),
            &format_count(entries.len() as u64),
            app.t("目录和文件 Top-N", "Directory and file Top-N"),
        );
        compact_metric_block(
            &mut columns[2],
            app.t("资源策略", "Resource Policy"),
            app.t("轻量", "Lightweight"),
            app.t("不常驻完整结果树", "Full tree is not kept resident"),
        );
    });
    ui.add_space(12.0);
    surface_panel(ui, |ui| {
        ui.label(
            egui::RichText::new(app.t("空间占用 Treemap", "Space Usage Treemap"))
                .text_style(egui::TextStyle::Name("title".into())),
        );
        ui.label(
            egui::RichText::new(app.t(
                "点击矩形可联动右侧检查器；如需更细层级，请重新扫描目标子目录。",
                "Click a rectangle to sync the inspector. For a deeper level, scan that subfolder directly.",
            ))
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(8.0);
        draw_ranked_treemap_blocks(app, ui, &entries, total.max(1));
    });
}

fn ranked_treemap_entry(path: Arc<str>, size_bytes: u64, kind: NodeKind) -> RankedTreemapEntry {
    let name = PathBuf::from(path.as_ref())
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.to_string());
    RankedTreemapEntry {
        path,
        name,
        size_bytes,
        kind,
    }
}

fn draw_ranked_treemap_blocks(
    app: &mut DirOtterNativeApp,
    ui: &mut egui::Ui,
    entries: &[RankedTreemapEntry],
    scope_total: u64,
) {
    let visible_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.size_bytes > 0)
        .take(96)
        .collect();
    if visible_entries.is_empty() {
        return;
    }

    let width = ui.available_width().max(320.0);
    let height = (width * 0.46).clamp(360.0, 620.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let canvas = rect.shrink(8.0);
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals().clone();
    let border = border_color(&visuals);
    let text_color = visuals.text_color();
    let weak_text = visuals.weak_text_color();
    let total = scope_total.max(1);
    let palette = [
        river_teal(),
        info_blue(),
        success_green(),
        egui::Color32::from_rgb(0xC4, 0x79, 0x3B),
        egui::Color32::from_rgb(0x76, 0x7C, 0xC8),
        egui::Color32::from_rgb(0x96, 0x7B, 0x5A),
    ];
    painter.rect_filled(rect, egui::Rounding::same(0.0), visuals.panel_fill);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(0.0),
        egui::Stroke::new(1.0, border),
    );

    let mut remaining = canvas;
    let mut remaining_total = visible_entries
        .iter()
        .map(|entry| entry.size_bytes)
        .sum::<u64>()
        .max(1) as f32;

    for (idx, entry) in visible_entries.iter().enumerate() {
        let last = idx + 1 == visible_entries.len();
        let share = (entry.size_bytes as f32 / remaining_total).clamp(0.0, 1.0);
        let cell = if last {
            remaining
        } else if remaining.width() >= remaining.height() {
            let split_width = (remaining.width() * share).clamp(18.0, remaining.width());
            let cell = egui::Rect::from_min_max(
                remaining.min,
                egui::pos2(remaining.left() + split_width, remaining.bottom()),
            );
            remaining.min.x = (remaining.min.x + split_width + 3.0).min(remaining.max.x);
            cell
        } else {
            let split_height = (remaining.height() * share).clamp(18.0, remaining.height());
            let cell = egui::Rect::from_min_max(
                remaining.min,
                egui::pos2(remaining.right(), remaining.top() + split_height),
            );
            remaining.min.y = (remaining.min.y + split_height + 3.0).min(remaining.max.y);
            cell
        };
        remaining_total = (remaining_total - entry.size_bytes as f32).max(1.0);
        if cell.width() < 4.0 || cell.height() < 4.0 {
            continue;
        }

        let selected = app.selection_matches_path(entry.path.as_ref());
        let fill = if selected {
            visuals.selection.bg_fill
        } else if matches!(entry.kind, NodeKind::Dir) {
            palette[idx % palette.len()]
        } else if visuals.dark_mode {
            egui::Color32::from_rgb(0x3D, 0x62, 0x72)
        } else {
            egui::Color32::from_rgb(0x7B, 0xB3, 0xC7)
        };
        painter.rect_filled(cell, egui::Rounding::same(0.0), fill);
        painter.rect_stroke(
            cell,
            egui::Rounding::same(0.0),
            egui::Stroke::new(1.0, border),
        );

        let response = ui
            .interact(
                cell,
                ui.make_persistent_id(("ranked_treemap_block", entry.path.as_ref())),
                egui::Sense::click(),
            )
            .on_hover_text(format!(
                "{}\n{}",
                entry.path,
                format_bytes(entry.size_bytes)
            ));
        if response.clicked() {
            app.select_path(entry.path.as_ref(), SelectionSource::Treemap);
        }

        if cell.width() > 82.0 && cell.height() > 46.0 {
            let title = truncate_middle(&entry.name, (cell.width() / 8.0) as usize);
            painter.text(
                cell.left_top() + egui::vec2(8.0, 7.0),
                egui::Align2::LEFT_TOP,
                title,
                egui::FontId::proportional(13.0),
                text_color,
            );
            painter.text(
                cell.left_top() + egui::vec2(8.0, 26.0),
                egui::Align2::LEFT_TOP,
                format_bytes(entry.size_bytes),
                egui::FontId::proportional(11.0),
                weak_text,
            );
        }
        if cell.width() > 70.0 && cell.height() > 70.0 {
            painter.text(
                cell.right_bottom() - egui::vec2(8.0, 8.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{:.1}%", entry.size_bytes as f32 / total as f32 * 100.0),
                egui::FontId::proportional(11.0),
                text_color,
            );
        }
    }
}

fn draw_treemap_result_blocks(
    app: &mut DirOtterNativeApp,
    ui: &mut egui::Ui,
    entries: &[TreemapEntry],
    scope_total: u64,
) {
    let visible_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.size_bytes > 0)
        .take(96)
        .collect();
    if visible_entries.is_empty() {
        return;
    }

    let width = ui.available_width().max(320.0);
    let height = (width * 0.46).clamp(360.0, 620.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let canvas = rect.shrink(8.0);
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals().clone();
    let border = border_color(&visuals);
    let text_color = visuals.text_color();
    let weak_text = visuals.weak_text_color();
    let total = visible_entries
        .iter()
        .map(|entry| entry.size_bytes)
        .sum::<u64>()
        .max(scope_total.min(u64::MAX - 1))
        .max(1);
    let palette = [
        river_teal(),
        info_blue(),
        success_green(),
        egui::Color32::from_rgb(0xC4, 0x79, 0x3B),
        egui::Color32::from_rgb(0x76, 0x7C, 0xC8),
        egui::Color32::from_rgb(0x96, 0x7B, 0x5A),
    ];
    painter.rect_filled(rect, egui::Rounding::same(8.0), visuals.panel_fill);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(8.0),
        egui::Stroke::new(1.0, border),
    );

    let mut remaining = canvas;
    let mut remaining_total = visible_entries
        .iter()
        .map(|entry| entry.size_bytes)
        .sum::<u64>()
        .max(1) as f32;

    for (idx, entry) in visible_entries.iter().enumerate() {
        let last = idx + 1 == visible_entries.len();
        let share = (entry.size_bytes as f32 / remaining_total).clamp(0.0, 1.0);
        let cell = if last {
            remaining
        } else if remaining.width() >= remaining.height() {
            let split_width = (remaining.width() * share).clamp(18.0, remaining.width());
            let cell = egui::Rect::from_min_max(
                remaining.min,
                egui::pos2(remaining.left() + split_width, remaining.bottom()),
            );
            remaining.min.x = (remaining.min.x + split_width + 3.0).min(remaining.max.x);
            cell
        } else {
            let split_height = (remaining.height() * share).clamp(18.0, remaining.height());
            let cell = egui::Rect::from_min_max(
                remaining.min,
                egui::pos2(remaining.right(), remaining.top() + split_height),
            );
            remaining.min.y = (remaining.min.y + split_height + 3.0).min(remaining.max.y);
            cell
        };
        remaining_total = (remaining_total - entry.size_bytes as f32).max(1.0);
        if cell.width() < 4.0 || cell.height() < 4.0 {
            continue;
        }

        let selected = app.selection_matches_treemap_entry(entry);
        let mut fill = palette[idx % palette.len()];
        if !matches!(entry.kind, NodeKind::Dir) {
            fill = if visuals.dark_mode {
                egui::Color32::from_rgb(0x3D, 0x62, 0x72)
            } else {
                egui::Color32::from_rgb(0x7B, 0xB3, 0xC7)
            };
        }
        if selected {
            fill = visuals.selection.bg_fill;
        }
        painter.rect_filled(cell, egui::Rounding::same(6.0), fill);
        painter.rect_stroke(
            cell,
            egui::Rounding::same(6.0),
            egui::Stroke::new(1.0, border),
        );

        let response = ui
            .interact(
                cell,
                ui.make_persistent_id(("treemap_block", entry.node_id.0)),
                egui::Sense::click(),
            )
            .on_hover_text(format!(
                "{}\n{}\n{} {} / {} {}\n{}",
                entry.path,
                format_bytes(entry.size_bytes),
                format_count(entry.file_count),
                app.t("文件", "files"),
                format_count(entry.dir_count.saturating_sub(1)),
                app.t("子目录", "subdirs"),
                if matches!(entry.kind, NodeKind::Dir) {
                    app.t("点击进入下一层", "Click to open this level")
                } else {
                    app.t("点击选中文件", "Click to select this file")
                }
            ));
        if response.clicked() {
            if matches!(entry.kind, NodeKind::Dir) {
                app.focus_treemap_node(entry.node_id);
            } else {
                app.select_node(entry.node_id, SelectionSource::Treemap);
            }
        }

        let label_room = cell.width() > 82.0 && cell.height() > 46.0;
        if label_room {
            let title = truncate_middle(entry.name.as_ref(), (cell.width() / 8.0) as usize);
            painter.text(
                cell.left_top() + egui::vec2(8.0, 7.0),
                egui::Align2::LEFT_TOP,
                title,
                egui::FontId::proportional(13.0),
                text_color,
            );
            painter.text(
                cell.left_top() + egui::vec2(8.0, 26.0),
                egui::Align2::LEFT_TOP,
                format_bytes(entry.size_bytes),
                egui::FontId::proportional(11.0),
                weak_text,
            );
        }
        let percent = entry.size_bytes as f32 / total as f32;
        if cell.width() > 70.0 && cell.height() > 70.0 {
            painter.text(
                cell.right_bottom() - egui::vec2(8.0, 8.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{:.1}%", percent * 100.0),
                egui::FontId::proportional(11.0),
                text_color,
            );
        }
    }
}
