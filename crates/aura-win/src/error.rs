use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Win32 error: {0}")]
    Win32(#[from] windows::core::Error),

    #[error("WorkerW not found — Explorer may not be running")]
    WorkerWNotFound,

    #[error("another instance of wallpaperd is already running")]
    AlreadyRunning,

    #[error("no monitors found")]
    NoMonitors,

    #[error("monitor enumeration failed: {0}")]
    MonitorEnum(String),

    #[error("Media Foundation error: 0x{0:08X}")]
    MediaFoundation(u32),

    #[error("window creation failed")]
    WindowCreation,
}
