use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use aura_win::singleton::ProcessSingleton;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("storage error: {0}")]
    Storage(#[from] aura_storage::StorageError),
    #[error("Vulkan error: {0}")]
    Vulkan(#[from] aura_vulkan::VulkanError),
    #[error("platform error: {0}")]
    Platform(#[from] aura_win::PlatformError),
    #[error("media error: {0}")]
    Media(#[from] aura_media::MediaError),
    #[error("another instance of wallpaperd is already running")]
    AlreadyRunning,
    #[error("event pump channel disconnected")]
    EventPumpDisconnected,
    #[error("failed to spawn render thread")]
    ThreadSpawn,
}

/// Configuration passed into `run()` from the hosting binary.
pub struct DaemonOptions {
    pub wallpaper_path: Option<PathBuf>,
    pub shutdown_rx: crossbeam_channel::Receiver<()>,
    pub ready_tx: std::sync::mpsc::SyncSender<()>,
    pub done_tx: crossbeam_channel::Sender<()>,
    pub _singleton: ProcessSingleton,
    /// Shared Ctrl+C flag. When `Some`, the daemon uses this flag directly
    /// and does NOT register its own console handler (caller owns it).
    /// When `None` (standalone), the daemon creates and registers its own.
    pub ctrlc_flag: Option<Arc<AtomicBool>>,
}

impl DaemonOptions {
    /// Create options suitable for a standalone headless daemon
    /// (wallpaperd -- standalone binary).
    pub fn standalone(wallpaper_path: Option<PathBuf>) -> Self {
        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded::<()>(1);
        // Leak sender so receiver never closes (standalone runs until signal).
        std::mem::forget(shutdown_tx);
        let (ready_tx, _) = std::sync::mpsc::sync_channel(1);
        let (done_tx, _) = crossbeam_channel::bounded(1);
        let singleton =
            ProcessSingleton::acquire().expect("another wallpaperd instance is already running");
        Self {
            wallpaper_path,
            shutdown_rx,
            ready_tx,
            done_tx,
            _singleton: singleton,
            ctrlc_flag: None,
        }
    }
}
