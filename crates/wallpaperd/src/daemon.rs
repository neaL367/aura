use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use aura_core::wallpaper::MediaKind;
use aura_media::frame_channel;
use aura_media::{ImageDecoder, MediaDecoder};
use aura_platform_windows::PlatformError;
use aura_platform_windows::event_pump::{EventPump, HostEvent};
use aura_platform_windows::host_window::HostWindow;
use aura_platform_windows::monitor_enum::MonitorEnumerator;
use aura_platform_windows::singleton::ProcessSingleton;
use aura_platform_windows::workerw::WorkerWManager;
use aura_renderer_vulkan::VulkanContext;
use aura_renderer_vulkan::VulkanError;
use aura_renderer_vulkan::monitor_renderer::MonitorRenderer;
use crossbeam_channel::RecvTimeoutError;
use thiserror::Error;

use crate::decode_worker::DecodeWorkerHandle;
use crate::orchestrator::Orchestrator;
use crate::perf::PerfMonitor;
use crate::recovery::RecoveryManager;
use crate::render_coordinator::{MonitorContext, RenderCoordinator};

#[derive(Debug, Error)]
pub(crate) enum DaemonError {
    #[error("storage error: {0}")]
    Storage(#[from] aura_storage::StorageError),
    #[error("Vulkan error: {0}")]
    Vulkan(#[from] aura_renderer_vulkan::VulkanError),
    #[error("platform error: {0}")]
    Platform(#[from] aura_platform_windows::PlatformError),
    #[error("media error: {0}")]
    Media(#[from] aura_media::MediaError),
    #[error("another instance of wallpaperd is already running")]
    AlreadyRunning,
    #[error("event pump channel disconnected")]
    EventPumpDisconnected,
    #[error("failed to spawn render thread")]
    ThreadSpawn,
}

#[derive(Debug, Clone, Copy)]
enum AttachState {
    Attached,
    Detached { retry_count: u32 },
}

pub(crate) fn run(wallpaper_path: Option<PathBuf>) -> Result<(), DaemonError> {
    let _singleton = ProcessSingleton::acquire().map_err(|_| DaemonError::AlreadyRunning)?;
    tracing::info!("Process singleton acquired successfully");

    #[cfg(target_os = "windows")]
    let vulkan_context = Arc::new(VulkanContext::new()?);

    #[cfg(target_os = "windows")]
    let monitors = MonitorEnumerator::enumerate()?;
    #[cfg(not(target_os = "windows"))]
    let monitors: Vec<aura_core::monitor::MonitorInfo> = Vec::new();

    let mut workerw_manager = WorkerWManager::new();
    let mut attach_state = attach_or_detach(&mut workerw_manager);

    // Create per-monitor windows + renderers (each renderer runs in its own thread).
    #[cfg(target_os = "windows")]
    let monitor_contexts: Vec<MonitorContext> = {
        let mut contexts = Vec::with_capacity(monitors.len());
        for m in &monitors {
            match create_monitor_context(&vulkan_context, m, wallpaper_path.as_deref()) {
                Ok(ctx) => contexts.push(ctx),
                Err(e) => tracing::error!("Failed to create monitor context: {}", e),
            }
        }
        contexts
    };
    #[cfg(not(target_os = "windows"))]
    let monitor_contexts: Vec<MonitorContext> = Vec::new();

    let mut coordinator = RenderCoordinator::new(monitor_contexts);

    // Attach windows to WorkerW if available.
    if let AttachState::Attached = attach_state {
        coordinator.attach_all(workerw_manager.workerw());
    }

    // Spawn platform event pump thread.
    let event_pump = EventPump::new();
    let receiver = event_pump.receiver.clone();
    let _pump_handle = event_pump.spawn();

    let (ipc_shutdown_tx, ipc_shutdown_rx) = crossbeam_channel::bounded::<()>(1);
    let orchestrator = Orchestrator::new(coordinator.monitor_count(), ipc_shutdown_tx);

    // Spawn async IPC server on a dedicated Tokio thread.
    let orchestrator_ipc = orchestrator.clone();
    let (ipc_server_shutdown_tx, ipc_server_shutdown_rx) = tokio::sync::watch::channel(false);
    let _ipc_thread = std::thread::Builder::new()
        .name("ipc-server".into())
        .spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create Tokio runtime for IPC: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let handler = Box::new(move |req| orchestrator_ipc.handle_request(req));
                let server = aura_ipc::server::IpcServer::new(handler);
                if let Err(e) = server.serve(ipc_server_shutdown_rx).await {
                    tracing::error!("IPC server error: {}", e);
                }
            });
        })
        .map_err(|_| DaemonError::ThreadSpawn)?;

    tracing::info!(
        "wallpaperd orchestrator running — {} monitors, WorkerW: {:?}",
        coordinator.monitor_count(),
        attach_state
    );

    let mut perf_mon = PerfMonitor::new();

