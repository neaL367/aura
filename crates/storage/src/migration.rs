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

    // Future migrations go here:
    // if config.version == 1 { ... config.version = 2; }

    if config.version != CONFIG_VERSION {
        return Err(StorageError::Migration(format!(
            "unknown schema version {} after migration",
            config.version
        )));
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_v0_to_current() {
        let cfg = AppConfig {
            version: 0,
            ..Default::default()
        };
        let migrated = migrate(cfg).expect("migration must succeed");
        assert_eq!(migrated.version, CONFIG_VERSION);
    }

    #[test]
    fn migrate_current_version_is_noop() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.version, CONFIG_VERSION);
        // Migrating a current-version config should still produce the same version.
        let migrated = migrate(cfg).expect("migration must succeed");
        assert_eq!(migrated.version, CONFIG_VERSION);
    }
}
