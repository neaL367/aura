use std::time::Duration;

use aura_platform_windows::event_pump::{EventPump, HostEvent};
use aura_platform_windows::host_window::HostWindow;
use aura_platform_windows::monitor_enum::MonitorEnumerator;
use aura_platform_windows::singleton::ProcessSingleton;
use aura_platform_windows::workerw::{attach_to_workerw, WorkerWManager};
use aura_platform_windows::PlatformError;
use aura_renderer_vulkan::monitor_renderer::MonitorRenderer;
use aura_renderer_vulkan::VulkanContext;
use crossbeam_channel::RecvTimeoutError;
use thiserror::Error;

use crate::assignment::AssignmentManager;
use crate::recovery::RecoveryManager;

#[derive(Debug, Error)]
pub(crate) enum DaemonError {
    #[error("storage error: {0}")]
    Storage(#[from] aura_storage::StorageError),
    #[error("Vulkan error: {0}")]
    Vulkan(#[from] aura_renderer_vulkan::VulkanError),
    #[error("platform error: {0}")]
    Platform(#[from] aura_platform_windows::PlatformError),
    #[error("another instance of wallpaperd is already running")]
    AlreadyRunning,
    #[error("event pump channel disconnected")]
    EventPumpDisconnected,
}

/// Current WorkerW attachment state for the daemon event loop.
#[derive(Debug, Clone, Copy)]
enum AttachState {
    Attached,
    Detached { retry_count: u32 },
}

/// Per-monitor state owned by the daemon.
struct MonitorContext {
    host_window: HostWindow,
    renderer: MonitorRenderer,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
}

impl MonitorContext {
    /// Attach the host window to the given WorkerW and size to monitor bounds.
    fn attach_to_workerw(&mut self, workerw: windows::Win32::Foundation::HWND) {
        if let Err(e) = attach_to_workerw(self.host_window.hwnd(), workerw) {
            tracing::error!("Failed to attach window to WorkerW: {}", e);
            return;
        }
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                self.host_window.hwnd(),
                self.x,
                self.y,
                self.width as i32,
                self.height as i32,
                true,
            );
        }
    }

    /// Clean up GPU resources.
    unsafe fn destroy(&mut self, context: &mut VulkanContext) {
        unsafe { self.renderer.destroy(context) };
    }
}

pub(crate) fn run() -> Result<(), DaemonError> {
    // 1. Enforce process singleton (fatal if already running).
    let _singleton = ProcessSingleton::acquire().map_err(|_| DaemonError::AlreadyRunning)?;
    tracing::info!("Process singleton acquired successfully");

    // 2. Initialise Vulkan context (fatal if Vulkan not available).
    #[cfg(target_os = "windows")]
    let mut vulkan_context = VulkanContext::new()?;

    // 3. Enumerate monitors.
    #[cfg(target_os = "windows")]
    let monitors = MonitorEnumerator::enumerate()?;
    #[cfg(not(target_os = "windows"))]
    let monitors: Vec<aura_core::monitor::MonitorInfo> = Vec::new();

    // 4. Initialise WorkerW desktop host manager.
    let mut workerw_manager = WorkerWManager::new();
    let mut attach_state = attach_or_detach(&mut workerw_manager);

    // 5. Create host window + renderer per monitor.
    #[cfg(target_os = "windows")]
    let mut monitor_contexts: Vec<MonitorContext> = {
        let mut contexts = Vec::with_capacity(monitors.len());
        for m in &monitors {
            match create_monitor_context(&mut vulkan_context, m) {
                Ok(mut ctx) => {
                    // Upload a 1×1 white fallback so the descriptor set is valid.
                    let white = [255u8; 4];
                    let _ = ctx.renderer.set_wallpaper_pixels(
                        &mut vulkan_context, 1, 1, &white,
                    );
                    contexts.push(ctx);
                }
                Err(e) => tracing::error!("Failed to create monitor context: {}", e),
            }
        }
        // Attach any created windows if WorkerW is available.
        if let AttachState::Attached = attach_state {
            let workerw = workerw_manager.workerw();
            for ctx in &mut contexts {
                ctx.attach_to_workerw(workerw);
            }
        }
        // Draw first frame on all renderers.
        for ctx in &mut contexts {
            let _ = ctx.renderer.frame(&vulkan_context, [0.0, 0.0, 0.0, 1.0]);
        }
        contexts
    };
    #[cfg(not(target_os = "windows"))]
    let mut monitor_contexts: Vec<MonitorContext> = Vec::new();

    // 6. Spawn platform event pump thread.
    let event_pump = EventPump::new();
    let receiver = event_pump.receiver.clone();
    let _pump_handle = event_pump.spawn();

    // 7. Initialise assignment manager.
    let _assignments = AssignmentManager::new();

    tracing::info!(
        "wallpaperd orchestrator running — {} monitors, WorkerW: {:?}",
        monitor_contexts.len(),
        attach_state
    );

    // Main event dispatch loop with health-check tick.
    loop {
        let event = receiver.recv_timeout(Duration::from_secs(5));

        match event {
            Ok(HostEvent::ExplorerRestarted) => {
                tracing::warn!("Explorer restart signal received");
                RecoveryManager::handle_explorer_restart(&mut workerw_manager);
                attach_state = AttachState::Detached { retry_count: 0 };
            }
            Ok(HostEvent::DisplayChanged) => {
                tracing::info!("Display topology changed");
                attach_state = attach_or_detach(&mut workerw_manager);
            }
            Ok(HostEvent::PerformanceHint(profile)) => {
                tracing::info!("Performance profile changed to {:?}", profile);
            }
            Ok(HostEvent::ShutdownRequested) => {
                tracing::info!("Shutdown signal received. Exiting daemon...");
                break;
            }
            Err(RecvTimeoutError::Timeout) => {} // health-check tick
            Err(RecvTimeoutError::Disconnected) => {
                tracing::error!("Event pump channel disconnected");
                return Err(DaemonError::EventPumpDisconnected);
            }
        }

        // Background retry if detached.
        if let AttachState::Detached { retry_count } = &mut attach_state {
            if workerw_manager.try_find_workerw() {
                let workerw = workerw_manager.workerw();
                for ctx in &mut monitor_contexts {
                    ctx.attach_to_workerw(workerw);
                }
                tracing::info!(
                    "WorkerW re-attached in background retry (after {} attempts)",
                    *retry_count
                );
                attach_state = AttachState::Attached;
            } else {
                *retry_count += 1;
            }
        }

        // Draw one frame on each renderer (FIFO-paced).
        #[cfg(target_os = "windows")]
        for ctx in &mut monitor_contexts {
            if let Err(e) = ctx.renderer.frame(&vulkan_context, [0.0, 0.0, 0.0, 1.0]) {
                tracing::warn!("Render frame failed: {}", e);
            }
        }
    }

    // Cleanup: destroy GPU resources before VulkanContext drops.
    #[cfg(target_os = "windows")]
    unsafe {
        for ctx in &mut monitor_contexts {
            ctx.destroy(&mut vulkan_context);
        }
    }

    tracing::info!("wallpaperd daemon shutdown complete");
    Ok(())
}

/// Attempt full WorkerW attachment. Returns the resulting state (never fatal).
fn attach_or_detach(manager: &mut WorkerWManager) -> AttachState {
    match manager.find_workerw() {
        Ok(_) => {
            tracing::info!("WorkerW attachment established");
            AttachState::Attached
        }
        Err(PlatformError::WorkerWNotFound) => {
            tracing::warn!("WorkerW not found — entering detached state");
            AttachState::Detached { retry_count: 0 }
        }
        Err(e) => {
            tracing::error!("WorkerW attachment failed: {}", e);
            AttachState::Detached { retry_count: 0 }
        }
    }
}

/// Create a `HostWindow` + `MonitorRenderer` for one monitor.
#[cfg(target_os = "windows")]
fn create_monitor_context(
    context: &mut VulkanContext,
    info: &aura_core::monitor::MonitorInfo,
) -> Result<MonitorContext, DaemonError> {
    let host_window = HostWindow::create()?;
    let renderer = MonitorRenderer::create_win32(
        context,
        info.id,
        host_window.hwnd(),
        info.width,
        info.height,
    )?;
    Ok(MonitorContext {
        host_window,
        renderer,
        width: info.width,
        height: info.height,
        x: info.x,
        y: info.y,
    })
}
