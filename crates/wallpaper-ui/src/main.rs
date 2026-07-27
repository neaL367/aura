#![allow(dead_code)]

mod app;
mod gallery;
mod inspector;
mod ipc_client;
mod settings_panel;
mod sidebar;
mod status_bar;
mod theme;
mod toast;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wallpaper_ui=info".into()),
        )
        .init();

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
