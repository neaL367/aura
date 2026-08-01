use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

use aura_core::wallpaper::WallpaperId;
use aura_ipc::protocol::Response;
use aura_security::validate_path;

use super::{OrchestratorState, copy::copy_file_robust, do_refresh};

pub(crate) fn handle_import_files(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    paths: Vec<PathBuf>,
) -> Response {
    let validated_paths: Vec<PathBuf> = paths
        .iter()
        .filter_map(|p| match validate_path(p) {
            Ok(validated) => Some(validated),
            Err(e) => {
                tracing::warn!(
                    "Rejected import path {}: {}",
                    aura_security::redact_path(p),
                    e
                );
                None
            }
        })
        .collect();

    if validated_paths.is_empty() {
        return Response::Error {
            reason: "No valid import paths provided".to_string(),
        };
    }

    let mut library_path = {
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

    if !library_path.is_dir() && std::fs::create_dir_all(&library_path).is_err() {
        library_path = aura_core::config::default_library_path();
        let _ = std::fs::create_dir_all(&library_path);
    }

    let mut imported = 0;
    let mut errors = Vec::new();
    for src in &validated_paths {
        let file_name = match src.file_name() {
            Some(n) => n,
            None => continue,
        };
        let dest = library_path.join(file_name);

        match copy_file_robust(src, &dest) {
            Ok(()) => {
                imported += 1;
            }
            Err(copy_err) => {
                errors.push(format!("{}: {}", aura_security::redact_path(src), copy_err));
            }
        }
    }

    if imported == 0 && !errors.is_empty() {
        return Response::Error {
            reason: format!("Could not copy files: {}", errors.join("; ")),
        };
    }

    if !errors.is_empty() {
        tracing::warn!(
            "ImportFiles: imported {} file(s), {} error(s): {:?}",
            imported,
            errors.len(),
            errors
        );
    } else {
        info!("ImportFiles: imported {} file(s) into library", imported);
    }

    do_refresh(state_lock)
}

pub(crate) fn handle_set_wallpaper_library(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    path: PathBuf,
) -> Response {
    let validated_path = match validate_path(&path) {
        Ok(p) => p,
        Err(e) => {
            return Response::Error {
                reason: format!("Invalid library path: {}", e),
            };
        }
    };

    {
        let mut state = match state_lock.lock() {
            Ok(s) => s,
            Err(e) => {
                return Response::Error {
                    reason: e.to_string(),
                };
            }
        };

        if let Err(e) = state.mutate_config(|config| {
            config.library.library_path = validated_path.clone();
        }) {
            tracing::error!("Failed to update library path: {}", e);
            return Response::Error {
                reason: format!("Failed to update library path: {}", e),
            };
        }

        if let Some(watcher) = &mut state.watcher {
            watcher.replace_paths(std::slice::from_ref(&validated_path));
        }
    }

    do_refresh(state_lock)
}

pub(crate) fn handle_delete_wallpaper(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    id: WallpaperId,
) -> Response {
    let mut state = match state_lock.lock() {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                reason: e.to_string(),
            };
        }
    };

    let pos = match state.library_items.iter().position(|item| item.id == id) {
        Some(p) => p,
        None => {
            return Response::Error {
                reason: format!("Wallpaper {:?} not found in library", id),
            };
        }
    };

    let item = state.library_items.remove(pos);
    info!("DeleteWallpaper: removing {:?} from library", item.path);

    // Delete the on-disk wallpaper file.
    let _ = std::fs::remove_file(&item.path);

    // Remove any thumbnail for this wallpaper.
    let thumb_dir = aura_storage::ThumbnailStore::thumbs_dir();
    let thumb_path = thumb_dir.join(format!("{}.jpg", item.id));
    let _ = std::fs::remove_file(&thumb_path);

    state.library_items.shrink_to_fit();
    if let Err(e) = state.library_store.save(&state.library_items) {
        tracing::error!("Failed to save library after deletion: {}", e);
        return Response::Error {
            reason: format!("Failed to save library: {}", e),
        };
    }

    info!(
        "DeleteWallpaper complete — {} wallpaper(s) remaining",
        state.library_items.len()
    );

    Response::WallpaperList(super::build_wallpaper_list(&state.library_items))
}