    // Main event dispatch loop (no rendering — render threads handle that).
    loop {
        if ipc_shutdown_rx.try_recv().is_ok() {
            tracing::info!("IPC shutdown requested. Exiting daemon...");
            break;
        }

        // Apply paused state to render threads if changed via IPC.
        coordinator.set_paused(orchestrator.is_paused());

        let event = receiver.recv_timeout(Duration::from_millis(500));

        match event {
            Ok(HostEvent::ExplorerRestarted) => {
                tracing::warn!("Explorer restart signal received");
                if RecoveryManager::handle_explorer_restart(&mut workerw_manager) {
                    attach_state = AttachState::Attached;
                    coordinator.attach_all(workerw_manager.workerw());
                } else {
                    attach_state = AttachState::Detached { retry_count: 0 };
                }
            }
            Ok(HostEvent::DisplayChanged) => {
                tracing::info!("Display topology changed");
                if let Ok(_new_monitors) = RecoveryManager::handle_display_change() {
                    attach_state = attach_or_detach(&mut workerw_manager);
                    if let AttachState::Attached = attach_state {
                        coordinator.attach_all(workerw_manager.workerw());
                    }
                }
            }
            Ok(HostEvent::PerformanceHint(profile)) => {
                tracing::info!("Performance profile changed to {:?}", profile);
            }
            Ok(HostEvent::ShutdownRequested) => {
                tracing::info!("Shutdown signal received. Exiting daemon...");
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::error!("Event pump channel disconnected");
                return Err(DaemonError::EventPumpDisconnected);
            }
        }

        // Background retry if detached.
        if let AttachState::Detached { retry_count } = &mut attach_state {
            if workerw_manager.try_find_workerw() {
                coordinator.attach_all(workerw_manager.workerw());
                tracing::info!(
                    "WorkerW re-attached in background retry (after {} attempts)",
                    *retry_count
                );
                attach_state = AttachState::Attached;
            } else {
                *retry_count += 1;
            }
        }

        perf_mon.record_frame();
    }

    // Shutdown: signal IPC server and render threads.
    let _ = ipc_server_shutdown_tx.send(true);
    coordinator.shutdown();

    tracing::info!("wallpaperd daemon shutdown complete");
    Ok(())
}

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

fn detect_media_kind(path: &Path) -> Option<MediaKind> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("gif") => Some(MediaKind::Gif),
        Some("png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif" | "webp") => Some(MediaKind::Image),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn create_monitor_context(
    context: &Arc<VulkanContext>,
    info: &aura_core::monitor::MonitorInfo,
    wallpaper_path: Option<&Path>,
) -> Result<MonitorContext, DaemonError> {
    let host_window = HostWindow::create()?;
    let mut renderer = MonitorRenderer::create_win32(
        context,
        info.id,
        host_window.hwnd(),
        info.width,
        info.height,
    )?;

    // Upload a 1x1 white fallback so the descriptor set is valid.
    let white = [255u8; 4];
    renderer.set_wallpaper_pixels(context, 1, 1, &white)?;
    // Wait for fallback upload to complete.
    unsafe {
        context
            .device
            .wait_for_fences(std::slice::from_ref(&renderer.upload_fence), true, u64::MAX)
            .ok();
        context
            .device
            .reset_fences(std::slice::from_ref(&renderer.upload_fence))
            .ok();
    }

    // Handle wallpaper path: static image or animated GIF.
    let (frame_rx, width, height) = if let Some(path) = wallpaper_path {
        match detect_media_kind(path) {
            Some(MediaKind::Gif) => {
                let (tx, rx) = frame_channel();
                let _handle = DecodeWorkerHandle::spawn_gif_worker(path.to_owned(), tx);
                (Some(rx), info.width, info.height)
            }
            Some(MediaKind::Image) => {
                let mut decoder = ImageDecoder::open(path)?;
                if let Ok(Some(frame)) = decoder.next_frame() {
                    renderer.set_wallpaper_pixels(
                        context,
                        frame.width,
                        frame.height,
                        &frame.data,
                    )?;
                }
                (None, info.width, info.height) // static, no frame channel
            }
            _ => {
                tracing::warn!("Unsupported wallpaper path: {}", path.display());
                (None, info.width, info.height)
            }
        }
    } else {
        (None, info.width, info.height)
    };

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let pause_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = shutdown_flag.clone();
    let pause_clone = pause_flag.clone();
    let context_clone = context.clone();

    let handle = std::thread::Builder::new()
        .name(format!("render-{}", info.id))
        .spawn(move || {
            // Render loop: check for new frames, then draw (FIFO-gated by present mode).
            loop {
                if flag_clone.load(Ordering::Relaxed) {
                    break;
                }

                if pause_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }

                if let Some(ref rx) = frame_rx
                    && let Some(frame) = rx.try_recv()
                    && let Err(e) = renderer.set_wallpaper_pixels(
                        &context_clone,
                        frame.width,
                        frame.height,
                        &frame.data,
                    )
                {
                    tracing::warn!("Texture upload failed: {}", e);
                }

                match renderer.frame(&context_clone, [0.0, 0.0, 0.0, 1.0]) {
                    Ok(_) => {}
                    Err(VulkanError::SwapchainOutOfDate) => {
                        if let Err(e) = renderer.resize(&context_clone, width, height) {
                            tracing::warn!("Swapchain resize failed: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Render frame failed: {}", e);
                    }
                }
            }

            unsafe {
                renderer.destroy(&context_clone);
            }
        })
        .map_err(|_| DaemonError::ThreadSpawn)?;

    Ok(MonitorContext::new(
        host_window,
        handle,
        shutdown_flag,
        pause_flag,
        info.width,
        info.height,
        info.x,
        info.y,
    ))
}
