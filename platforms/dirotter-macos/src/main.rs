fn main() -> eframe::Result<()> {
    dirotter_telemetry::init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DirOtter (macOS)")
            .with_inner_size([1600.0, 960.0])
            .with_min_inner_size([1280.0, 760.0]),
        ..Default::default()
    };

    eframe::run_native(
        "DirOtter (macOS)",
        options,
        Box::new(|cc| Ok(Box::new(dirotter_ui::DirOtterNativeApp::new(cc)))),
    )
}
