use aura_platform_windows::PlatformError;
use aura_platform_windows::workerw::WorkerWManager;

/// Handles desktop integration recovery after Explorer crashes or display layout changes.
pub struct RecoveryManager;

impl RecoveryManager {
    /// Attempt to re-establish WorkerW attachment after Explorer restart (never fatal).
    pub fn handle_explorer_restart(manager: &mut WorkerWManager) -> bool {
        tracing::info!("RecoveryManager: Re-attaching host windows after Explorer restart...");
        match manager.find_workerw() {
            Ok(workerw) => {
                tracing::info!(
                    "RecoveryManager: WorkerW handle re-established: {:?}",
                    workerw
                );
                true
            }
            Err(e) => {
                tracing::error!("RecoveryManager: Failed to re-establish WorkerW: {}", e);
                false
            }
        }
    }

    /// Re-enumerate monitors on display topology changes.
    pub fn handle_display_change() -> Result<Vec<aura_core::monitor::MonitorInfo>, PlatformError> {
        tracing::info!("RecoveryManager: Re-enumerating monitors after display change...");
        #[cfg(target_os = "windows")]
        {
            aura_platform_windows::monitor_enum::MonitorEnumerator::enumerate()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(Vec::new())
        }
    }
}
