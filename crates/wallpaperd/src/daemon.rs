use aura_platform_windows::event_pump::{EventPump, HostEvent};
use aura_platform_windows::singleton::ProcessSingleton;
use aura_platform_windows::workerw::WorkerWManager;
use thiserror::Error;

use crate::assignment::AssignmentManager;
use crate::recovery::RecoveryManager;

#[derive(Debug, Error)]
pub(crate) enum DaemonError {
    #[error("platform error: {0}")]
    Platform(#[from] aura_platform_windows::PlatformError),
    #[error("storage error: {0}")]
    Storage(#[from] aura_storage::StorageError),
    #[error("IPC error: {0}")]
    Ipc(#[from] aura_ipc::IpcError),
    #[error("Vulkan error: {0}")]
    Vulkan(#[from] aura_renderer_vulkan::VulkanError),
    #[error("another instance of wallpaperd is already running")]
    AlreadyRunning,
}

pub(crate) fn run() -> Result<(), DaemonError> {
    // 1. Enforce process singleton
    let _singleton = ProcessSingleton::acquire().map_err(|_| DaemonError::AlreadyRunning)?;
    tracing::info!("Process singleton acquired successfully");

    // 2. Initialise WorkerW desktop host manager
    let mut workerw_manager = WorkerWManager::new();
    workerw_manager.find_workerw()?;

    // 3. Spawn platform event pump thread (TaskbarCreated, WM_DISPLAYCHANGE, WM_POWERBROADCAST)
    let event_pump = EventPump::new();
    let receiver = event_pump.receiver.clone();
    let _pump_handle = event_pump.spawn();

    // 4. Initialise assignment manager
    let _assignments = AssignmentManager::new();

    // 5. Initialise Vulkan context
    #[cfg(target_os = "windows")]
    let _vulkan_context = aura_renderer_vulkan::VulkanContext::new()?;

    tracing::info!("wallpaperd orchestrator running — listening for platform events...");

    // Main daemon event dispatch loop
    while let Ok(event) = receiver.recv() {
        match event {
            HostEvent::ExplorerRestarted => {
                tracing::warn!("Orchestrator: Explorer restart signal received");
                if let Err(e) = RecoveryManager::handle_explorer_restart(&mut workerw_manager) {
                    tracing::error!("Failed to recover after Explorer restart: {}", e);
                }
            }
            HostEvent::DisplayChanged => {
                tracing::info!("Orchestrator: Display topology changed");
                if let Err(e) = workerw_manager.find_workerw() {
                    tracing::error!("Failed to re-attach after display change: {}", e);
                }
            }
            HostEvent::PerformanceHint(profile) => {
                tracing::info!("Orchestrator: Performance profile changed to {:?}", profile);
            }
            HostEvent::ShutdownRequested => {
                tracing::info!("Orchestrator: Shutdown signal received. Exiting daemon...");
                break;
            }
        }
    }

    tracing::info!("wallpaperd daemon shutdown complete");
    Ok(())
}
