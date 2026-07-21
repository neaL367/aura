use std::path::{Path, PathBuf};
use std::time::SystemTime;

use aura_core::wallpaper::{MediaKind, WallpaperId, WallpaperMeta};
use tracing::{info, warn};

/// Scans configured directories on disk to discover wallpaper media files.
pub struct LibraryScanner;

impl LibraryScanner {
    /// Scan a set of directory paths and return all discovered `WallpaperMeta` items.
    pub fn scan_paths(paths: &[PathBuf]) -> Vec<WallpaperMeta> {
        let mut results = Vec::new();
        for dir in paths {
            if !dir.is_dir() {
                warn!("Scan path {:?} is not a directory or does not exist", dir);
                continue;
            }
            Self::scan_directory(dir, &mut results);
        }
        info!(
            "Library scan complete — discovered {} wallpapers",
            results.len()
        );
        results
    }

    fn scan_directory(dir: &Path, results: &mut Vec<WallpaperMeta>) {
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
                Self::scan_directory(&path, results);
            } else if path.is_file() {
                let meta = Self::inspect_file(&path);
                if let Some(meta) = meta {
                    results.push(meta);
                }
            }
        }
    }

    /// Inspect a single file to determine if it is valid media and create `WallpaperMeta`.
    pub fn inspect_file(path: &Path) -> Option<WallpaperMeta> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())?;

        let kind = match ext.as_str() {
            "gif" => MediaKind::Gif,
            "png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif" | "webp" => MediaKind::Image,
            "mp4" | "webm" | "mkv" | "avi" => MediaKind::Video,
            _ => return None,
        };

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let (width, height) = match kind {
            MediaKind::Image | MediaKind::Gif => image::image_dimensions(path).unwrap_or((0, 0)),
            MediaKind::Video => (0, 0),
        };

        let scanned_at = chrono_iso8601_now();

        Some(WallpaperMeta {
            id: WallpaperId::new(),
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

fn chrono_iso8601_now() -> String {
    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("UNIX-{}", duration.as_secs())
}
