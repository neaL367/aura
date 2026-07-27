use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

use aura_core::wallpaper::WallpaperMeta;
use aura_ipc::protocol::{Response, WallpaperEntry};
use aura_security::validate_path;
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

fn do_refresh(state_lock: &Arc<Mutex<OrchestratorState>>) -> Response {
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

pub(super) fn handle_import_files(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    paths: Vec<PathBuf>,
) -> Response {
    let validated_paths: Vec<PathBuf> = paths
        .iter()
        .filter_map(|p| match validate_path(p) {
            Ok(validated) => Some(validated),
            Err(e) => {
                tracing::warn!("Rejected import path {}: {}", aura_security::redact_path(p), e);
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
            Err(copy_err) if LibraryScanner::inspect_file(src).is_some() => {
                imported += 1;
                info!(
                    "ImportFiles: file {} included directly (copy failed: {})",
                    aura_security::redact_path(src),
                    copy_err
                );
            }
            Err(copy_err) => {
                errors.push(format!(
                    "{}: {}",
                    aura_security::redact_path(src),
                    copy_err
                ));
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

pub(super) fn handle_set_wallpaper_library(
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

fn copy_file_robust(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    if dest.exists() {
        // Clear stale leftover file: remove it entirely so we don't inherit
        // restrictive ACLs, lock state, or a readonly attribute from a
        // previous failed import.
        if let Ok(meta) = std::fs::metadata(dest) {
            let mut perms = meta.permissions();
            if perms.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                let _ = std::fs::set_permissions(dest, perms);
            }
        }
        let _ = std::fs::remove_file(dest);
    }

    let mut last_err = None::<std::io::Error>;

    #[cfg(target_os = "windows")]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        for attempt in 0..3 {
            let result = (|| -> std::io::Result<()> {
                let mut src_file = OpenOptions::new().read(true).share_mode(7).open(src)?;
                let mut dest_file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(dest)?;
                std::io::copy(&mut src_file, &mut dest_file)?;
                Ok(())
            })();

            match result {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
                }
            }
        }
    }

    for attempt in 0..3 {
        match std::fs::copy(src, dest) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| std::io::Error::other("Copy failed")))
}
