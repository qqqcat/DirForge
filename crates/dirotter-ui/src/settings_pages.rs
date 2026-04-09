use super::*;

pub(super) fn ui_diagnostics(app: &mut DirOtterNativeApp, ui: &mut egui::Ui) {
    page_header(
        ui,
        app.t("DirOtter 工作台", "DirOtter Workspace"),
        app.t("诊断信息", "Diagnostics"),
        app.t(
            "只保留当前会话的结构化诊断信息，不再要求额外导出或持久化。",
            "Keep diagnostics as a structured view of the current session without requiring export or persistence.",
        ),
    );
    ui.add_space(8.0);
    let mut refresh_diag = false;
    let mut optimize_app_memory = false;
    let mut clean_interrupted_cleanup_area = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(app.t("刷新诊断", "Refresh diagnostics"))
            .clicked()
        {
            refresh_diag = true;
        }
    });
    ui.add_space(10.0);
    settings_section(
            ui,
            app.t("高级维护", "Advanced Maintenance"),
            app.t(
                "这些动作只影响当前会话的内存与恢复状态，不再写入扫描历史或导出诊断包。",
                "These actions only affect the current session's memory and recovery state. They no longer write scan history or export diagnostic bundles.",
            ),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled_ui(!app.scan_active() && !app.delete_active(), |ui| {
                            sized_button(
                                ui,
                                220.0,
                                app.t("优化 DirOtter 内存占用", "Optimize DirOtter Memory"),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        optimize_app_memory = true;
                    }
                    if ui
                        .add_enabled_ui(!app.scan_active() && !app.delete_active(), |ui| {
                            sized_button(
                                ui,
                                260.0,
                                app.t(
                                    "清理异常中断的临时删除区",
                                    "Clean Interrupted Cleanup Area",
                                ),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        clean_interrupted_cleanup_area = true;
                    }
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(app.t(
                        "当快速清理缓存在后台删除完成前被异常中断时，内部 staging 临时区可能会留下待删内容。这个动作只负责把这些残留项清掉。",
                        "If a fast cache cleanup is interrupted before background deletion finishes, the internal staging area may keep leftover temporary items. This action only removes those leftovers.",
                    ))
                    .text_style(egui::TextStyle::Small)
                    .color(ui.visuals().weak_text_color()),
                );
            },
        );
    if let Some((message, success)) = app.maintenance_feedback.as_ref() {
        ui.add_space(8.0);
        tone_banner(
            ui,
            if *success {
                app.t("已完成", "Done")
            } else {
                app.t("操作失败", "Action Failed")
            },
            message,
        );
    }
    if refresh_diag {
        app.refresh_diagnostics();
    }
    if optimize_app_memory {
        app.release_dir_otter_memory();
    }
    if clean_interrupted_cleanup_area {
        app.purge_staging_manually();
    }
    ui.separator();
    let panel_width = ui.available_width();
    let viewport_height = ui.ctx().input(|i| i.screen_rect().height());
    let editor_height = (viewport_height - TOOLBAR_HEIGHT - STATUSBAR_HEIGHT - 220.0).max(420.0);
    surface_panel(ui, |ui| {
        ui.set_min_width(panel_width);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), editor_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add_sized(
                            [ui.available_width().max(320.0), editor_height],
                            egui::TextEdit::multiline(&mut app.diagnostics_json)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .code_editor()
                                .interactive(false),
                        );
                    });
            },
        );
    });
}

