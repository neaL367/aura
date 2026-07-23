use std::sync::{Arc, Mutex};
use tracing::info;

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
    config: AppConfig,
) -> Response {
    info!("UpdateConfig received — saving config & broadcasting performance parameters");
    let state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };
    if let Err(e) = state.config_store.save(&config) {
        tracing::error!("Failed to save config: {}", e);
        Response::Error {
            reason: e.to_string(),
        }
    } else {
        for tx in state.wallpaper_txs.values() {
            let _ = tx.send(RenderCommand::SetTargetFps(config.performance.target_fps));
            let _ = tx.send(RenderCommand::SetPerformanceProfile(
                config.performance.default_profile,
            ));
        }
        Response::Config(config)
    }
}
