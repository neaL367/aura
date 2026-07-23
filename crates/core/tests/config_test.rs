use aura_core::config::{AppConfig, CONFIG_VERSION, PerformanceConfig, LibraryConfig};
use aura_core::playback::PerformanceProfile;

#[test]
fn performance_config_defaults() {
    let cfg = PerformanceConfig::default();
    assert_eq!(cfg.default_profile, PerformanceProfile::Balanced);
    assert_eq!(cfg.session_locked, PerformanceProfile::Paused);
    assert_eq!(cfg.display_off, PerformanceProfile::Paused);
    assert_eq!(cfg.on_battery, PerformanceProfile::Balanced);
    assert_eq!(cfg.fullscreen_app, PerformanceProfile::Paused);
    assert_eq!(cfg.target_fps, 60);
}

#[test]
fn library_config_defaults() {
    let cfg = LibraryConfig::default();
    assert_eq!(cfg.thumbnail_cache_limit, 512);
}

#[test]
fn app_config_defaults() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.version, CONFIG_VERSION);
    assert!(cfg.assignments.is_empty());
}
