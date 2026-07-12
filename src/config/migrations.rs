use serde_json::Value;
use tracing::info;

pub const CURRENT_VERSION: u32 = 1;

/// Migrates dynamic JSON configuration from its existing version to the current version.
pub fn migrate(mut json: Value) -> Value {
    let version = json.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    if version < CURRENT_VERSION {
        info!(from = version, to = CURRENT_VERSION, "Migrating config file version");
        
        // Example structure for future migrations:
        // if version == 1 {
        //     json = migrate_v1_to_v2(json);
        //     version = 2;
        // }
        
        if let Some(obj) = json.as_object_mut() {
            obj.insert("version".to_string(), serde_json::Value::from(CURRENT_VERSION));
        }
    }

    json
}
