fn main() -> eframe::Result<()> {
    dirforge_telemetry::init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "DirForge",
        options,
        Box::new(|cc| Ok(Box::new(dirforge_ui::DirForgeNativeApp::new(cc)))),
    )
}
