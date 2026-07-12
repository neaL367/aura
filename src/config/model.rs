use serde::{Deserialize, Serialize};
use crate::domain::fit_mode::FitMode;
use crate::library::model::WallpaperLibraryEntry;

/// Configuration for the entire application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub monitors: Vec<MonitorConfig>,
    pub library: Vec<WallpaperLibraryEntry>,
    pub launch_on_startup: bool,
}

/// Configuration for a specific monitor's wallpaper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Stable monitor device path or hardware ID.
    pub monitor_id: String,
    /// Unique identifier referencing an entry in the central library.
    pub wallpaper_id: Option<String>,
    /// Sizing and layout fit mode.
    pub fit_mode: FitMode,
    /// Whether to loop video playback.
    pub loop_playback: bool,
    /// Video playback speed multiplier (default 1.0).
    pub playback_speed: f32,
    /// Whether to mute audio.
    pub mute: bool,
    /// Audio volume level (0.0 to 1.0).
    pub volume: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            monitors: Vec::new(),
            library: Vec::new(),
            launch_on_startup: false,
        }
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            monitor_id: String::new(),
            wallpaper_id: None,
            fit_mode: FitMode::Fill,
            loop_playback: true,
            playback_speed: 1.0,
            mute: true,
            volume: 0.5,
        }
    }
}

