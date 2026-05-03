use super::*;

pub(super) fn ui_dashboard(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    page_header(
            ui,
            app.t("DirOtter 工作台", "DirOtter Workspace"),
            app.t("磁盘概览", "Drive Overview"),
            app.t(
                "先看结论和动作，再进入扫描设置，以及最大的文件夹和文件。",
                "Start with the conclusion and action, then move into scan setup and the largest folders and files.",
            ),
        );
    let ranked_dirs = app.current_ranked_dirs(10);
    let (items_title, items_subtitle, ranked_items) = app.contextual_ranked_files_panel(10);
    let folders_title = app.t("最大文件夹", "Largest Folders").to_string();
    let folders_subtitle = app
        .t(
            "优先看哪些目录占空间最多。",
            "Start with the folders consuming the most space.",
        )
        .to_string();
    let folders_empty_body = app
        .t(
            "开始扫描后会显示占用空间最多的目录。",
            "Start a scan to see which directories consume the most space.",
        )
        .to_string();
    let items_empty_body = app
        .t(
            "开始扫描后会优先显示最值得检查的大文件。",
            "Start a scan to surface the largest files worth reviewing first.",
        )
        .to_string();
    if app.scan_active() {
        render_live_overview_hero(app, ui);
    } else if app.summary.bytes_observed > 0 || app.cleanup.analysis.is_some() {
        render_overview_hero(app, ui);
    }
    ui.add_space(14.0);
    render_overview_metrics_strip(app, ui);
    ui.add_space(18.0);
    render_scan_target_card(app, ui);
    ui.add_space(18.0);
    let wide_layout = ui.available_width() >= 740.0;
    if wide_layout {
        let gap = 20.0;
        let total = ui.available_width();
        let left_width = (total - gap) / 2.0;
        let right_width = total - gap - left_width;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_ranked_size_list(
                        ui,
                        &folders_title,
                        &folders_subtitle,
                        &folders_empty_body,
                        &ranked_dirs,
                        app.summary.bytes_observed,
                        &mut app.selection,
                        &mut app.execution_report,
                    );
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_ranked_size_list(
                        ui,
                        &items_title,
                        &items_subtitle,
                        &items_empty_body,
                        &ranked_items,
                        app.summary.bytes_observed,
                        &mut app.selection,
                        &mut app.execution_report,
                    );
                },
            );
        });
    } else {
        render_ranked_size_list(
            ui,
            &folders_title,
            &folders_subtitle,
            &folders_empty_body,
            &ranked_dirs,
            app.summary.bytes_observed,
            &mut app.selection,
            &mut app.execution_report,
        );
        ui.add_space(18.0);
        render_ranked_size_list(
            ui,
            &items_title,
            &items_subtitle,
            &items_empty_body,
            &ranked_items,
            app.summary.bytes_observed,
            &mut app.selection,
            &mut app.execution_report,
        );
    }
}

