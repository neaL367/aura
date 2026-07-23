use crate::assignment::AssignmentManager;
use crate::render_thread::RenderCommand;
use aura_core::monitor::MonitorId;
use aura_core::wallpaper::WallpaperMeta;
use aura_ipc::protocol::{DaemonStatus, MonitorSummary, PROTOCOL_VERSION, Request, Response};
use aura_storage::{LibraryScanner, config_store::ConfigStore, library_store::LibraryStore};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct OrchestratorState {
    pub is_paused: bool,
    pub assignments: AssignmentManager,
    pub active_monitors: usize,
    pub monitors: Vec<MonitorSummary>,
    pub library_items: Vec<WallpaperMeta>,
    pub config_store: ConfigStore,
    pub library_store: LibraryStore,
    pub wallpaper_txs: HashMap<MonitorId, crossbeam_channel::Sender<RenderCommand>>,
}

#[derive(Clone)]
pub struct Orchestrator {
    state: Arc<Mutex<OrchestratorState>>,
    shutdown_tx: crossbeam_channel::Sender<()>,
}

impl Orchestrator {
    pub fn new(
        monitors: Vec<MonitorSummary>,
        wallpaper_txs: HashMap<MonitorId, crossbeam_channel::Sender<RenderCommand>>,
        shutdown_tx: crossbeam_channel::Sender<()>,
    ) -> Self {
        let active_monitors = monitors.len();
        let config_path = ConfigStore::default_path();
        let config_store = ConfigStore::new(&config_path);
        let mut config = config_store.load().unwrap_or_default();
        if config.library.scan_paths.is_empty() {
            config.library.scan_paths = aura_core::config::LibraryConfig::default().scan_paths;
            let _ = config_store.save(&config);
        }

        let library_path = config_path.with_file_name("library.json");
        let library_store = LibraryStore::new(&library_path);

        // Load cached library metadata immediately for sub-millisecond startup
        let library_items = library_store.load().unwrap_or_default();

        let mut assignments = AssignmentManager::new();
        for a in &config.assignments {
            assignments.assign(a.monitor_id, a.wallpaper_id);
        }

        info!(
            "Orchestrator initialized — {} wallpaper(s) in cached library, {} monitor(s)",
            library_items.len(),
            monitors.len()
        );

        let orchestrator = Self {
            state: Arc::new(Mutex::new(OrchestratorState {
                is_paused: false,
                assignments,
                active_monitors,
                monitors,
                library_items,
                config_store,
                library_store,
                wallpaper_txs,
            })),
            shutdown_tx,
        };

        // Asynchronously rescan configured library scan paths in a background thread
        // so daemon startup and IPC server initialization are never blocked by filesystem I/O.
        let orch_bg = orchestrator.clone();
        std::thread::Builder::new()
            .name("library-rescan".into())
            .spawn(move || {
                let scan_paths = {
                    let state = orch_bg.state.lock().unwrap();
                    let config = state.config_store.load().unwrap_or_default();
                    config.library.scan_paths
                };
                if !scan_paths.is_empty() {
                    info!("Background library rescan starting...");
                    let scanned = LibraryScanner::scan_paths(&scan_paths);
                    // Generate thumbnails before moving scanned into state (avoids clone).
                    for meta in &scanned {
                        aura_storage::ThumbnailStore::get_or_create(meta);
                    }
                    if let Ok(mut state) = orch_bg.state.lock() {
                        state.library_items = scanned;
                        let _ = state.library_store.save(&state.library_items);
                        info!(
                            "Background library rescan complete — {} wallpaper(s), thumbnails ready",
                            state.library_items.len()
                        );
                    }
                }
            })
            .expect("failed to spawn library rescan thread");

        orchestrator
    }

    pub fn is_paused(&self) -> bool {
        self.state.lock().unwrap().is_paused
    }

