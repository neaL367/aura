pub mod importer;
pub mod types;
pub mod worker;

use std::path::PathBuf;
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
    last_error: Arc<Mutex<Option<String>>>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<Request>,
    toast_tx: std::sync::mpsc::Sender<ToastEvent>,
}

impl UiIpcClient {
    pub fn new(ctx: egui::Context, toast_tx: std::sync::mpsc::Sender<ToastEvent>) -> Self {
        let status = Arc::new(Mutex::new(ConnectionStatus::Connecting));
        let wallpapers = Arc::new(Mutex::new(Vec::new()));
        let config = Arc::new(Mutex::new(None));
        let last_error = Arc::new(Mutex::new(None));

        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<Request>();

        spawn_ipc_worker(
            ctx,
            status.clone(),
            wallpapers.clone(),
            config.clone(),
            last_error.clone(),
            cmd_rx,
            toast_tx.clone(),
        );

        Self {
            status,
            wallpapers,
            config,
            last_error,
            cmd_tx,
            toast_tx,
        }
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

    #[allow(dead_code)]
    pub fn fetch_wallpapers(&self) {
        self.send(Request::ListWallpapers);
    }

    pub fn library_path(&self) -> Option<PathBuf> {
        self.config().map(|c| c.library.library_path)
    }

    /// Import files into the managed library: copies them to `library_path` on
    /// the UI side (so the current user's permissions apply), then sends a
    /// `RefreshLibrary` IPC request so the daemon rescans.
    pub fn import_files(&self, paths: Vec<PathBuf>) {
        let cmd_tx = self.cmd_tx.clone();
        import_files_to_library(
            paths,
            self.library_path(),
            &self.toast_tx,
            &self.last_error,
            move || {
                let _ = cmd_tx.send(Request::RefreshLibrary);
            },
        );
    }

    pub fn send(&self, req: Request) {
        let _ = self.cmd_tx.send(req);
    }
}