pub(super) fn render_overview_hero(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    let analysis = app.cleanup.analysis.as_ref();
    let reclaimable = analysis.map(|a| a.reclaimable_bytes).unwrap_or(0);
    let quick_clean = analysis.map(|a| a.quick_clean_bytes).unwrap_or(0);
    let has_items = analysis.is_some_and(|analysis| !analysis.items.is_empty());
    let default_category =
        analysis.and_then(|analysis| analysis.categories.first().map(|entry| entry.category));
    let top_categories: Vec<_> = analysis
        .map(|analysis| {
            analysis
                .categories
                .iter()
                .filter(|category| category.reclaimable_bytes > 0)
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let boost_action = app.recommended_boost_action();
    let boost_title = app.t("一键提速", "One-Tap Boost").to_string();
    let boost_body = match boost_action {
            BoostAction::QuickCacheCleanup => format!(
                "{} {}。",
                app.t(
                    "当前最安全、最直接的一键提速动作是清理缓存，预计可先释放",
                    "The safest and most direct one-tap boost right now is cache cleanup, with about",
                ),
                format_bytes(quick_clean)
            ),
            BoostAction::StartScan => app
                .t(
                    "先完成一次扫描，DirOtter 才能识别安全缓存和真正值得处理的大文件。",
                    "Run a scan first so DirOtter can identify safe cache and the largest cleanup targets.",
                )
                .to_string(),
            BoostAction::ReviewSuggestions => app
                .t(
                    "已经找到可疑似拖慢系统的占用点，但它们还需要你确认后再执行。",
                    "Potential system-slowing storage targets were found, but they still need your confirmation before execution.",
                )
                .to_string(),
            BoostAction::NoImmediateAction => app
                .t(
                    "当前没有明确的安全一键提速项，通常从最大的文件夹和文件开始最有效。",
                    "No safe one-tap boost stands out right now. Starting from the largest folders and files is usually the most effective next step.",
                )
                .to_string(),
        };
    let boost_button = match boost_action {
        BoostAction::QuickCacheCleanup => app.t("一键提速（推荐）", "Boost Now (Recommended)"),
        BoostAction::StartScan => app.t("开始提速扫描", "Start Boost Scan"),
        BoostAction::ReviewSuggestions => app.t("查看提速建议", "Review Boost Suggestions"),
        BoostAction::NoImmediateAction => app.t("查看最大占用", "Review Largest Items"),
    }
    .to_string();
    let review_suggestions_button = app.t("查看建议详情", "Review Suggestions").to_string();
    let action_enabled = !app.scan_active() && !app.delete_active();
    let action_returns_to_dashboard = matches!(boost_action, BoostAction::NoImmediateAction);
    let hero_value_size = if reclaimable > 0 || app.summary.bytes_observed > 0 {
        36.0
    } else {
        26.0
    };
    let hero_label = if reclaimable > 0 {
        app.t("清理建议", "Cleanup Suggestions")
    } else if app.summary.bytes_observed > 0 {
        app.t("磁盘概览", "Drive Overview")
    } else {
        app.t("准备开始一次目录巡检", "Ready for a New Pass")
    };
    let hero_value = if reclaimable > 0 {
        format_bytes(reclaimable)
    } else if app.summary.bytes_observed > 0 {
        format_bytes(app.summary.bytes_observed)
    } else {
        app.t("先选一个盘符开始扫描。", "Pick a drive to begin scanning.")
            .to_string()
    };
    let hero_body = if reclaimable > 0 {
        app.t(
            "只统计通过规则筛选后的建议项，先告诉你哪里最值得处理。",
            "Only counts rule-based suggestions so the next action is obvious.",
        )
    } else if app.summary.bytes_observed > 0 {
        app.t(
                "如果当前还没有明确建议，就先从最大文件夹和最大文件开始处理。",
                "If there is no clear cleanup suggestion yet, start from the largest folders and files.",
            )
    } else {
        app.t(
            "从盘符按钮直接开始，或先调整根目录和扫描模式。",
            "Start from a drive button, or adjust the root path and scan mode first.",
        )
    };
    let current_scope = if app.root_input.trim().is_empty() {
        app.t("未设置", "Not set").to_string()
    } else {
        truncate_middle(&app.root_input, 44)
    };
    let scope_mode_title = app.t("当前范围与模式", "Current Scope & Mode").to_string();
    let root_label = app.t("根目录", "Root path").to_string();
    let root_subtitle = app.t("当前扫描目标", "Current scope").to_string();
    let mode_label = app.t("当前模式", "Current Mode").to_string();
    let mode_title = app.scan_mode_title(app.scan_mode).to_string();
    let mode_description = app.scan_mode_description(app.scan_mode).to_string();
    let no_suggestions_title = app.t("还没有建议项", "No Suggestions Yet").to_string();
    let no_suggestions_body = app
        .t(
            "继续完成一次扫描，或直接看下方的最大文件夹和最大文件。",
            "Finish a scan or move straight to the largest folders and files below.",
        )
        .to_string();
    let top_sources_label = app.t("主要来源", "Top Sources").to_string();
    let top_source_rows: Vec<_> = top_categories
        .iter()
        .map(|category| {
            (
                app.cleanup_category_color(category.category),
                app.cleanup_category_label(category.category).to_string(),
                format_bytes(category.reclaimable_bytes),
            )
        })
        .collect();
    dashboard_panel(ui, |ui| {
        dashboard_split(
            ui,
            320.0,
            20.0,
            |ui| {
                ui.label(
                    egui::RichText::new(&boost_title)
                        .text_style(egui::TextStyle::Small)
                        .color(river_teal()),
                );
                ui.add_space(8.0);
                ui.label(egui::RichText::new(&boost_button).size(28.0).strong());
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(&boost_body)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled_ui(action_enabled, |ui| {
                            sized_primary_button(ui, 220.0, &boost_button)
                        })
                        .inner
                        .clicked()
                    {
                        if action_returns_to_dashboard {
                            app.page = Page::Dashboard;
                        }
                        app.execute_recommended_boost();
                    }

                    if ui
                        .add_enabled_ui(has_items, |ui| {
                            sized_button(ui, 180.0, &review_suggestions_button)
                        })
                        .inner
                        .clicked()
                    {
                        app.cleanup.detail_category = default_category;
                    }
                });
            },
            |ui| {
                ui.label(
                    egui::RichText::new(hero_label)
                        .text_style(egui::TextStyle::Small)
                        .color(river_teal()),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(hero_value)
                        .size(hero_value_size)
                        .strong(),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(hero_body)
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                );
            },
        );
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(14.0);
        dashboard_split(
            ui,
            320.0,
            20.0,
            |ui| {
                ui.label(egui::RichText::new(&scope_mode_title).strong());
                ui.add_space(6.0);
                stat_row(ui, &root_label, &current_scope, &root_subtitle);
                ui.add_space(8.0);
                stat_row(ui, &mode_label, &mode_title, &mode_description);
            },
            |ui| {
                if top_source_rows.is_empty() {
                    empty_state_panel(ui, &no_suggestions_title, &no_suggestions_body);
                } else {
                    ui.label(
                        egui::RichText::new(&top_sources_label)
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(6.0);
                    for (color, label, value) in &top_source_rows {
                        ui.horizontal(|ui| {
                            ui.colored_label(*color, "●");
                            ui.label(egui::RichText::new(label).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new(value).strong());
                                },
                            );
                        });
                        ui.add_space(6.0);
                    }
                }
            },
        );
    });
}

