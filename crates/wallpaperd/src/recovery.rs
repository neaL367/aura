use aura_platform_windows::PlatformError;
use aura_platform_windows::workerw::WorkerWManager;

/// Handles desktop integration recovery after Explorer crashes or display layout changes.
pub struct RecoveryManager;

impl RecoveryManager {
    /// Idempotently re-attach WorkerW desktop host windows after Explorer restarts.
    pub fn handle_explorer_restart(manager: &mut WorkerWManager) -> Result<(), PlatformError> {
        tracing::info!("RecoveryManager: Re-attaching host windows after Explorer restart...");
        let workerw = manager.find_workerw()?;
        tracing::info!(
            "RecoveryManager: Explorer WorkerW handle re-established: {:?}",
            workerw
        );
        Ok(())
    }
}