pub(super) fn ui_settings(app: &mut DirOtterNativeApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    page_header(
        ui,
        app.t("DirOtter 工作台", "DirOtter Workspace"),
        app.t("偏好设置", "Settings"),
        app.t(
            "让 DirOtter 保持冷静、低对比、长时间可用的工作状态。",
            "Keep DirOtter calm, low-contrast, and comfortable for long sessions.",
        ),
    );
    ui.add_space(10.0);
    if app.cache.uses_ephemeral_settings() {
        tone_banner(
            ui,
            app.t("当前为临时会话存储", "Temporary Session Storage Active"),
            app.t(
                "DirOtter 当前无法写入持久设置目录，已退回到临时会话存储。本次运行中的语言、主题和高级工具设置会在退出后丢失。",
                "DirOtter could not write to the persistent settings directory and has fallen back to temporary session storage. Language, theme, and advanced tool settings from this run will be lost after exit.",
            ),
        );
        ui.add_space(14.0);
    }
    tone_banner(
            ui,
            app.t("舒适优先的工作台", "A Comfort-First Workspace"),
            app.t(
                "语言、主题和字体回退都会立即生效。这里的目标不是“更花哨”，而是让长时间浏览目录树时更稳定、更耐看。",
                "Language, theme, and font fallback all apply immediately. The goal here is not flashy UI, but a steadier workspace for long file-tree sessions.",
            ),
        );
    ui.add_space(14.0);
    settings_section(
            ui,
            app.t("常用设置", "Common Settings"),
            app.t(
                "主流设置页会把高频项放在最上面，并保持分组稳定、可预期。",
                "Mainstream settings pages place high-frequency controls first and keep groups stable and predictable.",
            ),
            |ui| {
                settings_row(
                    ui,
                    app.t("界面语言", "Interface Language"),
                    app.t(
                        "手动选择会覆盖系统语言检测。",
                        "Manual selection overrides automatic locale detection.",
                    ),
                    |ui| {
                        let mut selected_language = app.language;
                        surface_frame(ui).show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.vertical(|ui| {
                                egui::ComboBox::from_id_source("settings.language")
                                    .width(ui.available_width().min(320.0))
                                    .selected_text(format!(
                                        "{} ({})",
                                        lang_native_label(selected_language),
                                        lang_setting_value(selected_language).to_uppercase(),
                                    ))
                                    .truncate()
                                    .show_ui(ui, |ui| {
                                        for &lang in supported_languages() {
                                            ui.selectable_value(
                                                &mut selected_language,
                                                lang,
                                                lang_picker_label(lang),
                                            );
                                        }
                                    });
                            });
                        });
                        if selected_language != app.language {
                            app.set_language(selected_language);
                        }
                    },
                );
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);
                settings_row(
                    ui,
                    app.t("界面主题", "Interface Theme"),
                    app.t(
                        "深色更适合长时间分析；浅色则保持低对比和柔和明度。",
                        "Dark is better for long analysis sessions; light stays restrained and low contrast.",
                    ),
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_sized(
                                    [132.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(
                                        !app.theme_dark,
                                        app.t("浅色", "Light"),
                                    ),
                                )
                                .clicked()
                            {
                                app.theme_dark = false;
                                app.apply_theme(ctx);
                                let _ = app.cache.set_setting("theme", "light");
                            }
                            if ui
                                .add_sized(
                                    [132.0, CONTROL_HEIGHT],
                                    egui::SelectableLabel::new(
                                        app.theme_dark,
                                        app.t("深色", "Dark"),
                                    ),
                                )
                                .clicked()
                            {
                                app.theme_dark = true;
                                app.apply_theme(ctx);
                                let _ = app.cache.set_setting("theme", "dark");
                            }
                        });
                    },
                );
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);
                settings_row(
                    ui,
                    app.t("高级工具", "Advanced Tools"),
                    app.t(
                        "把错误与诊断页面收进二级入口。普通清理流程默认不需要它们。",
                        "Keeps errors and diagnostics behind a secondary entry. Most cleanup flows do not need them by default.",
                    ),
                    |ui| {
                        let button_width = 168.0;
                        if ui
                            .add_sized(
                                [button_width, CONTROL_HEIGHT],
                                egui::SelectableLabel::new(
                                    app.advanced_tools_enabled,
                                    if app.advanced_tools_enabled {
                                        app.t("已开启", "Enabled")
                                    } else {
                                        app.t("已隐藏", "Hidden")
                                    },
                                ),
                            )
                            .clicked()
                        {
                            app.set_advanced_tools_enabled(!app.advanced_tools_enabled);
                        }
                    },
                );
            },
        );

    ui.add_space(14.0);
    settings_section(
            ui,
            app.t("视觉方向", "Visual Direction"),
            app.t(
                "这一组只保留品牌语义和当前状态，不把说明文字拆成零散卡片。",
                "This section keeps brand semantics and current state together instead of splitting them into disconnected cards.",
            ),
            |ui| {
                color_note_row(
                    ui,
                    river_teal(),
                    app.t("River Teal", "River Teal"),
                    app.t(
                        "主品牌色，用于主按钮、选中与重点数据。",
                        "Primary brand accent for key actions, selection, and emphasis.",
                    ),
                );
                ui.add_space(10.0);
                color_note_row(
                    ui,
                    if app.theme_dark {
                        egui::Color32::from_rgb(0x18, 0x22, 0x27)
                    } else {
                        egui::Color32::from_rgb(0xEE, 0xF1, 0xF0)
                    },
                    app.t("基础面板", "Base Surfaces"),
                    app.t(
                        "保持低对比、长时间查看不刺眼。",
                        "Kept low-contrast so long sessions stay easy on the eyes.",
                    ),
                );
                ui.add_space(10.0);
                color_note_row(
                    ui,
                    sand_accent(),
                    app.t("暖色辅助", "Warm Accent"),
                    app.t(
                        "只做轻微平衡，不大面积出现。",
                        "Used sparingly to soften the palette, not dominate it.",
                    ),
                );
                ui.add_space(14.0);
                tone_banner(
                    ui,
                    app.t("当前模式", "Current Mode"),
                    if app.theme_dark {
                        app.t(
                            "深色主题已启用：更适合长时间扫描和对比文件体积。",
                            "Dark theme is enabled: better for extended scanning and file-size comparison.",
                        )
                    } else {
                        app.t(
                            "浅色主题已启用：保持低对比和柔和明度，避免纯白带来的刺眼感。",
                            "Light theme is enabled: low contrast and softer luminance to avoid harsh white surfaces.",
                        )
                    },
                );
            },
        );

    ui.add_space(14.0);
    settings_section(
            ui,
            app.t("本地化说明", "Localization Notes"),
            app.t(
                "把与语言相关的规则放在一起，减少用户在不同卡片间来回找解释。",
                "Keep language-related rules together so people do not have to hunt across separate cards.",
            ),
            |ui| {
                ui.label(app.t(
                    "应用会优先加载系统中的多脚本字体回退（Windows 优先 Microsoft YaHei / DengXian / Yu Gothic / Malgun / Nirmala / Leelawadee），尽量避免中文、日文、韩文、印地语、泰语等标签显示为方框。",
                    "The app now prefers multi-script system fallback fonts (Windows prioritizes Microsoft YaHei, DengXian, Yu Gothic, Malgun, Nirmala, and Leelawadee) to reduce tofu boxes across CJK, Indic, and Thai labels.",
                ));
                ui.add_space(8.0);
                ui.label(app.t(
                    "首次启动会根据系统语言环境识别已接入的 19 种语言；这里的手动选择仍然会覆盖自动检测结果。",
                    "The first launch can now infer all 19 supported languages from the system locale, and the manual choice here still overrides auto-detection.",
                ));
            },
        );

    ui.add_space(14.0);
    settings_section(
            ui,
            app.t("品牌含义", "Why DirOtter"),
            app.t(
                "把品牌语义单独留成一个说明章节，而不是塞进控制区旁边。",
                "Keep brand meaning in its own explanatory section instead of squeezing it beside controls.",
            ),
            |ui| {
                ui.label(app.t(
                    "Dir 指 directory，Otter 借用水獭聪明、灵活、善于整理的联想。它更像一个冷静探索存储结构的分析工具，而不是只会“清理垃圾”的工具。",
                    "Dir points to directories, while Otter brings a clever, tidy, exploratory character. The product should feel like a calm storage analyzer, not a noisy junk cleaner.",
                ));
            },
        );
}