pub(super) fn render_live_overview_hero(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    let current_path = app
        .scan_current_path
        .as_deref()
        .map(|path| truncate_middle(path, 84))
        .unwrap_or_else(|| {
            app.t("正在准备扫描路径…", "Preparing scan path...")
                .to_string()
        });
    let coverage_label = app
        .scanned_coverage_ratio()
        .map(|ratio| format!("{:.0}%", ratio * 100.0))
        .unwrap_or_else(|| app.t("估算中", "Estimating").to_string());
    dashboard_panel(ui, |ui| {
        ui.label(
            egui::RichText::new(app.t("实时总览", "Live Overview"))
                .text_style(egui::TextStyle::Small)
                .color(river_teal()),
        );
        ui.add_space(8.0);
        dashboard_split(
            ui,
            320.0,
            20.0,
            |ui| {
                ui.label(
                    egui::RichText::new(format_bytes(app.summary.bytes_observed))
                        .size(36.0)
                        .strong(),
                );
                ui.add_space(6.0);
                ui.label(
                        egui::RichText::new(app.t(
                            "扫描中首页只保留当前态势，不提前给最终结论。",
                            "While scanning, the overview stays focused on current state instead of premature conclusions.",
                        ))
                        .text_style(egui::TextStyle::Small)
                        .color(ui.visuals().weak_text_color()),
                    );
                ui.add_space(12.0);
                stat_row(
                    ui,
                    app.t("当前处理路径", "Current Path"),
                    &current_path,
                    app.scan_health_summary().as_str(),
                );
            },
            |ui| {
                ui.label(egui::RichText::new(app.t("扫描态势", "Scan Status")).strong());
                ui.add_space(6.0);
                stat_row(
                    ui,
                    app.t("扫描覆盖率", "Coverage"),
                    &coverage_label,
                    app.t("按卷容量估算", "Estimated against volume size"),
                );
                ui.add_space(8.0);
                stat_row(
                    ui,
                    app.t("错误", "Errors"),
                    &format_count(app.summary.error_count),
                    app.t("当前已累计的问题项", "Issues accumulated so far"),
                );
                ui.add_space(8.0);
                stat_row(
                    ui,
                    app.t("已观察体积", "Observed Bytes"),
                    &format_bytes(app.summary.bytes_observed),
                    app.t(
                        "这是实时增量状态，不是最终结论。",
                        "This is live incremental state, not the final conclusion.",
                    ),
                );
            },
        );
    });
}

