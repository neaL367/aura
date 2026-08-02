use serde::{Deserialize, Serialize};

use crate::{monitor::MonitorAssignment, playback::PerformanceProfile};

// ---------------------------------------------------------------------------
// AppConfig — versioned application configuration
// ---------------------------------------------------------------------------

/// Schema version for migration detection.
pub const CONFIG_VERSION: u32 = 2;

/// Top-level application configuration serialised to TOML.
///
/// Migration: the `version` field is checked at load time; missing fields
/// receive defaults; unknown fields are silently ignored (forward compat).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Schema version.  Must equal `CONFIG_VERSION` after migration.
    #[serde(default = "default_version")]
    pub version: u32,

    /// Current monitor assignments.
    #[serde(default)]
    pub assignments: Vec<MonitorAssignment>,

    /// Performance preferences.
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Wallpaper library settings.
    #[serde(default)]
    pub library: LibraryConfig,

    /// Appearance settings.
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            assignments: Vec::new(),
            performance: PerformanceConfig::default(),
            library: LibraryConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

impl AppConfig {
    /// Validate the config, clamping out-of-range values and returning a list of
    /// corrections applied. Errors on unrecoverable invalid state.
    pub fn validate(&mut self) -> Vec<String> {
        let mut corrections = Vec::new();
        if self.performance.target_fps < 1 {
            corrections.push(format!(
                "target_fps {} out of range (1–120), clamped to 1",
                self.performance.target_fps
            ));
            self.performance.target_fps = 1;
        }
        if self.performance.target_fps > 120 {
            corrections.push(format!(
                "target_fps {} out of range (1–120), clamped to 120",
                self.performance.target_fps
            ));
            self.performance.target_fps = 120;
        }
        corrections
    }
}

// ---------------------------------------------------------------------------
// PerformanceConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Profile applied during normal operation.
    #[serde(default)]
    pub default_profile: PerformanceProfile,

    /// Profile applied when the session is locked.
    #[serde(default = "paused_profile")]
    pub session_locked: PerformanceProfile,

    /// Profile applied when the display is off.
    #[serde(default = "paused_profile")]
    pub display_off: PerformanceProfile,

    /// Profile applied when running on battery.
    #[serde(default)]
    pub on_battery: PerformanceProfile,

    /// Profile applied when a full-screen application is detected.
    #[serde(default = "paused_profile")]
    pub fullscreen_app: PerformanceProfile,

    /// Target frames per second for animated wallpapers (1–120).
    #[serde(default = "default_fps")]
    pub target_fps: u8,
}

fn paused_profile() -> PerformanceProfile {
    PerformanceProfile::Paused
}

fn default_fps() -> u8 {
    60
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            default_profile: PerformanceProfile::default(),
            session_locked: paused_profile(),
            display_off: paused_profile(),
            on_battery: PerformanceProfile::Balanced,
            fullscreen_app: paused_profile(),
            target_fps: default_fps(),
        }
    }
}

// ---------------------------------------------------------------------------
// LibraryConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryConfig {
    /// Single root directory containing wallpaper files (scanned recursively).
    #[serde(default = "default_library_path")]
    pub library_path: std::path::PathBuf,

    /// Transitional: v1 configs serialised `scan_paths`. Kept for deserialization
    /// during v1→v2 migration. Removed in a future schema version.
    #[serde(default, skip_serializing)]
    pub scan_paths: Vec<std::path::PathBuf>,

    /// Maximum number of thumbnails kept in the on-disk cache.
    #[serde(default = "default_thumb_cache")]
    pub thumbnail_cache_limit: usize,
}

fn default_thumb_cache() -> usize {
    512
}

// ---------------------------------------------------------------------------
// AppearanceConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Use dark theme instead of light.
    #[serde(default)]
    pub dark_mode: bool,

    /// Register the Aura binary to launch at Windows login.
    #[serde(default)]
    pub auto_start: bool,

    /// Slideshow interval in seconds. 0 = disabled.
    #[serde(default)]
    pub slideshow_interval_secs: u64,
}

/// Default library path under `%USERPROFILE%/Pictures/Aura Wallpapers`.
/// Creates the directory if it doesn't exist, so import always works without
/// falling back to a system-owned path like `C:\Windows\Web\Wallpaper`.
pub fn default_library_path() -> std::path::PathBuf {
    let fallback = match std::env::var("APPDATA") {
        Ok(appdata) => std::path::PathBuf::from(appdata)
            .join("aura")
            .join("wallpapers"),
        Err(_) => std::path::PathBuf::from(r"C:\Users\Public\Pictures\Aura Wallpapers"),
    };

    if let Ok(p) = std::env::var("USERPROFILE") {
        let candidate = std::path::PathBuf::from(p)
            .join("Pictures")
            .join("Aura Wallpapers");

        if candidate.is_dir() || std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }

    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            library_path: default_library_path(),
            scan_paths: Vec::new(),
            thumbnail_cache_limit: default_thumb_cache(),
        }
    }
}
