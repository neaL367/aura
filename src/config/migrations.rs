use serde_json::Value;
use tracing::info;
use windows::Win32::System::SystemInformation::GetSystemTime;

pub const CURRENT_VERSION: u32 = 2;

/// Migrates dynamic JSON configuration from its existing version to the current version.
pub fn migrate(mut json: Value) -> Value {
    let version = json.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    if version < CURRENT_VERSION {
        info!(from = version, to = CURRENT_VERSION, "Migrating config file version");
        
        if version == 1 {
            json = migrate_v1_to_v2(json);
        }

        if let Some(obj) = json.as_object_mut() {
            obj.insert("version".to_string(), serde_json::Value::from(CURRENT_VERSION));
        }
    }

    json
}

fn migrate_v1_to_v2(mut json: Value) -> Value {
    let mut library = Vec::new();
    let mut seen_paths = std::collections::HashMap::new();

    if let Some(monitors) = json.get_mut("monitors").and_then(|v| v.as_array_mut()) {
        for monitor in monitors.iter_mut() {
            if let Some(obj) = monitor.as_object_mut() {
                if let Some(path_val) = obj.remove("wallpaper_path") {
                    if let Some(path_str) = path_val.as_str() {
                        let path = std::path::PathBuf::from(path_str);
                        let id = seen_paths.entry(path_str.to_string()).or_insert_with(|| {
                            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                            let mut hasher = std::collections::hash_map::DefaultHasher::new();
                            use std::hash::Hash;
                            canonical.to_string_lossy().hash(&mut hasher);
                            use std::hash::Hasher;
                            let new_id = format!("{:016x}", hasher.finish());

                            let ext = path.extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let wallpaper_type = match ext.as_str() {
                                "mp4" | "webm" | "mov" => "Video",
                                _ => "Image",
                            };

                            let timestamp = get_system_time_iso8601();

                            library.push(serde_json::json!({
                                "id": new_id,
                                "path": path_str,
                                "wallpaper_type": wallpaper_type,
                                "added_at": timestamp,
                            }));

                            new_id
                        }).clone();

                        obj.insert("wallpaper_id".to_string(), serde_json::Value::String(id));
                    }
                }
                obj.remove("wallpaper_type");
            }
        }
    }

    if let Some(obj) = json.as_object_mut() {
        obj.insert("library".to_string(), serde_json::Value::Array(library));
    }

    json
}

fn get_system_time_iso8601() -> String {
    unsafe {
        let st = GetSystemTime();
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_migration_v1_to_v2() {
        let v1_json = json!({
            "version": 1,
            "monitors": [
                {
                    "monitor_id": "monitor_1",
                    "wallpaper_path": "C:\\Wallpaper\\bg.jpg",
                    "wallpaper_type": "Image",
                    "fit_mode": "Fill",
                    "loop_playback": true,
                    "playback_speed": 1.0,
                    "mute": true,
                    "volume": 0.5
                },
                {
                    "monitor_id": "monitor_2",
                    "wallpaper_path": "C:\\Wallpaper\\bg.jpg",
                    "wallpaper_type": "Image",
                    "fit_mode": "Fit",
                    "loop_playback": true,
                    "playback_speed": 1.0,
                    "mute": true,
                    "volume": 0.5
                }
            ],
            "launch_on_startup": false
        });

        let migrated = migrate(v1_json);
        assert_eq!(migrated["version"], 2);

        let library = migrated["library"].as_array().expect("library should be an array");
        assert_eq!(library.len(), 1, "Duplicate wallpaper paths should be deduped into 1 library entry");

        let entry = &library[0];
        let derived_id = entry["id"].as_str().expect("entry ID should be a string");
        assert_eq!(entry["path"], "C:\\Wallpaper\\bg.jpg");
        assert_eq!(entry["wallpaper_type"], "Image");

        let monitors = migrated["monitors"].as_array().expect("monitors should be an array");
        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[0]["wallpaper_id"], derived_id);
        assert_eq!(monitors[1]["wallpaper_id"], derived_id);
        assert!(monitors[0].get("wallpaper_path").is_none());
        assert!(monitors[0].get("wallpaper_type").is_none());
    }
}


