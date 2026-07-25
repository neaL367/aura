use aura_core::config::AppConfig;
use aura_core::monitor::{MonitorAssignment, MonitorId};
use aura_core::wallpaper::{FitMode, WallpaperId};

#[test]
fn monitor_assignment_toml_roundtrip() {
    let assignment = MonitorAssignment {
        monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
        wallpaper_id: WallpaperId::new(),
        fit_mode: FitMode::Center,
    };

    let mut config = AppConfig::default();
    config.assignments.push(assignment.clone());

    let toml_str = toml::to_string(&config).unwrap();
    let back: AppConfig = toml::from_str(&toml_str).unwrap();

    assert_eq!(back.assignments.len(), 1);
    assert_eq!(back.assignments[0].monitor_id, assignment.monitor_id);
    assert_eq!(back.assignments[0].wallpaper_id, assignment.wallpaper_id);
    assert_eq!(back.assignments[0].fit_mode, assignment.fit_mode);
}

#[test]
fn monitor_assignment_default_fit_mode_is_fill() {
    let toml_str = r#"
version = 1

[[assignments]]
monitor_id = "00000000-0000-0000-0000-000000000001"
wallpaper_id = "00000000-0000-0000-0000-000000000002"
"#;

    let config: AppConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.assignments.len(), 1);
    assert_eq!(config.assignments[0].fit_mode, FitMode::Fill);
}

#[test]
fn monitor_assignment_custom_fit_mode_in_toml() {
    let toml_str = r#"
version = 1

[[assignments]]
monitor_id = "00000000-0000-0000-0000-000000000001"
wallpaper_id = "00000000-0000-0000-0000-000000000002"
fit_mode = "center"
"#;

    let config: AppConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.assignments[0].fit_mode, FitMode::Center);
}

#[test]
fn monitor_assignment_fit_mode_stretch() {
    let toml_str = r#"
version = 1

[[assignments]]
monitor_id = "00000000-0000-0000-0000-000000000001"
wallpaper_id = "00000000-0000-0000-0000-000000000002"
fit_mode = "stretch"
"#;

    let config: AppConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.assignments[0].fit_mode, FitMode::Stretch);
}
