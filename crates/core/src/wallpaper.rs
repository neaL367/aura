use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Unique, stable identifier for a wallpaper file in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WallpaperId(Uuid);

impl WallpaperId {
    /// Create a new random wallpaper ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a stable, deterministic wallpaper ID from a file path (UUID v5).
    pub fn from_path(path: &std::path::Path) -> Self {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy();
        Self(Uuid::new_v5(&Uuid::NAMESPACE_URL, path_str.as_bytes()))
    }
}

impl Default for WallpaperId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WallpaperId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// MediaKind
// ---------------------------------------------------------------------------

/// Media type of a wallpaper file, detected from content (not extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    /// Single-frame static image (PNG, JPEG, BMP, TIFF, WebP, …).
    Image,
    /// Animated GIF with one or more frames.
    Gif,
    /// Video file decoded via Media Foundation.
    Video,
}

impl std::fmt::Display for MediaKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Image => write!(f, "Image"),
            Self::Gif => write!(f, "GIF"),
            Self::Video => write!(f, "Video"),
        }
    }
}

/// Detect media kind from a file path extension (case-insensitive).
///
/// This is the single canonical extension-to-kind mapper for the entire
/// Aura platform. All crates must use this function rather than duplicating
/// the match logic.
pub fn detect_media_kind(path: &Path) -> Option<MediaKind> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("gif") => Some(MediaKind::Gif),
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm") => Some(MediaKind::Video),
        Some("png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif" | "webp") => Some(MediaKind::Image),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// FitMode
// ---------------------------------------------------------------------------

/// Scaling/positioning mode for displaying a wallpaper on a monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    /// Scale image to fill the entire monitor, cropping overflow.
    #[default]
    Fill,
    /// Scale image to fit within monitor, preserving aspect ratio (letterbox/pillarbox).
    Fit,
    /// Stretch image to exact monitor dimensions without preserving aspect ratio.
    Stretch,
    /// Display image at 1:1 original pixel scale, centered on monitor.
    Center,
    /// Repeat image at 1:1 original pixel scale across monitor surface.
    Tile,
    /// Stretch/fill image across the combined virtual desktop bounding box.
    Span,
}

impl std::fmt::Display for FitMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fill => write!(f, "Fill"),
            Self::Fit => write!(f, "Fit"),
            Self::Stretch => write!(f, "Stretch"),
            Self::Center => write!(f, "Center"),
            Self::Tile => write!(f, "Tile"),
            Self::Span => write!(f, "Span"),
        }
    }
}

// ---------------------------------------------------------------------------
// WallpaperMeta
// ---------------------------------------------------------------------------

/// Metadata for a wallpaper file stored in the library.
///
/// Populated at library-scan time; never mutated during rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperMeta {
    pub id: WallpaperId,
    /// Absolute path to the media file.
    pub path: std::path::PathBuf,
    pub kind: MediaKind,
    /// Width in pixels (0 for unknown).
    pub width: u32,
    /// Height in pixels (0 for unknown).
    pub height: u32,
    /// Duration in milliseconds for GIF/Video; 0 for static images.
    pub duration_ms: u64,
    /// File size in bytes at time of scan.
    pub file_size: u64,
    /// ISO-8601 timestamp of the last library scan.
    pub scanned_at: String,
}

// ---------------------------------------------------------------------------
// WallpaperState — lifecycle state machine
// ---------------------------------------------------------------------------

/// Lifecycle state of a wallpaper that is active in the daemon.
///
/// State transitions:
/// ```text
/// Unloaded → Loading → Ready → Rendering ⇌ Paused
///                ↓                ↓
///             (error)          Unloaded
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallpaperState {
    /// Not loaded; no decoder or GPU resources allocated.
    Unloaded,
    /// Decoder is starting; resources being allocated.
    Loading,
    /// Decoder is ready; first frame decoded; GPU texture allocated.
    Ready,
    /// Rendering actively; frames flowing from decoder to GPU.
    Rendering,
    /// Rendering suspended (user-requested pause, battery, session lock, …).
    Paused,
}

impl WallpaperState {
    /// Returns `true` if the wallpaper is consuming GPU resources.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Rendering | Self::Paused)
    }

    /// Validate a state transition and return the new state, or an error.
    pub fn transition(self, to: Self) -> Result<Self, StateTransitionError> {
        use WallpaperState::*;
        let valid = matches!(
            (self, to),
            (Unloaded, Loading)
                | (Loading, Ready)
                | (Loading, Unloaded)   // load failed
                | (Ready, Rendering)
                | (Rendering, Paused)
                | (Paused, Rendering)
                | (Rendering, Unloaded) // wallpaper removed
                | (Paused, Unloaded)
        );
        if valid {
            Ok(to)
        } else {
            Err(StateTransitionError { from: self, to })
        }
    }
}

/// Error returned when an invalid state transition is attempted.
#[derive(Debug, Error)]
#[error("invalid wallpaper state transition {from:?} → {to:?}")]
pub struct StateTransitionError {
    pub from: WallpaperState,
    pub to: WallpaperState,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        use WallpaperState::*;
        assert_eq!(Unloaded.transition(Loading).unwrap(), Loading);
        assert_eq!(Loading.transition(Ready).unwrap(), Ready);
        assert_eq!(Ready.transition(Rendering).unwrap(), Rendering);
        assert_eq!(Rendering.transition(Paused).unwrap(), Paused);
        assert_eq!(Paused.transition(Rendering).unwrap(), Rendering);
        assert_eq!(Rendering.transition(Unloaded).unwrap(), Unloaded);
    }

    #[test]
    fn invalid_transitions() {
        use WallpaperState::*;
        assert!(Unloaded.transition(Rendering).is_err());
        assert!(Ready.transition(Paused).is_err());
        assert!(Paused.transition(Loading).is_err());
    }

    #[test]
    fn wallpaper_id_is_unique() {
        let a = WallpaperId::new();
        let b = WallpaperId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn wallpaper_id_from_path_is_deterministic() {
        let path_a = std::path::Path::new("C:/Wallpapers/bg1.png");
        let path_b = std::path::Path::new("C:/Wallpapers/bg2.png");
        let id_a1 = WallpaperId::from_path(path_a);
        let id_a2 = WallpaperId::from_path(path_a);
        let id_b = WallpaperId::from_path(path_b);
        assert_eq!(id_a1, id_a2);
        assert_ne!(id_a1, id_b);
    }
}
