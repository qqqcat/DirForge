use eframe::egui;

/// 主题配置
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeConfig {
    pub name: String,
    pub dark_mode: bool,
    pub colors: ColorPalette,
    pub spacing: SpacingConfig,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            dark_mode: true,
            colors: ColorPalette::default(),
            spacing: SpacingConfig::default(),
        }
    }
}

/// 颜色调色板
#[derive(Debug, Clone, PartialEq)]
pub struct ColorPalette {
    // 背景色
    pub window_fill: egui::Color32,
    pub panel_fill: egui::Color32,
    pub extreme_bg: egui::Color32,
    pub faint_bg: egui::Color32,
    pub code_bg: egui::Color32,

    // 文本色
    pub text: egui::Color32,
    pub weak_text: egui::Color32,
    pub override_text: Option<egui::Color32>,

    // 交互色
    pub selection_fill: egui::Color32,
    pub selection_stroke: egui::Stroke,
    pub widget_noninteractive_fill: egui::Color32,
    pub widget_noninteractive_stroke: egui::Stroke,
    pub widget_inactive_fill: egui::Color32,
    pub widget_inactive_stroke: egui::Stroke,
    pub widget_hovered_fill: egui::Color32,
    pub widget_hovered_stroke: egui::Stroke,
    pub widget_active_fill: egui::Color32,
    pub widget_active_stroke: egui::Stroke,
    pub widget_open_fill: egui::Color32,
}

impl Default for ColorPalette {
    fn default() -> Self {
        // 默认深色主题
        Self::dark()
    }
}

impl ColorPalette {
    pub fn dark() -> Self {
        Self {
            // 背景：更深的蓝灰色，提供清晰的层次
            window_fill: egui::Color32::from_rgb(0x0D, 0x14, 0x1A),
            panel_fill: egui::Color32::from_rgb(0x15, 0x1F, 0x28),
            extreme_bg: egui::Color32::from_rgb(0x09, 0x0F, 0x14),
            faint_bg: egui::Color32::from_rgb(0x1A, 0x25, 0x30),
            code_bg: egui::Color32::from_rgb(0x11, 0x18, 0x20),

            // 文本：纯白色确保最高对比度
            text: egui::Color32::WHITE,
            weak_text: egui::Color32::from_rgb(0x8B, 0x94, 0x9E),
            override_text: None,

            // 交互：更鲜明的青色
            selection_fill: egui::Color32::from_rgb(0x3D, 0x9E, 0xAA),
            selection_stroke: egui::Stroke::new(1.0, egui::Color32::WHITE),

            // 控件：更明显的层次
            widget_noninteractive_fill: egui::Color32::from_rgb(0x12, 0x1C, 0x24),
            widget_noninteractive_stroke: egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(0x24, 0x30, 0x3C),
            ),
            widget_inactive_fill: egui::Color32::from_rgb(0x18, 0x22, 0x2C),
            widget_inactive_stroke: egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(0x26, 0x32, 0x3E),
            ),
            widget_hovered_fill: egui::Color32::from_rgb(0x3D, 0x9E, 0xAA),
            widget_hovered_stroke: egui::Stroke::new(
                1.5,
                egui::Color32::from_rgb(0x3D, 0x9E, 0xAA),
            ),
            widget_active_fill: egui::Color32::from_rgb(0x2D, 0x8E, 0x98),
            widget_active_stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(0x3D, 0x9E, 0xAA)),
            widget_open_fill: egui::Color32::from_rgb(0x1A, 0x25, 0x30),
        }
    }

    pub fn light() -> Self {
        Self {
            // 背景：干净的灰白色
            window_fill: egui::Color32::from_rgb(0xFA, 0xFC, 0xFE),
            panel_fill: egui::Color32::from_rgb(0xF0, 0xF4, 0xF8),
            extreme_bg: egui::Color32::from_rgb(0xE4, 0xEC, 0xF2),
            faint_bg: egui::Color32::from_rgb(0xF5, 0xF8, 0xFA),
            code_bg: egui::Color32::from_rgb(0xEB, 0xF0, 0xF5),

            // 文本：深灰色，确保高对比度
            text: egui::Color32::from_rgb(0x1F, 0x23, 0x28),
            weak_text: egui::Color32::from_rgb(0x65, 0x6D, 0x76),
            override_text: None,

            // 交互：鲜明的青色
            selection_fill: egui::Color32::from_rgb(0x2D, 0x8E, 0x98),
            selection_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(0xFA, 0xFC, 0xFE)),

            // 控件：清晰的层次
            widget_noninteractive_fill: egui::Color32::from_rgb(0xED, 0xF2, 0xF6),
            widget_noninteractive_stroke: egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(0xCC, 0xD8, 0xE2),
            ),
            widget_inactive_fill: egui::Color32::from_rgb(0xE5, 0xEB, 0xF0),
            widget_inactive_stroke: egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(0xBB, 0xCC, 0xD8),
            ),
            widget_hovered_fill: egui::Color32::from_rgb(0xD5, 0xE5, 0xEB),
            widget_hovered_stroke: egui::Stroke::new(
                1.5,
                egui::Color32::from_rgb(0x2D, 0x8E, 0x98),
            ),
            widget_active_fill: egui::Color32::from_rgb(0xB8, 0xD5, 0xDD),
            widget_active_stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(0x2D, 0x8E, 0x98)),
            widget_open_fill: egui::Color32::from_rgb(0xE0, 0xE8, 0xEF),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

