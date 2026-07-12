use crate::domain::monitor::{MonitorId, MonitorGeometry};
use crate::domain::wallpaper::WallpaperId;

/// Application-wide events emitted by services, background loops, or the Win32 window message pump.
#[derive(Debug, Clone)]
pub enum AppEvent {
    MonitorAdded(MonitorId),
    MonitorRemoved(MonitorId),
    MonitorChanged(MonitorId, MonitorGeometry),
    WallpaperChanged(MonitorId, WallpaperId),
    VideoFinished(MonitorId),
    ExplorerRestarted,
    SettingsUpdated,
    SessionLocked,
    SessionUnlocked,
    MonitorPoweredOff,
    MonitorPoweredOn,
}
