//! `wallpaperd` — Aura wallpaper daemon (binary entry point).
//!
//! All logic is in the `wallpaperd` library crate. This file is a thin
//! wrapper that initialises logging and delegates to `daemon::run`.

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wallpaperd=info,aura_platform_windows=info".into()),
        )
        .init();

    let wallpaper_path = std::env::args().nth(1).map(std::path::PathBuf::from);

    #[cfg(target_os = "windows")]
    {
        if let Err(e) = aura_platform_windows::enable_dpi_awareness() {
            tracing::warn!("Failed to enable process-wide DPI awareness: {}", e);
        }
        tracing::info!("wallpaperd starting");
        if let Err(e) = wallpaperd::daemon::run(wallpaper_path) {
            tracing::error!("wallpaperd exited with error: {}", e);
            std::process::exit(1);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        tracing::error!("wallpaperd is only supported on Windows");
        std::process::exit(1);
    }
}
