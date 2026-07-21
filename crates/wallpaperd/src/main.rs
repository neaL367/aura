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
mod daemon;
mod decode_worker;
mod orchestrator;
mod perf;
mod recovery;
mod render_coordinator;

fn main() {
    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wallpaperd=info,aura_platform_windows=info".into()),
        )
        .init();

    tracing::info!("wallpaperd starting");

    if let Err(e) = daemon::run() {
        tracing::error!("wallpaperd exited with error: {}", e);
        std::process::exit(1);
    }
}
