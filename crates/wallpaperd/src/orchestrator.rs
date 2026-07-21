use crate::assignment::AssignmentManager;
use aura_core::wallpaper::WallpaperMeta;
use aura_ipc::protocol::{DaemonStatus, PROTOCOL_VERSION, Request, Response};
use aura_storage::{LibraryScanner, config_store::ConfigStore, library_store::LibraryStore};
use std::sync::{Arc, Mutex};
use tracing::info;

pub(crate) struct OrchestratorState {
    pub is_paused: bool,
    pub assignments: AssignmentManager,
    pub active_monitors: usize,
    pub library_items: Vec<WallpaperMeta>,
    pub config_store: ConfigStore,
    pub library_store: LibraryStore,
}

#[derive(Clone)]
pub(crate) struct Orchestrator {
    state: Arc<Mutex<OrchestratorState>>,
    shutdown_tx: crossbeam_channel::Sender<()>,
}

impl Orchestrator {
    pub fn new(active_monitors: usize, shutdown_tx: crossbeam_channel::Sender<()>) -> Self {
        let config_path = ConfigStore::default_path();
        let config_store = ConfigStore::new(&config_path);
        let mut config = config_store.load().unwrap_or_default();
        if config.library.scan_paths.is_empty() {
            config.library.scan_paths = aura_core::config::LibraryConfig::default().scan_paths;
            let _ = config_store.save(&config);
        }

        let library_path = config_path.with_file_name("library.json");
        let library_store = LibraryStore::new(&library_path);

        let mut library_items = Vec::new();
        if !config.library.scan_paths.is_empty() {
            library_items = LibraryScanner::scan_paths(&config.library.scan_paths);
            let _ = library_store.save(&library_items);
        }

        info!(
            "Orchestrator initialized — {} wallpaper(s) in library",
            library_items.len()
        );

        Self {
            state: Arc::new(Mutex::new(OrchestratorState {
                is_paused: false,
                assignments: AssignmentManager::new(),
                active_monitors,
                library_items,
                config_store,
                library_store,
            })),
            shutdown_tx,
        }
    }

    pub fn is_paused(&self) -> bool {
        self.state.lock().unwrap().is_paused
    }

    pub fn handle_request(&self, request: Request) -> Response {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };

        match request {
            Request::GetStatus => Response::Status(DaemonStatus {
                protocol_version: PROTOCOL_VERSION,
                active_monitors: state.active_monitors,
                assigned_wallpapers: state.assignments.all().len(),
                is_paused: state.is_paused,
            }),
            Request::ListWallpapers => {
                info!(
                    "ListWallpapers requested — returning {} wallpaper(s)",
                    state.library_items.len()
                );
                Response::WallpaperList(state.library_items.iter().map(Into::into).collect())
            }
            Request::AssignWallpaper {
                monitor_id,
                wallpaper_id,
            } => {
                state.assignments.assign(monitor_id, wallpaper_id);
                Response::Ok
            }
            Request::RemoveAssignment { monitor_id } => {
                state.assignments.remove(&monitor_id);
                Response::Ok
            }
            Request::PauseAll => {
                state.is_paused = true;
                Response::Ok
            }
            Request::ResumeAll => {
                state.is_paused = false;
                Response::Ok
            }
            Request::RefreshLibrary => {
                let config = state.config_store.load().unwrap_or_default();
                let scanned = LibraryScanner::scan_paths(&config.library.scan_paths);
                state.library_items = scanned;
                let _ = state.library_store.save(&state.library_items);
                info!(
                    "RefreshLibrary complete — {} wallpaper(s) in library",
                    state.library_items.len()
                );
                Response::WallpaperList(state.library_items.iter().map(Into::into).collect())
            }
            Request::AddScanPath { path } => {
                info!("AddScanPath received for {:?}", path);
                let mut config = state.config_store.load().unwrap_or_default();
                if !config.library.scan_paths.contains(&path) {
                    config.library.scan_paths.push(path.clone());
                    let _ = state.config_store.save(&config);
                }
                let scanned = LibraryScanner::scan_paths(&config.library.scan_paths);
                info!("Rescanned library — now has {} wallpaper(s)", scanned.len());
                state.library_items = scanned;
                let _ = state.library_store.save(&state.library_items);
                Response::WallpaperList(state.library_items.iter().map(Into::into).collect())
            }
            Request::RemoveScanPath { path } => {
                let mut config = state.config_store.load().unwrap_or_default();
                if let Some(pos) = config.library.scan_paths.iter().position(|p| p == &path) {
                    config.library.scan_paths.remove(pos);
                    let _ = state.config_store.save(&config);
                    let scanned = LibraryScanner::scan_paths(&config.library.scan_paths);
                    state.library_items = scanned;
                    let _ = state.library_store.save(&state.library_items);
                }
                Response::WallpaperList(state.library_items.iter().map(Into::into).collect())
            }
            Request::Shutdown => {
                let _ = self.shutdown_tx.send(());
                Response::Ok
            }
        }
    }
}
