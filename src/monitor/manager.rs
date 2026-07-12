use std::collections::HashMap;
use crate::domain::events::AppEvent;
use crate::domain::monitor::{MonitorId, MonitorGeometry};
use crate::monitor::enumeration::{enumerate_monitors, MonitorInfo};
use crate::utils::error::Result;

pub struct MonitorManager {
    /// Last known snapshot, keyed by durable `MonitorId`, not `HMONITOR`.
    known: HashMap<MonitorId, MonitorInfo>,
}

impl MonitorManager {
    pub fn new() -> Self {
        Self {
            known: HashMap::new(),
        }
    }

    /// Call once at startup to establish the initial snapshot without
    /// emitting spurious `MonitorAdded` events for the whole desktop.
    pub fn initialize(&mut self) -> Result<Vec<MonitorId>> {
        let monitors = enumerate_monitors()?;
        let ids = monitors
            .iter()
            .map(|m| MonitorId(m.device_id.clone()))
            .collect();
        self.known = monitors
            .into_iter()
            .map(|m| (MonitorId(m.device_id.clone()), m))
            .collect();
        Ok(ids)
    }

    /// Call on every `WM_DISPLAYCHANGE` / `WM_DEVICECHANGE` / `WM_DPICHANGED`.
    /// Re-enumerates from scratch, diffs against the last snapshot by `device_id`,
    /// and returns the events that should be processed/broadcast.
    pub fn sync(&mut self) -> Result<Vec<AppEvent>> {
        let fresh = enumerate_monitors()?;
        let fresh_map: HashMap<MonitorId, MonitorInfo> = fresh
            .into_iter()
            .map(|m| (MonitorId(m.device_id.clone()), m))
            .collect();

        let mut events = Vec::new();

        // Removed: known but not in fresh set.
        for id in self.known.keys() {
            if !fresh_map.contains_key(id) {
                events.push(AppEvent::MonitorRemoved(id.clone()));
            }
        }

        // Added or changed.
        for (id, fresh_info) in &fresh_map {
            match self.known.get(id) {
                None => {
                    events.push(AppEvent::MonitorAdded(id.clone()));
                }
                Some(old_info) => {
                    if geometry_changed(old_info, fresh_info) {
                        events.push(AppEvent::MonitorChanged(
                            id.clone(),
                            to_domain_geometry(fresh_info),
                        ));
                    }
                }
            }
        }

        self.known = fresh_map;
        Ok(events)
    }

    pub fn current(&self, id: &MonitorId) -> Option<&MonitorInfo> {
        self.known.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = (&MonitorId, &MonitorInfo)> {
        self.known.iter()
    }
}

fn geometry_changed(old: &MonitorInfo, new: &MonitorInfo) -> bool {
    old.rect.left != new.rect.left
        || old.rect.top != new.rect.top
        || old.rect.right != new.rect.right
        || old.rect.bottom != new.rect.bottom
        || old.dpi != new.dpi
        || old.is_primary != new.is_primary
}

fn to_domain_geometry(info: &MonitorInfo) -> MonitorGeometry {
    MonitorGeometry {
        x: info.rect.left,
        y: info.rect.top,
        width: info.rect.right - info.rect.left,
        height: info.rect.bottom - info.rect.top,
    }
}
