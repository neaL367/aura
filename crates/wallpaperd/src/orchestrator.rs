use crate::assignment::AssignmentManager;
use aura_ipc::protocol::{DaemonStatus, PROTOCOL_VERSION, Request, Response};
use std::sync::{Arc, Mutex};

pub(crate) struct OrchestratorState {
    pub is_paused: bool,
    pub assignments: AssignmentManager,
    pub active_monitors: usize,
}

#[derive(Clone)]
pub(crate) struct Orchestrator {
    state: Arc<Mutex<OrchestratorState>>,
    shutdown_tx: crossbeam_channel::Sender<()>,
}

impl Orchestrator {
    pub fn new(active_monitors: usize, shutdown_tx: crossbeam_channel::Sender<()>) -> Self {
        Self {
            state: Arc::new(Mutex::new(OrchestratorState {
                is_paused: false,
                assignments: AssignmentManager::new(),
                active_monitors,
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
            Request::ListWallpapers => Response::WallpaperList(Vec::new()),
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
            Request::RefreshLibrary => Response::Ok,
            Request::Shutdown => {
                let _ = self.shutdown_tx.send(());
                Response::Ok
            }
        }
    }
}
