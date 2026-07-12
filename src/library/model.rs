use std::path::{Path, PathBuf};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use serde::{Deserialize, Serialize};
use crate::domain::wallpaper::WallpaperType;

/// A single entry in the central wallpaper library catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WallpaperLibraryEntry {
    /// Content-derived identifier of the wallpaper.
    pub id: String,
    /// Absolute path to the local wallpaper file.
    pub path: PathBuf,
    /// Whether the file is an image or video.
    pub wallpaper_type: WallpaperType,
    /// ISO-8601 UTC timestamp of when this wallpaper was added.
    pub added_at: String,
}

/// Derives a stable, content-independent, path-derived unique identifier for a file path.
/// Attempts to canonicalize the path first so that relative/absolute variations refer
/// to the same database ID.
pub fn derive_id(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut hasher = DefaultHasher::new();
    canonical.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Infers whether the file format is an image or a video based on its extension.
pub fn infer_type_from_extension(path: &Path) -> WallpaperType {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "mp4" | "webm" | "mov" => WallpaperType::Video,
        _ => WallpaperType::Image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_derive_id_consistency() {
        let path1 = Path::new("C:\\some\\path\\file.jpg");
        let path2 = Path::new("C:\\some\\path\\file.jpg");
        assert_eq!(derive_id(path1), derive_id(path2));
    }

    #[test]
    fn test_infer_type_from_extension() {
        assert_eq!(infer_type_from_extension(Path::new("video.mp4")), WallpaperType::Video);
        assert_eq!(infer_type_from_extension(Path::new("video.webm")), WallpaperType::Video);
        assert_eq!(infer_type_from_extension(Path::new("video.mov")), WallpaperType::Video);
        assert_eq!(infer_type_from_extension(Path::new("image.png")), WallpaperType::Image);
        assert_eq!(infer_type_from_extension(Path::new("image.jpg")), WallpaperType::Image);
        assert_eq!(infer_type_from_extension(Path::new("image.JPEG")), WallpaperType::Image);
        assert_eq!(infer_type_from_extension(Path::new("unknown")), WallpaperType::Image);
    }
}

