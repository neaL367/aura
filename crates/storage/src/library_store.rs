use std::path::PathBuf;

use aura_core::wallpaper::WallpaperMeta;
use tracing::warn;

use crate::error::StorageError;

/// Simple JSON-based wallpaper library metadata cache.
///
/// Stores a flat list of `WallpaperMeta` entries.  Bounded by
/// filesystem capacity; intended for library sizes up to ~100k entries.
pub struct LibraryStore {
    path: PathBuf,
}

impl LibraryStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Load all cached wallpaper metadata.
    pub fn load(&self) -> Result<Vec<WallpaperMeta>, StorageError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let entries: Vec<WallpaperMeta> = serde_json::from_str(&raw).unwrap_or_else(|e| {
            warn!("Library cache corrupt ({}); starting fresh", e);
            Vec::new()
        });
        Ok(entries)
    }

    /// Persist the full list of wallpaper metadata (atomic write).
    pub fn save(&self, entries: &[WallpaperMeta]) -> Result<(), StorageError> {
        let serialised = serde_json::to_string_pretty(entries)?;
        crate::atomic::atomic_save_file(&self.path, &serialised)
    }
}
