//! `wallpaper-ui` — Aura control panel UI.
//!
//! Built with `egui`/`eframe` (immediate mode, GPU-accelerated via wgpu).
//!
//! **Dependency boundary**: This crate must NOT depend on
//! `aura-media`, `aura-platform-windows`, or `aura-renderer-vulkan`.
//! All daemon communication goes through `aura-ipc`.

mod app;
mod ipc_client;
mod library_panel;
mod monitor_panel;
mod settings_panel;
mod status_bar;

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
