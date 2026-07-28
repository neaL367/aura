use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::monitor::MonitorId;
use crate::wallpaper::WallpaperId;

pub const SLIDESHOW_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideshowState {
    pub version: u32,
    pub queue: Vec<WallpaperId>,
    pub index: usize,
    pub last_cycle: u64,
    pub last_wallpapers: HashMap<MonitorId, WallpaperId>,
}

impl SlideshowState {
    pub fn new() -> Self {
        Self {
            version: SLIDESHOW_STATE_VERSION,
            queue: Vec::new(),
            index: 0,
            last_cycle: 0,
            last_wallpapers: HashMap::new(),
        }
    }

    pub fn reset() -> Self {
        let mut s = Self::new();
        s.queue.shrink_to_fit();
        s
    }
}

impl Default for SlideshowState {
    fn default() -> Self {
        Self::new()
    }
}
