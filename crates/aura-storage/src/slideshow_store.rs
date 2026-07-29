use std::path::PathBuf;

use aura_core::slideshow_state::{SLIDESHOW_STATE_VERSION, SlideshowState};
use tracing::warn;

use crate::{atomic_file, error::StorageError};

/// Reads and writes `SlideshowState` to a JSON file.
///
/// Best-effort: corrupt/missing files reset to empty rather than
/// preventing the daemon from starting.
pub struct SlideshowStore {
    path: PathBuf,
}

impl SlideshowStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\AuraData"));
        base.join("Aura").join("slideshow_state.json")
    }

    pub fn load(&self) -> Result<Option<SlideshowState>, StorageError> {
        if !self.path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&self.path)?;
        let state: SlideshowState = match serde_json::from_str(&raw) {
            Ok(s) => s,
            Err(e) => {
                warn!(path = ?self.path, error = %e, "Slideshow state corrupt — resetting");
                let _ = std::fs::remove_file(&self.path);
                return Ok(None);
            }
        };

        if state.version != SLIDESHOW_STATE_VERSION {
            warn!(
                from = state.version,
                expected = SLIDESHOW_STATE_VERSION,
                "Slideshow state version mismatch — resetting"
            );
            let _ = std::fs::remove_file(&self.path);
            return Ok(None);
        }

        Ok(Some(state))
    }

    pub fn save(&self, state: &SlideshowState) -> Result<(), StorageError> {
        let serialised = serde_json::to_string_pretty(state)?;
        atomic_file::atomic_save_file(&self.path, &serialised)
    }

    pub fn reset(&self) -> Result<(), StorageError> {
        let _ = std::fs::remove_file(&self.path);
        let fresh = SlideshowState::new();
        self.save(&fresh)
    }
}
