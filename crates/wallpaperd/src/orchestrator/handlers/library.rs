use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

use aura_core::wallpaper::WallpaperMeta;
use aura_ipc::protocol::{Response, WallpaperEntry};
use aura_storage::LibraryScanner;

use super::OrchestratorState;

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

pub(super) fn handle_refresh_library(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
    let scan_paths = {
        let state = match state_lock.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };
        let config = state.config_store.load().unwrap_or_default();
        config.library.scan_paths
    };

    let scanned = LibraryScanner::scan_paths(&scan_paths);

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

pub(super) fn handle_update_scan_path(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    path: PathBuf,
    is_add: bool,
) -> Response {
    let scan_paths = {
        let mut state = match state_lock.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };
        let mut paths = Vec::new();
        if let Err(e) = state.mutate_config(|config| {
            if is_add {
                if !config.library.scan_paths.contains(&path) {
                    config.library.scan_paths.push(path.clone());
                }
            } else if let Some(pos) = config.library.scan_paths.iter().position(|p| p == &path) {
                config.library.scan_paths.remove(pos);
            }
            paths = config.library.scan_paths.clone();
        }) {
            tracing::error!("Failed to update scan path: {}", e);
            return Response::Error {
                reason: format!("Failed to update scan path: {}", e),
            };
        }
        paths
    };

    let scanned = LibraryScanner::scan_paths(&scan_paths);
    info!("Rescanned library — now has {} wallpaper(s)", scanned.len());

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
        if let Err(e) = state.library_store.save(&state.library_items) {
            tracing::error!("Failed to save library cache: {}", e);
            return Response::Error {
                reason: format!("Failed to save library cache: {}", e),
            };
        }
    }

    Response::WallpaperList(build_wallpaper_list(&scanned))
}

fn build_wallpaper_list(items: &[WallpaperMeta]) -> Vec<WallpaperEntry> {
    items
        .iter()
        .map(|meta| {
            let mut entry = WallpaperEntry::from(meta);
            entry.thumbnail_path =
                tokio::task::block_in_place(|| aura_storage::ThumbnailStore::get_or_create(meta));
            entry
        })
        .collect()
}
