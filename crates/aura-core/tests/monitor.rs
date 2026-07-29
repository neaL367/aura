use aura_core::monitor::MonitorId;

#[test]
fn monitor_id_is_stable() {
    let path = r"\\.\DISPLAY1\Monitor0";
    let a = MonitorId::from_device_path(path);
    let b = MonitorId::from_device_path(path);
    assert_eq!(a, b, "Same path must produce same MonitorId");
}

#[test]
fn different_paths_produce_different_ids() {
    let a = MonitorId::from_device_path(r"\\.\DISPLAY1\Monitor0");
    let b = MonitorId::from_device_path(r"\\.\DISPLAY2\Monitor0");
    assert_ne!(a, b);
}
