use wallpaperd::daemon::DaemonError;

#[test]
fn display_storage_error() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = DaemonError::Storage(aura_storage::StorageError::Io(io));
    let msg = err.to_string();
    assert!(msg.contains("storage error"), "got: {msg}");
}

#[test]
fn display_vulkan_error() {
    let err = DaemonError::ThreadSpawn;
    let msg = err.to_string();
    assert!(msg.contains("failed to spawn"), "got: {msg}");
}

#[test]
fn display_already_running() {
    let err = DaemonError::AlreadyRunning;
    assert_eq!(err.to_string(), "another instance of wallpaperd is already running");
}

#[test]
fn display_event_pump_disconnected() {
    let err = DaemonError::EventPumpDisconnected;
    assert_eq!(err.to_string(), "event pump channel disconnected");
}

#[test]
fn debug_all_variants() {
    let io = std::io::Error::new(std::io::ErrorKind::Other, "io");
    let variants: Vec<DaemonError> = vec![
        DaemonError::Storage(aura_storage::StorageError::Io(io)),
        DaemonError::ThreadSpawn,
        DaemonError::AlreadyRunning,
        DaemonError::EventPumpDisconnected,
    ];
    for v in &variants {
        let _debug = format!("{:?}", v);
    }
}
