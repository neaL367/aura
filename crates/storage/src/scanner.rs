use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use aura_core::wallpaper::{MediaKind, WallpaperId, WallpaperMeta, detect_media_kind};
use tracing::{info, warn};

/// Scans configured directories on disk to discover wallpaper media files.
pub struct LibraryScanner;

impl LibraryScanner {
    /// Scan a set of directory paths and return all discovered `WallpaperMeta` items.
    pub fn scan_paths(paths: &[PathBuf]) -> Vec<WallpaperMeta> {
        let mut results = Vec::with_capacity(32);
        let mut visited = HashSet::new();
        for path in paths {
            if path.is_dir() {
                Self::scan_directory(path, &mut results, &mut visited, 0);
            } else if path.is_file() {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                if visited.insert(canonical) {
                    let meta = Self::inspect_file(path);
                    if let Some(meta) = meta {
                        results.push(meta);
                    }
                }
            } else {
                warn!("Scan path {:?} is not a valid file or directory", path);
            }
        }
        info!(
            "Library scan complete — discovered {} wallpapers",
            results.len()
        );
        results
    }

    fn scan_directory(
        dir: &Path,
        results: &mut Vec<WallpaperMeta>,
        visited: &mut HashSet<PathBuf>,
        depth: u32,
    ) {
        if depth > 16 {
            warn!("Maximum directory recursion depth reached at {:?}", dir);
            return;
        }

        let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        if !visited.insert(canonical_dir) {
            return;
        }

        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) => {
                warn!("Failed to read directory {:?}: {}", dir, e);
                return;
            }
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::scan_directory(&path, results, visited, depth + 1);
            } else if path.is_file() {
                let canonical_file = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                if visited.insert(canonical_file) {
                    let meta = Self::inspect_file(&path);
                    if let Some(meta) = meta {
                        results.push(meta);
                    }
                }
            }
        }
    }

    /// Inspect a single file to determine if it is valid media and create `WallpaperMeta`.
    pub fn inspect_file(path: &Path) -> Option<WallpaperMeta> {
        let kind = detect_media_kind(path)?;

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let (width, height) = match kind {
            MediaKind::Image | MediaKind::Gif => image::image_dimensions(path).unwrap_or((0, 0)),
            MediaKind::Video => (0, 0),
        };

        let scanned_at = chrono_iso8601_now();

        Some(WallpaperMeta {
            id: WallpaperId::from_path(path),
            path: path.to_path_buf(),
            kind,
            width,
            height,
            duration_ms: 0,
            file_size,
            scanned_at,
        })
    }
}

pub fn chrono_iso8601_now() -> String {
    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("UNIX-{}", duration.as_secs())
}
