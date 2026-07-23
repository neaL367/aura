use std::collections::HashMap;

use aura_core::monitor::MonitorId;
use aura_core::wallpaper::WallpaperId;

/// Manages per-monitor wallpaper assignments.
#[derive(Debug, Default)]
pub struct AssignmentManager {
    assignments: HashMap<MonitorId, WallpaperId>,
}

impl AssignmentManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn assign(&mut self, monitor_id: MonitorId, wallpaper_id: WallpaperId) {
        self.assignments.insert(monitor_id, wallpaper_id);
    }

    pub fn remove(&mut self, monitor_id: &MonitorId) -> Option<WallpaperId> {
        self.assignments.remove(monitor_id)
    }

    pub fn get(&self, monitor_id: &MonitorId) -> Option<&WallpaperId> {
        self.assignments.get(monitor_id)
    }

    pub fn all(&self) -> &HashMap<MonitorId, WallpaperId> {
        &self.assignments
    }
}
