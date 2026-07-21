use aura_platform_windows::workerw::WorkerWManager;

/// Handles desktop integration recovery after Explorer crashes or display layout changes.
pub struct RecoveryManager;

impl RecoveryManager {
    /// Attempt to re-establish WorkerW attachment after Explorer restart (never fatal).
    pub fn handle_explorer_restart(manager: &mut WorkerWManager) {
        tracing::info!("RecoveryManager: Re-attaching host windows after Explorer restart...");
        match manager.find_workerw() {
            Ok(workerw) => tracing::info!(
                "RecoveryManager: WorkerW handle re-established: {:?}",
                workerw
            ),
            Err(e) => tracing::error!("RecoveryManager: Failed to re-establish WorkerW: {}", e),
        }
    }
}
