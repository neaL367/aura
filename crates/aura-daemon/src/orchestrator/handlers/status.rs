use std::sync::{Arc, Mutex};
use tracing::info;

#[cfg(target_os = "windows")]
use aura_win::set_autostart;

use aura_core::config::AppConfig;
use aura_ipc::protocol::{DaemonStatus, PROTOCOL_VERSION, Response};

use super::OrchestratorState;
use crate::render_thread::RenderCommand;

pub(super) fn handle_get_status(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
    let state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };
    Response::Status(DaemonStatus {
        protocol_version: PROTOCOL_VERSION,
        active_monitors: state.active_monitors,
        assigned_wallpapers: state.assignments.all().len(),
        is_paused: state.is_paused,
        monitors: state.monitors.clone(),
    })
}

pub(super) fn handle_get_config(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
    let state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };
    let config = state.config_store.load().unwrap_or_default();
    Response::Config(config)
}

pub(super) fn handle_update_config(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    mut config: AppConfig,
) -> Response {
    info!("UpdateConfig received — saving config & broadcasting performance parameters");
    let corrections = config.validate();
    for c in &corrections {
        tracing::warn!("Config validation correction: {}", c);
    }
    let mut state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };
    if let Err(e) = state.config_store.save(&config) {
        tracing::error!("Failed to save config: {}", e);
        return Response::Error {
            reason: e.to_string(),
        };
    }

    #[cfg(target_os = "windows")]
    set_autostart(config.appearance.auto_start);

    // Apply performance parameters to all render threads.
    for tx in state.wallpaper_txs.values() {
        let _ = tx.send(RenderCommand::SetTargetFps(config.performance.target_fps));
        let _ = tx.send(RenderCommand::SetPerformanceProfile(
            config.performance.default_profile,
        ));
    }

    // Reconcile assignments from the incoming config.
    state.assignments.clear();
    for assignment in &config.assignments {
        state
            .assignments
            .assign(assignment.monitor_id, assignment.wallpaper_id);
        if let Some(tx) = state.wallpaper_txs.get(&assignment.monitor_id)
            && let Some(item) = state
                .library_items
                .iter()
                .find(|i| i.id == assignment.wallpaper_id)
        {
            let _ = tx.send(RenderCommand::SetWallpaper {
                path: item.path.clone(),
                fit_mode: Some(assignment.fit_mode),
            });
        }
    }

    // Sync library watch paths.
    let library_path = config.library.library_path.clone();
    if let Some(watcher) = &mut state.watcher {
        watcher.replace_paths(std::slice::from_ref(&library_path));
    }

    Response::Config(config)
}
