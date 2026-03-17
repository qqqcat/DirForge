fn main() -> eframe::Result<()> {
    dirforge_telemetry::init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1680.0, 980.0])
            .with_min_inner_size([1360.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DirForge",
        options,
        Box::new(|cc| Ok(Box::new(dirforge_ui::DirForgeNativeApp::new(cc)))),
    )
}
