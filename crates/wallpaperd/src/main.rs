//! `wallpaperd` — Aura wallpaper daemon.
//!
//! Process responsibilities:
//! - Own the ProcessSingleton (mutex).
//! - Own the Win32 event pump thread.
//! - Own WorkerW attachment state.
//! - Own per-monitor HostWindows and MonitorRenderers.
//! - Own the IPC server.
//! - Dispatch PlaybackCommands to decode worker threads.

#![allow(dead_code)]

mod assignment;
mod decode_worker;
mod orchestrator;
mod perf;
mod recovery;
mod render_coordinator;

#[cfg(target_os = "windows")]
mod daemon;

fn main() {
    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wallpaperd=info,aura_platform_windows=info".into()),
        )
        .init();

    let wallpaper_path = std::env::args().nth(1).map(std::path::PathBuf::from);

    #[cfg(target_os = "windows")]
    {
        // NOTE: deliberately NOT calling enable_dpi_awareness() process-wide here.
        // Setting process-wide PER_MONITOR_AWARE_V2 changes the DPI awareness
        // context of every window this process creates, and SetParent() fails
        // with ERROR_INVALID_PARAMETER (0x80070057) when the child and new
        // parent windows have different DPI awareness contexts — which broke
        // reparenting into Explorer's WorkerW on Explorer-restart recovery.
        // Correct virtual-screen metrics are obtained with a thread-scoped
        // context switch (see workerw::with_virtual_screen_metrics) only around
        // the specific GetSystemMetrics calls that need it.
        tracing::info!("wallpaperd starting");
        if let Err(e) = daemon::run(wallpaper_path) {
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
