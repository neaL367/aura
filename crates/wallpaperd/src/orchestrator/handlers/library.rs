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
                library_path.display()
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

    let mut copied = 0;
    let mut errors = Vec::new();
    for src in &paths {
        let file_name = match src.file_name() {
            Some(n) => n,
            None => continue,
        };
        let dest = library_path.join(file_name);
        if dest.exists() {
            continue;
        }
        if let Err(e) = copy_file_robust(src, &dest) {
            errors.push(format!("{}: {}", src.display(), e));
        } else {
            copied += 1;
        }
    }

    if copied == 0 {
        return Response::Error {
            reason: if errors.is_empty() {
                "No files were imported — all selected files already exist in the library.".into()
            } else {
                format!("Failed to copy any files: {}", errors.join("; "))
            },
        };
    }

    if !errors.is_empty() {
        tracing::warn!(
            "ImportFiles: copied {} file(s), {} error(s): {:?}",
            copied,
            errors.len(),
            errors
        );
    } else {
        info!("ImportFiles: copied {} file(s) into library", copied);
    }

    do_refresh(state_lock)
}

pub(super) fn handle_set_wallpaper_library(
    state_lock: &Arc<Mutex<OrchestratorState>>,
    path: PathBuf,
) -> Response {
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
            config.library.library_path = path.clone();
        }) {
            tracing::error!("Failed to update library path: {}", e);
            return Response::Error {
                reason: format!("Failed to update library path: {}", e),
            };
        }

        if let Some(watcher) = &mut state.watcher {
            watcher.replace_paths(std::slice::from_ref(&path));
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
    if dest.exists()
        && let Ok(meta) = std::fs::metadata(dest)
    {
        let mut perms = meta.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(dest, perms);
        }
    }

    let mut last_err = None;
    for attempt in 0..3 {
        match std::fs::copy(src, dest) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        if let Ok(mut src_file) = OpenOptions::new().read(true).share_mode(7).open(src)
            && let Ok(mut dest_file) = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(dest)
            && std::io::copy(&mut src_file, &mut dest_file).is_ok()
        {
            return Ok(());
        }
    }

    Err(last_err.unwrap_or_else(|| std::io::Error::other("Copy failed")))
}
