use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum DaemonError {
    #[error("platform error: {0}")]
    Platform(#[from] aura_platform_windows::PlatformError),
    #[error("storage error: {0}")]
    Storage(#[from] aura_storage::StorageError),
    #[error("IPC error: {0}")]
    Ipc(#[from] aura_ipc::IpcError),
}

pub(crate) fn run() -> Result<(), DaemonError> {
    // TODO: full daemon implementation in Phase 10
    tracing::info!("daemon run() — stub");
    Ok(())
}