pub(super) fn render_overview_metrics_strip(app: &DirOtterNativeApp, ui: &mut egui::Ui) {
    let cards = if let Some((used, free, total)) = app.volume_numbers() {
        [
            (
                app.t("磁盘已用", "Used"),
                format_bytes(used),
                format!(
                    "{} {}",
                    format_bytes(total),
                    app.t("总容量", "total capacity")
                ),
                river_teal(),
            ),
            (
                app.t("磁盘可用", "Free"),
                format_bytes(free),
                app.t("当前卷剩余可用空间", "Remaining free space on this volume")
                    .to_string(),
                info_blue(),
            ),
            (
                app.t("已扫描体积", "Observed"),
                format_bytes(app.summary.bytes_observed),
                app.t(
                    "本次扫描已经确认的文件体积",
                    "File bytes already confirmed in this scan",
                )
                .to_string(),
                success_green(),
            ),
            (
                app.t("错误", "Errors"),
                format_count(app.summary.error_count),
                app.t("无法读取或被跳过的路径", "Unreadable or skipped paths")
                    .to_string(),
                if app.summary.error_count > 0 {
                    danger_red()
                } else {
                    egui::Color32::from_rgb(0x5F, 0x8D, 0x96)
                },
            ),
        ]
    } else {
        [
            (
                app.t("文件", "Files"),
                format_count(app.summary.scanned_files),
                app.t("已发现文件数", "Files discovered").to_string(),
                river_teal(),
            ),
            (
                app.t("目录", "Folders"),
                format_count(app.summary.scanned_dirs),
                app.t("已遍历目录数", "Folders traversed").to_string(),
                info_blue(),
            ),
            (
                app.t("已扫描体积", "Observed"),
                format_bytes(app.summary.bytes_observed),
                app.t(
                    "本次扫描已经确认的文件体积",
                    "File bytes already confirmed in this scan",
                )
                .to_string(),
                success_green(),
            ),
            (
                app.t("错误", "Errors"),
                format_count(app.summary.error_count),
                app.t("无法读取或被跳过的路径", "Unreadable or skipped paths")
                    .to_string(),
                if app.summary.error_count > 0 {
                    danger_red()
                } else {
                    egui::Color32::from_rgb(0x5F, 0x8D, 0x96)
                },
            ),
        ]
    };
    if ui.available_width() >= 980.0 {
        dashboard_metric_row(ui, &cards);
    } else if ui.available_width() >= 620.0 {
        dashboard_metric_row(ui, &cards[..2]);
        ui.add_space(12.0);
        dashboard_metric_row(ui, &cards[2..]);
    } else {
        for card in cards {
            dashboard_metric_tile(ui, card.0, &card.1, &card.2, card.3);
            ui.add_space(12.0);
        }
    }
}

