use aura_core::{
    monitor::MonitorId,
    playback::PlaybackCommand,
    wallpaper::{FitMode, WallpaperId},
};
use serde::{Deserialize, Serialize};

/// Named pipe path used by both client and server.
pub const PIPE_NAME: &str = r"\\.\pipe\aura-wallpaperd";

/// Protocol version — increment on breaking changes.
pub const PROTOCOL_VERSION: u16 = 1;

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

/// Commands sent from `wallpaper-ui` to `wallpaperd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Query daemon status.
    GetStatus,
    /// List all wallpapers in the library.
    ListWallpapers,
    /// Assign a wallpaper to a monitor with an optional fit mode.
    AssignWallpaper {
        monitor_id: MonitorId,
        wallpaper_id: WallpaperId,
        #[serde(default)]
        fit_mode: Option<FitMode>,
    },
    /// Set scaling/fit mode for an assigned monitor wallpaper.
    SetFitMode {
        monitor_id: MonitorId,
        fit_mode: FitMode,
    },
    /// Remove the wallpaper from a monitor.
    RemoveAssignment { monitor_id: MonitorId },
    /// Control playback of an animated wallpaper (play/pause) on a monitor.
    SetPlayback {
        monitor_id: MonitorId,
        command: PlaybackCommand,
    },
    /// Pause rendering on all monitors.
    PauseAll,
    /// Resume rendering on all monitors.
    ResumeAll,
    /// Refresh the wallpaper library (rescan configured paths).
    RefreshLibrary,
    /// Add a scan directory path to the wallpaper library.
    AddScanPath { path: std::path::PathBuf },
    /// Remove a scan directory path from the wallpaper library.
    RemoveScanPath { path: std::path::PathBuf },
    /// Fetch the current application configuration.
    GetConfig,
    /// Update the application configuration.
    UpdateConfig {
        config: aura_core::config::AppConfig,
    },
    /// Gracefully shut down the daemon.
    Shutdown,
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// Responses sent from `wallpaperd` to `wallpaper-ui`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Response {
    /// Operation succeeded with no payload.
    Ok,
    /// Operation failed; reason is human-readable.
    Error { reason: String },
    /// Response to `GetStatus`.
    Status(DaemonStatus),
    /// Response to `ListWallpapers`.
    WallpaperList(Vec<WallpaperEntry>),
    /// Response to `GetConfig`.
    Config(aura_core::config::AppConfig),
}

// ---------------------------------------------------------------------------
// Response payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorSummary {
    pub id: MonitorId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub protocol_version: u16,
    pub active_monitors: usize,
    pub assigned_wallpapers: usize,
    pub is_paused: bool,
    pub monitors: Vec<MonitorSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperEntry {
    pub id: WallpaperId,
    pub path: std::path::PathBuf,
    pub kind: aura_core::wallpaper::MediaKind,
    #[serde(default)]
    pub thumbnail_path: Option<std::path::PathBuf>,
}

impl From<&aura_core::wallpaper::WallpaperMeta> for WallpaperEntry {
    fn from(meta: &aura_core::wallpaper::WallpaperMeta) -> Self {
        Self {
            id: meta.id,
            path: meta.path.clone(),
            kind: meta.kind,
            thumbnail_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// IpcMessage — versioned envelope
// ---------------------------------------------------------------------------

/// Framed, versioned IPC message.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcMessage<T> {
    pub version: u16,
    pub payload: T,
}

impl<T> IpcMessage<T> {
    pub fn new(payload: T) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            payload,
        }
    }

    pub fn with_version(payload: T, version: u16) -> Self {
        Self { version, payload }
    }
}
