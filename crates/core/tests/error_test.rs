use aura_core::CoreError;

#[test]
fn display_state_transition() {
    let err = CoreError::Config("bad config".into());
    let msg = err.to_string();
    assert!(msg.contains("configuration error"), "got: {msg}");
    assert!(msg.contains("bad config"), "got: {msg}");
}

#[test]
fn display_config_error() {
    let err = CoreError::Config("test".into());
    let msg = err.to_string();
    assert!(msg.contains("configuration error: test"), "got: {msg}");
}

#[test]
fn display_io_error() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err: CoreError = io.into();
    let msg = err.to_string();
    assert!(msg.contains("I/O error"), "got: {msg}");
}

#[test]
fn from_io_error() {
    let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err: CoreError = io.into();
    assert!(matches!(err, CoreError::Io(_)));
}