    pub fn update_monitors(
        &self,
        monitors: Vec<MonitorSummary>,
        wallpaper_txs: HashMap<MonitorId, crossbeam_channel::Sender<RenderCommand>>,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.active_monitors = monitors.len();
            state.monitors = monitors;
            state.wallpaper_txs = wallpaper_txs;
            info!(
                "Orchestrator monitors updated — now active: {} monitor(s)",
                state.active_monitors
            );
        }
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
                monitors: state.monitors.clone(),
            }),
            Request::ListWallpapers => {
                info!(
                    "ListWallpapers requested — returning {} wallpaper(s)",
                    state.library_items.len()
                );
                Response::WallpaperList(build_wallpaper_list(&state.library_items))
            }
            Request::AssignWallpaper {
                monitor_id,
                wallpaper_id,
                fit_mode,
            } => {
                let wallpaper_meta = state
                    .library_items
                    .iter()
                    .find(|item| item.id == wallpaper_id)
                    .cloned();
                match wallpaper_meta {
                    Some(meta) => {
                        let tx = state.wallpaper_txs.get(&monitor_id).cloned();
                        match tx {
                            Some(tx) => {
                                info!(
                                    "Assigning wallpaper {:?} (fit_mode: {:?}) to monitor {:?}",
                                    meta.path, fit_mode, monitor_id
                                );
                                state.assignments.assign(monitor_id, wallpaper_id);
                                let mut config = state.config_store.load().unwrap_or_default();
                                let effective_fit = fit_mode.unwrap_or_default();
                                if let Some(pos) = config
                                    .assignments
                                    .iter()
                                    .position(|a| a.monitor_id == monitor_id)
                                {
                                    config.assignments[pos].wallpaper_id = wallpaper_id;
                                    if fit_mode.is_some() {
                                        config.assignments[pos].fit_mode = effective_fit;
                                    }
                                } else {
                                    config.assignments.push(
                                        aura_core::monitor::MonitorAssignment {
                                            monitor_id,
                                            wallpaper_id,
                                            fit_mode: effective_fit,
                                        },
                                    );
                                }
                                let _ = state.config_store.save(&config);

                                if tx
                                    .send(RenderCommand::SetWallpaper {
                                        path: meta.path,
                                        fit_mode,
                                    })
                                    .is_err()
                                {
                                    tracing::error!(
                                        "Render thread for monitor {:?} is gone",
                                        monitor_id
                                    );
                                    return Response::Error {
                                        reason: format!(
                                            "render thread for monitor {:?} is not running",
                                            monitor_id
                                        ),
                                    };
                                }
                                Response::Ok
                            }
                            None => {
                                let monitor_exists =
                                    state.monitors.iter().any(|m| m.id == monitor_id);
                                if monitor_exists {
                                    info!(
                                        "Saved assignment for monitor {:?} (render channel initializing): {:?}",
                                        monitor_id, meta.path
                                    );
                                    state.assignments.assign(monitor_id, wallpaper_id);
                                    let mut config = state.config_store.load().unwrap_or_default();
                                    let effective_fit = fit_mode.unwrap_or_default();
                                    if let Some(pos) = config
                                        .assignments
                                        .iter()
                                        .position(|a| a.monitor_id == monitor_id)
                                    {
                                        config.assignments[pos].wallpaper_id = wallpaper_id;
                                        if fit_mode.is_some() {
                                            config.assignments[pos].fit_mode = effective_fit;
                                        }
                                    } else {
                                        config.assignments.push(
                                            aura_core::monitor::MonitorAssignment {
                                                monitor_id,
                                                wallpaper_id,
                                                fit_mode: effective_fit,
                                            },
                                        );
                                    }
                                    let _ = state.config_store.save(&config);
                                    Response::Ok
                                } else {
                                    tracing::warn!("No channel found for monitor {:?}", monitor_id);
                                    Response::Error {
                                        reason: format!("unknown monitor {:?}", monitor_id),
                                    }
                                }
                            }
                        }
                    }
                    None => Response::Error {
                        reason: "wallpaper not found".into(),
                    },
                }
            }
            Request::SetFitMode {
                monitor_id,
                fit_mode,
            } => {
                let tx = state.wallpaper_txs.get(&monitor_id).cloned();
                match tx {
                    Some(tx) => {
                        info!(
                            "Setting fit mode {:?} for monitor {:?}",
                            fit_mode, monitor_id
                        );
                        let mut config = state.config_store.load().unwrap_or_default();
                        if let Some(pos) = config
                            .assignments
                            .iter()
                            .position(|a| a.monitor_id == monitor_id)
                        {
                            config.assignments[pos].fit_mode = fit_mode;
                            let _ = state.config_store.save(&config);
                        }
                        if tx.send(RenderCommand::SetFitMode(fit_mode)).is_err() {
                            return Response::Error {
                                reason: format!(
                                    "render thread for monitor {:?} is not running",
                                    monitor_id
                                ),
                            };
                        }
                        Response::Ok
                    }
                    None => Response::Error {
                        reason: format!("unknown monitor {:?}", monitor_id),
                    },
                }
            }
            Request::RemoveAssignment { monitor_id } => {
                state.assignments.remove(&monitor_id);
                let mut config = state.config_store.load().unwrap_or_default();
                if let Some(pos) = config
                    .assignments
                    .iter()
                    .position(|a| a.monitor_id == monitor_id)
                {
                    config.assignments.remove(pos);
                    let _ = state.config_store.save(&config);
                }
                Response::Ok
            }
            Request::SetPlayback {
                monitor_id,
                command,
            } => {
                let tx = state.wallpaper_txs.get(&monitor_id).cloned();
                match tx {
                    Some(tx) => {
                        info!("Forwarding playback command {:?} to monitor {:?}", command, monitor_id);
                        if tx.send(RenderCommand::Playback(command)).is_err() {
                            Response::Error {
                                reason: format!(
                                    "render thread for monitor {:?} is not running",
                                    monitor_id
                                ),
                            }
                        } else {
                            Response::Ok
                        }
                    }
                    None => Response::Error {
                        reason: format!("unknown monitor {:?}", monitor_id),
                    },
                }
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
                Response::WallpaperList(build_wallpaper_list(&state.library_items))
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
                Response::WallpaperList(build_wallpaper_list(&state.library_items))
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
                Response::WallpaperList(build_wallpaper_list(&state.library_items))
            }
            Request::Shutdown => {
                let _ = self.shutdown_tx.send(());
                Response::Ok
            }
        }
    }
}

fn build_wallpaper_list(
    items: &[aura_core::wallpaper::WallpaperMeta],
) -> Vec<aura_ipc::protocol::WallpaperEntry> {
    items
        .iter()
        .map(|meta| {
            let mut entry = aura_ipc::protocol::WallpaperEntry::from(meta);
            entry.thumbnail_path = aura_storage::ThumbnailStore::get_or_create(meta);
            entry
        })
        .collect()
}