pub(super) fn render_scan_target_card(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    dashboard_panel(ui, |ui| {
        ui.label(
            egui::RichText::new(app.t("开始扫描", "Start Scan"))
                .text_style(egui::TextStyle::Name("title".into())),
        );
        ui.add_space(4.0);
        ui.label(
                egui::RichText::new(app.t(
                    "扫描负责查找磁盘占用；内存释放请使用右侧快速操作中的独立入口。",
                    "Scanning finds storage hotspots. Use the separate memory action in Quick Actions for memory release.",
                ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
            );
        ui.add_space(12.0);
        if ui
            .add_enabled_ui(!app.scan_active(), |ui| {
                sized_primary_button(
                    ui,
                    ui.available_width(),
                    if app.scan_active() {
                        app.t("扫描进行中", "Scanning")
                    } else {
                        app.t("开始扫描", "Start Scan")
                    },
                )
            })
            .inner
            .on_hover_text(app.t(
                "扫描进行中时请使用右上角的停止按钮。",
                "Use the top-right stop button while a scan is running.",
            ))
            .clicked()
        {
            app.start_scan();
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);
        ui.label(egui::RichText::new(app.t("扫描设置", "Scan Setup")).strong());
        ui.label(
                egui::RichText::new(app.t(
                    "如果需要更细粒度地排查空间占用，再手动调整盘符、目录和扫描模式。",
                    "Adjust the drive, folder, and scan mode only when you need a more targeted storage investigation.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
        ui.add_space(12.0);
        ui.label(egui::RichText::new(app.t("快速盘符", "Quick Drives")).strong());
        ui.label(
                egui::RichText::new(app.t(
                    "优先点击盘符直接开始；只有要扫子目录时再手动输入。",
                    "Start with a drive button first. Only type a manual path when you need a subfolder.",
                ))
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
            );
        ui.add_space(8.0);
        if app.available_volumes.is_empty() {
            empty_state_panel(
                ui,
                app.t("没有检测到卷", "No Volumes Detected"),
                app.t(
                    "仍可手动输入任意目录作为扫描目标。",
                    "You can still enter any folder manually as the scan target.",
                ),
            );
        } else {
            let volumes = app.available_volumes.clone();
            ui.horizontal_wrapped(|ui| {
                for volume in volumes {
                    let used = volume.total_bytes.saturating_sub(volume.available_bytes);
                    let selected = app.root_input == volume.mount_point;
                    let label = format!(
                        "{}  {} / {}",
                        short_volume_label(&volume),
                        format_bytes(used),
                        format_bytes(volume.total_bytes)
                    );
                    let response = ui
                        .add_enabled_ui(!app.scan_active(), |ui| {
                            sized_selectable(ui, 156.0, selected, &label)
                        })
                        .inner
                        .on_hover_text(format!(
                            "{}\n{} {}\n{} {}",
                            volume.name,
                            app.t("已用", "Used"),
                            format_bytes(used),
                            app.t("总量", "Total"),
                            format_bytes(volume.total_bytes)
                        ));
                    if response.clicked() {
                        app.start_scan_for_root(volume.mount_point.clone());
                    }
                }
            });
        }

        ui.add_space(14.0);
        ui.label(egui::RichText::new(app.t("手动目录（可选）", "Manual path (optional)")).strong());
        ui.add_space(6.0);
        let root_hint = app
            .t("例如 D:\\Projects", "For example D:\\Projects")
            .to_string();
        ui.add_sized(
            [ui.available_width().min(420.0), CONTROL_HEIGHT],
            egui::TextEdit::singleline(&mut app.root_input)
                .desired_width(420.0)
                .hint_text(root_hint),
        );

        ui.add_space(14.0);
        ui.label(egui::RichText::new(app.t("扫描策略", "Scan Strategy")).strong());
        ui.label(
            egui::RichText::new(app.t(
                "默认策略足够日常清理；只有超大目录、外置盘或压力测试时再展开高级节奏。",
                "Default strategy is enough for normal cleanup. Open advanced pacing only for huge folders, external drives, or stress testing.",
            ))
            .text_style(egui::TextStyle::Small)
            .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(8.0);
        ui.add_enabled_ui(!app.scan_active(), |ui| {
            let recommended = ScanMode::Quick;
            let response = sized_selectable(
                ui,
                220.0,
                app.scan_mode == recommended,
                app.scan_mode_title(recommended),
            )
            .on_hover_text(app.scan_mode_description(recommended));
            if response.clicked() {
                app.set_scan_mode(recommended);
            }

            ui.add_space(6.0);
            egui::CollapsingHeader::new(app.t("高级扫描节奏", "Advanced scan pacing"))
                .default_open(app.scan_mode != ScanMode::Quick)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(app.scan_mode_note())
                            .text_style(egui::TextStyle::Small)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for mode in [ScanMode::Deep, ScanMode::LargeDisk] {
                            let response = sized_selectable(
                                ui,
                                190.0,
                                app.scan_mode == mode,
                                app.scan_mode_title(mode),
                            )
                            .on_hover_text(app.scan_mode_description(mode));
                            if response.clicked() {
                                app.set_scan_mode(mode);
                            }
                        }
                    });
                });
        });
        ui.add_space(10.0);
        tone_banner(
            ui,
            app.scan_mode_title(app.scan_mode),
            app.scan_mode_description(app.scan_mode),
        );

        if let Some((used, free, _)) = app.volume_numbers() {
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                compact_stat_chip(ui, app.t("磁盘已用", "Used"), &format_bytes(used));
                compact_stat_chip(ui, app.t("磁盘可用", "Free"), &format_bytes(free));
                if let Some(ratio) = app.scanned_coverage_ratio() {
                    compact_stat_chip(
                        ui,
                        app.t("扫描覆盖率", "Coverage"),
                        &format!("{:.0}%", ratio * 100.0),
                    );
                }
                compact_stat_chip(
                    ui,
                    app.t("文件", "Files"),
                    &format_count(app.summary.scanned_files),
                );
            });
        }

        ui.add_space(14.0);
        let scan_only_response = ui
            .add_enabled_ui(!app.scan_active(), |ui| {
                sized_button(ui, ui.available_width(), app.t("仅执行扫描", "Scan Only"))
            })
            .inner
            .on_hover_text(app.t(
                "按当前路径和模式直接开始扫描。",
                "Start a scan directly with the current path and mode.",
            ));
        if scan_only_response.clicked() {
            app.start_scan();
        }
    });
}
