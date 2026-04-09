#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() -> eframe::Result<()> {
    dirotter_telemetry::init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1680.0, 980.0])
            .with_min_inner_size([1360.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DirOtter",
        options,
        Box::new(|cc| Ok(Box::new(dirotter_ui::DirOtterNativeApp::new(cc)))),
    )
}
