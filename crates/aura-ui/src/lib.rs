pub mod action;
pub mod app;
pub mod canvas;
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
            .with_title(aura_core::WINDOW_TITLE)
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    if let Err(err) = eframe::run_native(
        aura_core::WINDOW_TITLE,
        native_options,
        Box::new(|cc| Ok(Box::new(app::AuraApp::new(cc)))),
    ) {
        tracing::error!("eframe UI failed: {err}");
    }
}
