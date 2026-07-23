use std::sync::{Arc, Mutex};
use tracing::info;

use aura_core::monitor::MonitorId;
use aura_core::wallpaper::{FitMode, WallpaperId};
use aura_ipc::protocol::Response;

use super::OrchestratorState;
use crate::render_thread::RenderCommand;

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
            let tx = state.wallpaper_txs.get(&monitor_id).cloned();
            match tx {
                Some(tx) => {
                    info!(
                        "Assigning wallpaper {:?} (fit_mode: {:?}) to monitor {:?}",
                        meta.path, fit_mode, monitor_id
                    );
                    if let Err(e) =
                        persist_assignment_config(&mut state, monitor_id, wallpaper_id, fit_mode)
                    {
                        return Response::Error { reason: e };
                    }
                    state.assignments.assign(monitor_id, wallpaper_id);

                    if tx
                        .send(RenderCommand::SetWallpaper {
                            path: meta.path,
                            fit_mode,
                        })
                        .is_err()
                    {
                        tracing::error!("Render thread for monitor {:?} is gone", monitor_id);
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
                    let monitor_exists = state.monitors.iter().any(|m| m.id == monitor_id);
                    if monitor_exists {
                        info!(
                            "Saved assignment for monitor {:?} (render channel initializing): {:?}",
                            monitor_id, meta.path
                        );
                        if let Err(e) = persist_assignment_config(
                            &mut state,
                            monitor_id,
                            wallpaper_id,
                            fit_mode,
                        ) {
                            return Response::Error { reason: e };
                        }
                        state.assignments.assign(monitor_id, wallpaper_id);
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

    let tx = state.wallpaper_txs.get(&monitor_id).cloned();
    match tx {
        Some(tx) => {
            info!(
                "Setting fit mode {:?} for monitor {:?}",
                fit_mode, monitor_id
            );
            if let Err(e) = state.mutate_config(|config| {
                if let Some(pos) = config
                    .assignments
                    .iter()
                    .position(|a| a.monitor_id == monitor_id)
                {
                    config.assignments[pos].fit_mode = fit_mode;
                }
            }) {
                tracing::error!("Failed to persist fit mode: {}", e);
                return Response::Error {
                    reason: format!("Failed to save fit mode: {}", e),
                };
            }
            if tx.send(RenderCommand::SetFitMode(fit_mode)).is_err() {
                return Response::Error {
                    reason: format!("render thread for monitor {:?} is not running", monitor_id),
                };
            }
            Response::Ok
        }
        None => Response::Error {
            reason: format!("unknown monitor {:?}", monitor_id),
        },
    }
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

    if let Err(e) = state.mutate_config(|config| {
        if let Some(pos) = config
            .assignments
            .iter()
            .position(|a| a.monitor_id == monitor_id)
        {
            config.assignments.remove(pos);
        }
    }) {
        tracing::error!("Failed to persist assignment removal: {}", e);
        return Response::Error {
            reason: format!("Failed to save assignment removal: {}", e),
        };
    }
    state.assignments.remove(&monitor_id);
    Response::Ok
}

pub(super) fn handle_set_playback(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    monitor_id: MonitorId,
    command: aura_core::playback::PlaybackCommand,
) -> Response {
    let state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };

    let tx = state.wallpaper_txs.get(&monitor_id).cloned();
    match tx {
        Some(tx) => {
            info!(
                "Forwarding playback command {:?} to monitor {:?}",
                command, monitor_id
            );
            if tx.send(RenderCommand::Playback(command)).is_err() {
                Response::Error {
                    reason: format!("render thread for monitor {:?} is not running", monitor_id),
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

/// Helper function to deduplicate persisting assignment state to `aura.toml`.
fn persist_assignment_config(
    state: &mut OrchestratorState,
    monitor_id: MonitorId,
    wallpaper_id: WallpaperId,
    fit_mode: Option<FitMode>,
) -> Result<(), String> {
    let effective_fit = fit_mode.unwrap_or_default();
    state
        .mutate_config(|config| {
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
                config
                    .assignments
                    .push(aura_core::monitor::MonitorAssignment {
                        monitor_id,
                        wallpaper_id,
                        fit_mode: effective_fit,
                    });
            }
        })
        .map_err(|e| {
            tracing::error!("Failed to persist wallpaper assignment: {}", e);
            format!("Failed to save assignment: {}", e)
        })?;
    Ok(())
}
