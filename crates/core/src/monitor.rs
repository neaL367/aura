use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::wallpaper::{FitMode, WallpaperId};

// ---------------------------------------------------------------------------
// MonitorId
// ---------------------------------------------------------------------------

/// Stable monitor identity derived from the device path, not enumeration order.
///
/// Windows assigns device paths (e.g. `\\.\DISPLAY1\Monitor0`) to physical
/// monitors.  These persist across display configuration changes, unlike
/// monitor enumeration indices.
///
/// The ID is a UUID derived deterministically from the device path hash so it
/// is stable across process restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorId(Uuid);

impl MonitorId {
    /// Create a `MonitorId` from a device path string (e.g. from `SetupAPI`).
    ///
    /// The path is hashed to produce a stable UUID v5 (namespace = DNS).
    pub fn from_device_path(path: &str) -> Self {
        // UUID v5 uses SHA-1 to hash a name into a namespace.
        Self(Uuid::new_v5(&Uuid::NAMESPACE_DNS, path.as_bytes()))
    }

    /// Create a `MonitorId` directly from a raw UUID (used in tests/storage).
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Return the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for MonitorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "monitor:{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// MonitorInfo
// ---------------------------------------------------------------------------

/// Information about a connected monitor at a point in time.
///
/// Used to snapshot the monitor state when enumeration occurs.
/// Not guaranteed to remain valid after the next display change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: MonitorId,
    /// Human-readable device name (e.g. `\\.\DISPLAY1`).
    pub device_name: String,
    /// Device path used to derive the stable ID.
    pub device_path: String,
    /// X position of the monitor origin in virtual screen coordinates.
    pub x: i32,
    /// Y position of the monitor origin in virtual screen coordinates.
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// DPI scaling factor relative to 96 DPI (96 = 1.0×, 192 = 2.0×).
    pub dpi: u32,
    /// True if this is the primary monitor.
    pub is_primary: bool,
}

// ---------------------------------------------------------------------------
// MonitorAssignment
// ---------------------------------------------------------------------------

/// A wallpaper assigned to a specific monitor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorAssignment {
    pub monitor_id: MonitorId,
    pub wallpaper_id: WallpaperId,
    #[serde(default)]
    pub fit_mode: FitMode,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_id_is_stable() {
        let path = r"\\.\DISPLAY1\Monitor0";
        let a = MonitorId::from_device_path(path);
        let b = MonitorId::from_device_path(path);
        assert_eq!(a, b, "Same path must produce same MonitorId");
    }

    #[test]
    fn different_paths_produce_different_ids() {
        let a = MonitorId::from_device_path(r"\\.\DISPLAY1\Monitor0");
        let b = MonitorId::from_device_path(r"\\.\DISPLAY2\Monitor0");
        assert_ne!(a, b);
    }
}
