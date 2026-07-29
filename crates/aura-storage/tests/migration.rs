use aura_core::config::{AppConfig, CONFIG_VERSION};
use aura_storage::migration::migrate;

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
    let migrated = migrate(cfg).expect("migration must succeed");
    assert_eq!(migrated.version, CONFIG_VERSION);
}

#[test]
fn migrate_future_version_errors() {
    let cfg = AppConfig {
        version: 99,
        ..Default::default()
    };
    assert!(migrate(cfg).is_err());
}
