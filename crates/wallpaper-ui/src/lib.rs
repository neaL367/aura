pub mod app;
pub mod gallery;
pub mod inspector;
pub mod ipc_client;
pub mod settings_panel;
pub mod sidebar;
pub mod status_bar;
pub mod theme;
pub mod toast;

pub fn run() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Aura Wallpaper")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Aura Wallpaper",
        native_options,
        Box::new(|cc| Ok(Box::new(app::AuraApp::new(cc)))),
    )
    .expect("eframe failed to start");
}
