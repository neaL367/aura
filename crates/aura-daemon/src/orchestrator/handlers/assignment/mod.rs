use std::sync::{Arc, Mutex};
use tracing::info;

use aura_core::monitor::MonitorId;
use aura_core::wallpaper::{FitMode, WallpaperId};
use aura_ipc::protocol::Response;

use super::OrchestratorState;
use crate::render_thread::RenderCommand;

mod persist;

pub(super) fn handle_assign_wallpaper(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    monitor_id: MonitorId,
    wallpaper_id: WallpaperId,
    fit_mode: Option<FitMode>,
) -> Response {
    let mut state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };

    let wallpaper_meta = state
        .library_items
        .iter()
        .find(|item| item.id == wallpaper_id)
        .cloned();

    match wallpaper_meta {
        Some(meta) => {
            if let Err(e) =
                persist::persist_assignment_config(&mut state, monitor_id, wallpaper_id, fit_mode)
            {
                return Response::Error { reason: e };
            }
            state.assignments.assign(monitor_id, wallpaper_id);

            let cmd = RenderCommand::SetWallpaper {
                path: meta.path.clone(),
                fit_mode,
            };

            let tx = state.wallpaper_txs.get(&monitor_id).cloned();
            match tx {
                Some(tx) => {
                    info!(
                        "Assigning wallpaper {:?} (fit_mode: {:?}) to monitor {:?}",
                        meta.path, fit_mode, monitor_id
                    );
                    if tx.send(cmd).is_ok() {
                        return Response::Ok;
                    }
                    tracing::warn!(
                        "Render channel for monitor {:?} was dead; purging",
                        monitor_id
                    );
                    state.wallpaper_txs.remove(&monitor_id);
                }
                None => {
                    tracing::debug!(
                        "No render channel for monitor {:?} yet; assignment persisted, will flush on channel registration",
                        monitor_id
                    );
                }
            }

            Response::Ok
        }
        None => Response::Error {
            reason: "wallpaper not found".into(),
        },
    }
}

pub(super) fn handle_set_fit_mode(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    monitor_id: MonitorId,
    fit_mode: FitMode,
) -> Response {
    let mut state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };

    if let Err(e) = persist::persist_fit_mode_config(&mut state, monitor_id, fit_mode) {
        return Response::Error { reason: e };
    }

    let cmd = RenderCommand::SetFitMode(fit_mode);
    let tx = state.wallpaper_txs.get(&monitor_id).cloned();
    match tx {
        Some(tx) => {
            info!(
                "Setting fit mode {:?} for monitor {:?}",
                fit_mode, monitor_id
            );
            if tx.send(cmd).is_ok() {
                return Response::Ok;
            }
            tracing::warn!(
                "Render channel for monitor {:?} was dead during fit mode update; purging",
                monitor_id
            );
            state.wallpaper_txs.remove(&monitor_id);
        }
        None => {
            tracing::debug!(
                "No render channel for monitor {:?} yet; fit mode persisted",
                monitor_id
            );
        }
    }

    Response::Ok
}

pub(super) fn handle_remove_assignment(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    monitor_id: MonitorId,
) -> Response {
    let mut state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };

    if let Err(e) = persist::persist_remove_assignment_config(&mut state, monitor_id) {
        return Response::Error { reason: e };
    }
    state.assignments.remove(&monitor_id);

    let tx = state.wallpaper_txs.get(&monitor_id).cloned();
    if let Some(tx) = tx {
        let _ = tx.send(RenderCommand::Clear);
    }

    Response::Ok
}

pub(super) fn handle_set_playback(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    monitor_id: MonitorId,
    command: aura_core::playback::PlaybackCommand,
) -> Response {
    let mut state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };

    let cmd = RenderCommand::Playback(command);
    let tx = state.wallpaper_txs.get(&monitor_id).cloned();
    match tx {
        Some(tx) => {
            info!(
                "Forwarding playback command {:?} to monitor {:?}",
                command, monitor_id
            );
            if tx.send(cmd).is_ok() {
                return Response::Ok;
            }
            tracing::warn!(
                "Render channel for monitor {:?} was dead during playback command; purging",
                monitor_id
            );
            state.wallpaper_txs.remove(&monitor_id);
        }
        None => {
            tracing::debug!(
                "No render channel for monitor {:?} yet; playback command dropped",
                monitor_id
            );
        }
    }

    Response::Ok
}

pub(super) fn handle_set_paused(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    paused: bool,
) -> Response {
    let mut state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };
    state.is_paused = paused;
    Response::Ok
}
