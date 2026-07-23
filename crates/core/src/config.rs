use serde::{Deserialize, Serialize};

use crate::{monitor::MonitorAssignment, playback::PerformanceProfile};

// ---------------------------------------------------------------------------
// AppConfig — versioned application configuration
// ---------------------------------------------------------------------------

/// Schema version for migration detection.
pub const CONFIG_VERSION: u32 = 1;

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
        }
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
    /// Directories scanned for wallpaper files.
    #[serde(default)]
    pub scan_paths: Vec<std::path::PathBuf>,

    /// Maximum number of thumbnails kept in the on-disk cache.
    #[serde(default = "default_thumb_cache")]
    pub thumbnail_cache_limit: usize,
}

fn default_thumb_cache() -> usize {
    512
}

impl Default for LibraryConfig {
    fn default() -> Self {
        let mut scan_paths = Vec::new();

        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let user_buf = std::path::PathBuf::from(user_profile);
            let pics = user_buf.join("Pictures");
            if pics.is_dir() {
                scan_paths.push(pics.clone());
                let walls = pics.join("Wallpapers");
                if walls.is_dir() {
                    scan_paths.push(walls);
                }
            }
        }

        let win_wall = std::path::PathBuf::from(r"C:\Windows\Web\Wallpaper");
        if win_wall.is_dir() {
            scan_paths.push(win_wall);
        }

        Self {
            scan_paths,
            thumbnail_cache_limit: default_thumb_cache(),
        }
    }
}
