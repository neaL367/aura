use aura_core::{
    monitor::MonitorId,
    wallpaper::{FitMode, WallpaperId},
};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    AssignWallpaper {
        monitor_id: MonitorId,
        wallpaper_id: WallpaperId,
        fit_mode: Option<FitMode>,
    },
    RemoveAssignment {
        monitor_id: MonitorId,
    },
    DeleteWallpaper {
        wallpaper_id: WallpaperId,
    },
    RefreshLibrary,
    ImportFiles {
        paths: Vec<PathBuf>,
    },
    SetWallpaperLibrary {
        path: PathBuf,
    },
    PauseAll,
    ResumeAll,
}
