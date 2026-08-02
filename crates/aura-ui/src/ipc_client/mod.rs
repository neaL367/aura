pub mod importer;
pub mod types;
pub mod worker;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use aura_ipc::protocol::{Request, WallpaperEntry};

pub use types::ConnectionStatus;

use importer::import_files_to_library;
use worker::spawn_ipc_worker;

use crate::toast::ToastEvent;

pub struct UiIpcClient {
    status: Arc<Mutex<ConnectionStatus>>,
    wallpapers: Arc<Mutex<Vec<WallpaperEntry>>>,
    config: Arc<Mutex<Option<aura_core::config::AppConfig>>>,
    previous_config: Arc<Mutex<Option<aura_core::config::AppConfig>>>,
    last_error: Arc<Mutex<Option<String>>>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<Request>,
    _toast_tx: std::sync::mpsc::Sender<ToastEvent>,
    shutdown_flag: Arc<AtomicBool>,
}

impl UiIpcClient {
    pub fn new(ctx: egui::Context, toast_tx: std::sync::mpsc::Sender<ToastEvent>) -> Self {
        let status = Arc::new(Mutex::new(ConnectionStatus::Connecting));
        let wallpapers = Arc::new(Mutex::new(Vec::new()));
        let config = Arc::new(Mutex::new(None));
        let previous_config = Arc::new(Mutex::new(None));
        let last_error = Arc::new(Mutex::new(None));
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<Request>();

        spawn_ipc_worker(
            ctx,
            status.clone(),
            wallpapers.clone(),
            config.clone(),
            previous_config.clone(),
            last_error.clone(),
            cmd_rx,
            toast_tx.clone(),
            shutdown_flag.clone(),
        );

        Self {
            status,
            wallpapers,
            config,
            previous_config,
            last_error,
            cmd_tx,
            _toast_tx: toast_tx,
            shutdown_flag,
        }
    }

    pub fn shutdown(&self) {
        self.shutdown_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Snapshot the current config so it can be restored on mutation failure.
    fn snapshot_config(&self) {
        let current = self
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        *self
            .previous_config
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = current;
    }

    pub fn status(&self) -> ConnectionStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn wallpapers(&self) -> Vec<WallpaperEntry> {
        self.wallpapers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn config(&self) -> Option<aura_core::config::AppConfig> {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Reason the most recent command failed, if any. Cleared on the next
    /// successful command.
    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn library_path(&self) -> Option<PathBuf> {
        self.config().map(|c| c.library.library_path)
    }

    /// Import files into the managed library by sending a `Request::ImportFiles`
    /// request over IPC to the daemon, invoking its hardened copy and scan pipeline.
    pub fn import_files(&self, paths: Vec<PathBuf>) {
        let cmd_tx = self.cmd_tx.clone();
        import_files_to_library(paths, move |req| {
            let _ = cmd_tx.send(req);
        });
    }

    pub fn assign_wallpaper_optimistic(
        &self,
        monitor_id: aura_core::monitor::MonitorId,
        wallpaper_id: aura_core::wallpaper::WallpaperId,
        fit_mode: aura_core::wallpaper::FitMode,
    ) {
        self.snapshot_config();
        let mut guard = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref mut cfg) = *guard {
            cfg.assignments.retain(|a| a.monitor_id != monitor_id);
            cfg.assignments.push(aura_core::monitor::MonitorAssignment {
                monitor_id,
                wallpaper_id,
                fit_mode,
            });
        }
    }

    pub fn remove_assignment_optimistic(&self, monitor_id: aura_core::monitor::MonitorId) {
        self.snapshot_config();
        let mut guard = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref mut cfg) = *guard {
            cfg.assignments.retain(|a| a.monitor_id != monitor_id);
        }
    }

    /// Update the cached library path immediately so UI reads reflect the new
    /// folder before the daemon's authoritative response arrives.
    pub fn set_library_path_optimistic(&self, path: PathBuf) {
        let mut guard = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref mut cfg) = *guard {
            cfg.library.library_path = path;
        }
    }

    pub fn send(&self, req: Request) {
        let _ = self.cmd_tx.send(req);
    }
}
