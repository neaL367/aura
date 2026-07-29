use aura_core::config::{AppConfig, CONFIG_VERSION};

use crate::error::StorageError;

/// Apply all pending schema migrations to `config` in sequence.
///
/// Each migration step is numbered and idempotent.
pub fn migrate(mut config: AppConfig) -> Result<AppConfig, StorageError> {
    // v0 → v1: initial version; no structural changes needed
    if config.version == 0 {
        config.version = 1;
    }

    // v1 → v2: replace scan_paths Vec with single library_path
    if config.version == 1 {
        let new_library = if config.library.scan_paths.is_empty() {
            aura_core::config::default_library_path()
        } else {
            config.library.scan_paths[0].clone()
        };
        config.library.library_path = new_library;
        config.version = 2;
    }

    if config.version != CONFIG_VERSION {
        return Err(StorageError::Migration(format!(
            "unknown schema version {} after migration",
            config.version
        )));
    }

    Ok(config)
}
