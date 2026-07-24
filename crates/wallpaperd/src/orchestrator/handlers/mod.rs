pub mod assignment;
pub mod library;
pub mod status;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aura_core::monitor::MonitorId;
use aura_core::wallpaper::WallpaperMeta;
use aura_ipc::protocol::{MonitorSummary, Request, Response};
use aura_storage::{LibraryWatcher, config_store::ConfigStore, library_store::LibraryStore};

use crate::assignment::AssignmentManager;
use crate::render_thread::RenderCommand;

pub struct OrchestratorState {
    pub is_paused: bool,
    pub assignments: AssignmentManager,
    pub active_monitors: usize,
    pub monitors: Vec<MonitorSummary>,
    pub library_items: Vec<WallpaperMeta>,
    pub config_store: ConfigStore,
    pub library_store: LibraryStore,
    pub wallpaper_txs: HashMap<MonitorId, crossbeam_channel::Sender<RenderCommand>>,
    pub watcher: Option<LibraryWatcher>,
}

impl OrchestratorState {
    pub fn mutate_config<F>(
        &mut self,
        mutator: F,
    ) -> Result<aura_core::config::AppConfig, aura_storage::error::StorageError>
    where
        F: FnOnce(&mut aura_core::config::AppConfig),
    {
        let mut config = self.config_store.load().unwrap_or_default();
        mutator(&mut config);
        self.config_store.save(&config)?;
        Ok(config)
    }
}

pub fn handle_request(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    shutdown_tx: &crossbeam_channel::Sender<()>,
    request: Request,
) -> Response {
    match request {
        Request::GetStatus => status::handle_get_status(state_lock),
        Request::ListWallpapers => library::handle_list_wallpapers(state_lock),
        Request::AssignWallpaper {
            monitor_id,
            wallpaper_id,
            fit_mode,
        } => assignment::handle_assign_wallpaper(state_lock, monitor_id, wallpaper_id, fit_mode),
        Request::SetFitMode {
            monitor_id,
            fit_mode,
        } => assignment::handle_set_fit_mode(state_lock, monitor_id, fit_mode),
        Request::RemoveAssignment { monitor_id } => {
            assignment::handle_remove_assignment(state_lock, monitor_id)
        }
        Request::SetPlayback {
            monitor_id,
            command,
        } => assignment::handle_set_playback(state_lock, monitor_id, command),
        Request::PauseAll => assignment::handle_set_paused(state_lock, true),
        Request::ResumeAll => assignment::handle_set_paused(state_lock, false),
        Request::RefreshLibrary => library::handle_refresh_library(state_lock),
        Request::AddScanPath { path } => library::handle_update_scan_path(state_lock, path, true),
        Request::RemoveScanPath { path } => {
            library::handle_update_scan_path(state_lock, path, false)
        }
        Request::GetConfig => status::handle_get_config(state_lock),
        Request::UpdateConfig { config } => status::handle_update_config(state_lock, config),
        Request::Shutdown => {
            let _ = shutdown_tx.send(());
            Response::Ok
        }
    }
}
