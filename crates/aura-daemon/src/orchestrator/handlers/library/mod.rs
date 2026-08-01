use std::sync::{Arc, Mutex};
use tracing::info;

use aura_ipc::protocol::Response;
use aura_storage::LibraryScanner;

use super::OrchestratorState;

mod copy;
mod files;
mod list;

pub(super) use files::{
    handle_delete_wallpaper, handle_import_files, handle_set_wallpaper_library,
};
use list::build_wallpaper_list;

pub(super) fn handle_list_wallpapers(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
    let items = {
        let state = match state_lock.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };
        info!(
            "ListWallpapers requested — returning {} wallpaper(s)",
            state.library_items.len()
        );
        state.library_items.clone()
    };
    Response::WallpaperList(build_wallpaper_list(&items))
}

pub(super) fn do_refresh(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
    let library_path = {
        let state = match state_lock.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };
        let config = state.config_store.load().unwrap_or_default();
        config.library.library_path
    };

    if !library_path.is_dir() {
        return Response::Error {
            reason: format!(
                "Library path is not a directory: {}",
                aura_security::redact_path(&library_path)
            ),
        };
    }

    let scanned = LibraryScanner::scan_paths(&[library_path]);

    {
        let mut state = match state_lock.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };
        state.library_items = scanned.clone();
        state.library_items.shrink_to_fit();
        if let Err(e) = state.library_store.save(&state.library_items) {
            tracing::error!("Failed to save refreshed library cache: {}", e);
            return Response::Error {
                reason: format!("Failed to save library cache: {}", e),
            };
        }
        info!(
            "RefreshLibrary complete — {} wallpaper(s) in library",
            state.library_items.len()
        );
    }

    Response::WallpaperList(build_wallpaper_list(&scanned))
}

pub(super) fn handle_refresh_library(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
    do_refresh(state_lock)
}

pub(super) fn handle_get_wallpaper_library(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
    let path = {
        let state = match state_lock.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };
        let config = state.config_store.load().unwrap_or_default();
        config.library.library_path
    };
    Response::LibraryPath(path)
}
