use std::path::PathBuf;
use crate::utils::error::Result;
use crate::config::model::AppConfig;
use super::model::{WallpaperLibraryEntry, derive_id, infer_type_from_extension};
use windows::Win32::System::SystemInformation::GetSystemTime;

/// Adds a wallpaper file path to the central library.
/// Returns the new/existing entry and a boolean indicating whether it was a new addition.
pub fn add_entry(config: &mut AppConfig, path: PathBuf) -> Result<(WallpaperLibraryEntry, bool)> {
    let id = derive_id(&path);
    if let Some(existing) = config.library.iter().find(|e| e.id == id) {
        return Ok((existing.clone(), false));
    }

    let entry = WallpaperLibraryEntry {
        id: id.clone(),
        path: path.clone(),
        wallpaper_type: infer_type_from_extension(&path),
        added_at: get_system_time_iso8601(),
    };
    config.library.push(entry.clone());
    Ok((entry, true))
}

/// Removes a wallpaper entry from the catalog by ID.
/// Also deselects this wallpaper from any monitors that are currently using it.
/// Returns true if the entry existed and was removed.
pub fn remove_entry(config: &mut AppConfig, id: &str) -> bool {
    let mut removed = false;
    if let Some(pos) = config.library.iter().position(|e| e.id == id) {
        config.library.remove(pos);
        removed = true;
        
        // Clear references in monitor assignments
        for monitor in &mut config.monitors {
            if monitor.wallpaper_id.as_deref() == Some(id) {
                monitor.wallpaper_id = None;
            }
        }
    }
    removed
}

/// Generates a UTC ISO-8601 formatted timestamp string using native Win32 APIs.
pub fn get_system_time_iso8601() -> String {
    unsafe {
        let st = GetSystemTime();
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
        )
    }
}

/// Helper structure for UI views to interact with library addition logic modally.
pub struct LibraryScanner<'a> {
    pub config: &'a mut AppConfig,
}

impl<'a> LibraryScanner<'a> {
    pub fn new(config: &'a mut AppConfig) -> Self {
        Self { config }
    }

    pub fn add_entry(&mut self, path: PathBuf) -> Result<(WallpaperLibraryEntry, bool)> {
        add_entry(self.config, path)
    }
}

