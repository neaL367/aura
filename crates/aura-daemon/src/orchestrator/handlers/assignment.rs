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
            if let Err(e) =
                persist_assignment_config(&mut state, monitor_id, wallpaper_id, fit_mode)
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
                    if tx.send(cmd.clone()).is_ok() {
                        return Response::Ok;
                    }
                    tracing::warn!(
                        "Render channel for monitor {:?} was dead; purging and sending to fallback channel",
                        monitor_id
                    );
                    state.wallpaper_txs.remove(&monitor_id);
                }
                None => {
                    info!(
                        "No direct channel for monitor {:?}; attempting fallback delivery",
                        monitor_id
                    );
                }
            }

            // Fallback delivery to any live render channel if direct channel is missing or dead
            if let Some((&fallback_id, fallback_tx)) =
                state.wallpaper_txs.iter().find(|(_, tx)| !tx.is_full())
            {
                info!(
                    "Forwarding wallpaper assignment to active fallback monitor {:?}",
                    fallback_id
                );
                let _ = fallback_tx.send(cmd);
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

    if let Err(e) = persist_fit_mode_config(&mut state, monitor_id, fit_mode) {
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
            if tx.send(cmd.clone()).is_ok() {
                return Response::Ok;
            }
            tracing::warn!(
                "Render channel for monitor {:?} was dead during fit mode update; purging",
                monitor_id
            );
            state.wallpaper_txs.remove(&monitor_id);
        }
        None => {
            info!(
                "No direct channel for monitor {:?}; updating fit mode config",
                monitor_id
            );
        }
    }

    if let Some((_, fallback_tx)) = state.wallpaper_txs.iter().find(|(_, tx)| !tx.is_full()) {
        let _ = fallback_tx.send(cmd);
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

    if let Err(e) = persist_remove_assignment_config(&mut state, monitor_id) {
        return Response::Error { reason: e };
    }
    state.assignments.remove(&monitor_id);
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
            if tx.send(cmd.clone()).is_ok() {
                return Response::Ok;
            }
            tracing::warn!(
                "Render channel for monitor {:?} was dead during playback command; purging",
                monitor_id
            );
            state.wallpaper_txs.remove(&monitor_id);
        }
        None => {
            info!(
                "No direct channel for monitor {:?}; attempting playback command fallback",
                monitor_id
            );
        }
    }

    if let Some((_, fallback_tx)) = state.wallpaper_txs.iter().find(|(_, tx)| !tx.is_full()) {
        let _ = fallback_tx.send(cmd);
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

/// Helper function to deduplicate persisting fit mode updates to `aura.toml`.
fn persist_fit_mode_config(
    state: &mut OrchestratorState,
    monitor_id: MonitorId,
    fit_mode: FitMode,
) -> Result<(), String> {
    state
        .mutate_config(|config| {
            if let Some(pos) = config
                .assignments
                .iter()
                .position(|a| a.monitor_id == monitor_id)
            {
                config.assignments[pos].fit_mode = fit_mode;
            }
        })
        .map_err(|e| {
            tracing::error!("Failed to persist fit mode: {}", e);
            format!("Failed to save fit mode: {}", e)
        })?;
    Ok(())
}

/// Helper function to deduplicate persisting assignment removals from `aura.toml`.
fn persist_remove_assignment_config(
    state: &mut OrchestratorState,
    monitor_id: MonitorId,
) -> Result<(), String> {
    state
        .mutate_config(|config| {
            if let Some(pos) = config
                .assignments
                .iter()
                .position(|a| a.monitor_id == monitor_id)
            {
                config.assignments.remove(pos);
            }
        })
        .map_err(|e| {
            tracing::error!("Failed to persist assignment removal: {}", e);
            format!("Failed to save assignment removal: {}", e)
        })?;
    Ok(())
}