/// 间距配置
#[derive(Debug, Clone, PartialEq)]
pub struct SpacingConfig {
    pub card_padding: f32,
    pub card_stroke_width: f32,
    pub control_height: f32,
    pub nav_item_height: f32,
    pub status_badge_height: f32,
    pub control_min_width: f32,
    pub page_max_width: f32,
    pub page_side_gutter: f32,
}

impl Default for SpacingConfig {
    fn default() -> Self {
        Self {
            card_padding: 14.0,
            card_stroke_width: 1.0,
            control_height: 34.0,
            nav_item_height: 36.0,
            status_badge_height: 32.0,
            control_min_width: 56.0,
            page_max_width: 1160.0,
            page_side_gutter: 64.0,
        }
    }
}

/// 应用主题到 egui Visuals
pub fn apply_theme(visuals: &mut egui::Visuals, mode: ThemeMode, palette: &ColorPalette) {
    *visuals = match mode {
        ThemeMode::Dark => egui::Visuals::dark(),
        ThemeMode::Light => egui::Visuals::light(),
    };
    visuals.window_fill = palette.window_fill;
    visuals.panel_fill = palette.panel_fill;
    visuals.extreme_bg_color = palette.extreme_bg;
    visuals.faint_bg_color = palette.faint_bg;
    visuals.code_bg_color = palette.code_bg;
    // Explicit widget foregrounds keep disabled controls readable after theme switches.
    visuals.override_text_color = Some(palette.override_text.unwrap_or(palette.text));

    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, palette.text);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, palette.text);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, palette.text);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, palette.text);

    visuals.selection.bg_fill = palette.selection_fill;
    visuals.selection.stroke = palette.selection_stroke;

    visuals.widgets.noninteractive.bg_fill = palette.widget_noninteractive_fill;
    visuals.widgets.noninteractive.bg_stroke = palette.widget_noninteractive_stroke;
    visuals.widgets.inactive.bg_fill = palette.widget_inactive_fill;
    visuals.widgets.inactive.bg_stroke = palette.widget_inactive_stroke;
    visuals.widgets.hovered.bg_fill = palette.widget_hovered_fill;
    visuals.widgets.hovered.bg_stroke = palette.widget_hovered_stroke;
    visuals.widgets.active.bg_fill = palette.widget_active_fill;
    visuals.widgets.active.bg_stroke = palette.widget_active_stroke;
    visuals.widgets.open.bg_fill = palette.widget_open_fill;
}

/// 从设置创主题
#[allow(dead_code)]
pub fn theme_from_settings(dark_mode: bool) -> ThemeConfig {
    if dark_mode {
        ThemeConfig {
            name: "Dark".to_string(),
            dark_mode,
            colors: ColorPalette::dark(),
            ..ThemeConfig::default()
        }
    } else {
        ThemeConfig {
            name: "Light".to_string(),
            dark_mode,
            colors: ColorPalette::light(),
            ..ThemeConfig::default()
        }
    }
}
