use serde::{Deserialize, Serialize};

/// Stable identifier for a monitor, typically derived from its Win32 device path or registry key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorId(pub String);

impl std::fmt::Display for MonitorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The physical coordinates and dimensions of a monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
