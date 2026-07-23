use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aura_platform_windows::PlatformError;
use aura_platform_windows::event_pump::{EventPump, HostEvent};
use aura_platform_windows::monitor_enum::MonitorEnumerator;
use aura_platform_windows::singleton::ProcessSingleton;
use aura_platform_windows::workerw::WorkerWManager;
use aura_renderer_vulkan::VulkanContext;
use crossbeam_channel::RecvTimeoutError;
use thiserror::Error;

static CTRLC_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
type ConsoleHandlerRoutine = Option<unsafe extern "system" fn(u32) -> i32>;

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn SetConsoleCtrlHandler(handler: ConsoleHandlerRoutine, add: i32) -> i32;
}

#[cfg(target_os = "windows")]
const CTRL_C_EVENT: u32 = 0;

#[cfg(target_os = "windows")]
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
    if ctrl_type == CTRL_C_EVENT {
        CTRLC_REQUESTED.store(true, Ordering::Relaxed);
        1 // TRUE = handled, don't terminate
    } else {
        0 // FALSE = pass to next handler
    }
}

use crate::orchestrator::Orchestrator;
use crate::perf::PerfMonitor;
use crate::recovery::RecoveryManager;
use crate::render_coordinator::RenderCoordinator;
use crate::render_thread;

#[derive(Debug, Error)]
pub enum DaemonError {
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

pub fn run(wallpaper_path: Option<PathBuf>) -> Result<(), DaemonError> {
    let _singleton = ProcessSingleton::acquire().map_err(|_| DaemonError::AlreadyRunning)?;
    tracing::info!("Process singleton acquired successfully");

    // Spawn async IPC server on a dedicated Tokio thread IMMEDIATELY at process startup (<2ms)
    // so UI client connections are accepted instantly without waiting for GPU or WorkerW init.
    let (ipc_shutdown_tx, ipc_shutdown_rx) = crossbeam_channel::bounded::<()>(1);

    #[cfg(target_os = "windows")]
    let monitors = MonitorEnumerator::enumerate()?;
    #[cfg(not(target_os = "windows"))]
    let monitors: Vec<aura_core::monitor::MonitorInfo> = Vec::new();

    let initial_monitor_summaries: Vec<aura_ipc::protocol::MonitorSummary> = monitors
        .iter()
        .enumerate()
        .map(|(idx, m)| aura_ipc::protocol::MonitorSummary {
            id: m.id,
            name: format!("Display {} ({})", idx + 1, m.device_name),
        })
        .collect();

    let orchestrator = Orchestrator::new(
        initial_monitor_summaries,
        std::collections::HashMap::new(),
        ipc_shutdown_tx,
    );

    let orchestrator_ipc = orchestrator.clone();
    let (ipc_server_shutdown_tx, ipc_server_shutdown_rx) = tokio::sync::watch::channel(false);
    let ipc_thread = std::thread::Builder::new()
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

    let orchestrator_watcher = orchestrator.clone();
    let scan_paths = orchestrator.scan_paths();
    let _watcher = aura_storage::LibraryWatcher::new(&scan_paths, move || {
        orchestrator_watcher.trigger_auto_refresh();
    })
    .ok();

    tracing::info!("IPC server listening on \\\\.\\pipe\\aura-wallpaperd");

    // Install Ctrl+C handler for graceful shutdown.
    #[cfg(target_os = "windows")]
    {
        let result = unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 1) };
        if result == 0 {
            tracing::warn!("Failed to install Ctrl+C handler via SetConsoleCtrlHandler");
        } else {
            tracing::info!("Console Ctrl+C handler installed");
        }
    }

    #[cfg(target_os = "windows")]
    let vulkan_context = Arc::new(VulkanContext::new()?);

    let mut workerw_manager = WorkerWManager::new();
    let mut attach_state = attach_or_detach(&mut workerw_manager);

    // Create per-monitor windows + renderers (each renderer runs in its own thread).
    #[cfg(target_os = "windows")]
    let (monitor_contexts, mut wallpaper_txs, perf_counters) = {
        let mut contexts = Vec::with_capacity(monitors.len());
        let mut txs = std::collections::HashMap::new();
        let mut counters = Vec::with_capacity(monitors.len());

        let config_path = aura_storage::config_store::ConfigStore::default_path();
        let config_store = aura_storage::config_store::ConfigStore::new(&config_path);
        let config = config_store.load().unwrap_or_default();
        let library_path = config_path.with_file_name("library.json");
        let library_store = aura_storage::library_store::LibraryStore::new(&library_path);
        let library_items = library_store.load().unwrap_or_default();

        let workerw = workerw_manager.workerw();
        for m in &monitors {
            let assignment = config.assignments.iter().find(|a| a.monitor_id == m.id);
            let initial_path = wallpaper_path.as_deref().or_else(|| {
                assignment
                    .and_then(|a| library_items.iter().find(|item| item.id == a.wallpaper_id))
                    .map(|item| item.path.as_path())
            });
            let fit_mode = assignment.map(|a| a.fit_mode).unwrap_or_default();

            match render_thread::create_monitor_context(
                &vulkan_context,
                m,
                workerw,
                initial_path,
                fit_mode,
            ) {
                Ok((ctx, tx, counter)) => {
                    contexts.push(ctx);
                    txs.insert(m.id, tx);
                    counters.push((m.id, counter));
                }
                Err(e) => tracing::error!("Failed to create monitor context: {}", e),
            }
        }
        (contexts, txs, counters)
    };
    #[cfg(not(target_os = "windows"))]
    let (monitor_contexts, mut wallpaper_txs, perf_counters) =
        (Vec::new(), std::collections::HashMap::new(), Vec::new());

    let monitor_summaries: Vec<aura_ipc::protocol::MonitorSummary> = monitors
        .iter()
        .enumerate()
        .map(|(idx, m)| aura_ipc::protocol::MonitorSummary {
            id: m.id,
            name: format!("Display {} ({})", idx + 1, m.device_name),
        })
        .collect();

    // Update Orchestrator with monitor summaries and wallpaper channels once monitor contexts are ready.
    orchestrator.update_monitors(monitor_summaries, wallpaper_txs.clone());

    let mut coordinator = RenderCoordinator::new(monitor_contexts);

    // Spawn platform event pump thread.
    let event_pump = EventPump::new();
    let receiver = event_pump.receiver.clone();
    let (pump_handle, pump_thread) = event_pump.spawn();

    tracing::info!(
        "wallpaperd orchestrator running — {} monitors, WorkerW: {:?}",
        coordinator.monitor_count(),
        attach_state
    );

    let mut perf_mon = PerfMonitor::new(perf_counters);

    // Main event dispatch loop (no rendering — render threads handle that).
    loop {
        if ipc_shutdown_rx.try_recv().is_ok() {
            tracing::info!("IPC shutdown requested. Exiting daemon...");
            break;
        }

        if CTRLC_REQUESTED.load(Ordering::Relaxed) {
            tracing::info!("Ctrl+C received. Exiting daemon...");
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
                tracing::info!("Display topology changed — reconciling monitors");
                attach_state = attach_or_detach(&mut workerw_manager);
                #[cfg(target_os = "windows")]
                reconcile_monitors(
                    &vulkan_context,
                    &mut workerw_manager,
                    &mut coordinator,
                    &mut wallpaper_txs,
                    &orchestrator,
                    wallpaper_path.as_deref(),
                );
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

        perf_mon.log_if_interval();
    }

    // Shutdown: signal IPC server and render threads.
    let _ = ipc_server_shutdown_tx.send(true);

    // Join render threads with a timeout to prevent indefinite hangs.
    coordinator.shutdown_with_timeout(Duration::from_secs(3));

    // Join IPC server thread.
    let _ = ipc_thread.join();

    // Signal event pump message loop to exit, then join the thread.
    pump_handle.shutdown();
    let _ = pump_thread.join();

    #[cfg(target_os = "windows")]
    aura_platform_windows::workerw::restore_desktop_wallpaper();

    tracing::info!("wallpaperd daemon shutdown complete");
    Ok(())
}

fn attach_or_detach(manager: &mut WorkerWManager) -> AttachState {
    match manager.find_workerw() {
        Ok(hwnd) => {
            tracing::info!("WorkerW attachment target resolved: HWND({:?})", hwnd.0);
            unsafe {
                use windows::Win32::Foundation::RECT;
                use windows::Win32::UI::WindowsAndMessaging::{
                    GetClassNameW, GetClientRect, IsWindowVisible,
                };
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let mut class_buf = [0u16; 256];
                let len = GetClassNameW(hwnd, &mut class_buf);
                let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
                tracing::info!(
                    "Attach target class='{}' client_rect={}x{} visible={}",
                    class_name,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    IsWindowVisible(hwnd).as_bool(),
                );
            }
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

#[cfg(target_os = "windows")]
fn reconcile_monitors(
    vulkan_context: &Arc<VulkanContext>,
    workerw_manager: &mut WorkerWManager,
    coordinator: &mut RenderCoordinator,
    wallpaper_txs: &mut std::collections::HashMap<
        aura_core::monitor::MonitorId,
        crossbeam_channel::Sender<render_thread::RenderCommand>,
    >,
    orchestrator: &Orchestrator,
    wallpaper_path: Option<&std::path::Path>,
) {
    let new_monitors = match RecoveryManager::handle_display_change() {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to re-enumerate monitors: {}", e);
            return;
        }
    };

    let workerw = workerw_manager.workerw();
    let current_ids = coordinator.active_monitor_ids();
    let new_ids: std::collections::HashSet<_> = new_monitors.iter().map(|m| m.id).collect();

    // 1. Remove disconnected monitors
    for old_id in current_ids {
        if !new_ids.contains(&old_id) {
            tracing::info!("Monitor {:?} disconnected — stopping render thread", old_id);
            coordinator.remove_monitor(old_id);
            wallpaper_txs.remove(&old_id);
        }
    }

    // Load current config and library store for newly added monitors
    let config_path = aura_storage::config_store::ConfigStore::default_path();
    let config_store = aura_storage::config_store::ConfigStore::new(&config_path);
    let config = config_store.load().unwrap_or_default();
    let library_path = config_path.with_file_name("library.json");
    let library_store = aura_storage::library_store::LibraryStore::new(&library_path);
    let library_items = library_store.load().unwrap_or_default();

    // 2. Process active / added / resized monitors
    for m in &new_monitors {
        if let Some(ctx) = coordinator.find_monitor_mut(m.id) {
            // Check if bounds changed
            if ctx.width != m.width || ctx.height != m.height || ctx.x != m.x || ctx.y != m.y {
                tracing::info!(
                    "Monitor {:?} resized/moved: ({}x{}) -> ({}x{})",
                    m.id,
                    ctx.width,
                    ctx.height,
                    m.width,
                    m.height
                );
                ctx.update_geometry(workerw, m.x, m.y, m.width, m.height);
                if let Some(tx) = wallpaper_txs.get(&m.id) {
                    let _ = tx.send(render_thread::RenderCommand::Resize {
                        width: m.width,
                        height: m.height,
                    });
                }
            } else {
                ctx.attach_to_workerw(workerw);
            }
        } else {
            // Added monitor
            tracing::info!("New monitor detected: {:?}", m.id);
            let assignment = config.assignments.iter().find(|a| a.monitor_id == m.id);
            let initial_path = wallpaper_path.or_else(|| {
                assignment
                    .and_then(|a| library_items.iter().find(|item| item.id == a.wallpaper_id))
                    .map(|item| item.path.as_path())
            });
            let fit_mode = assignment.map(|a| a.fit_mode).unwrap_or_default();

            match render_thread::create_monitor_context(
                vulkan_context,
                m,
                workerw,
                initial_path,
                fit_mode,
            ) {
                Ok((ctx, tx, _counter)) => {
                    ctx.attach_to_workerw(workerw);
                    wallpaper_txs.insert(m.id, tx.clone());
                    coordinator.add_monitor(ctx);
                }
                Err(e) => {
                    tracing::error!("Failed to create monitor context for new monitor: {}", e);
                }
            }
        }
    }

    // 3. Update IPC Orchestrator summaries
    let summaries: Vec<aura_ipc::protocol::MonitorSummary> = new_monitors
        .iter()
        .enumerate()
        .map(|(idx, m)| aura_ipc::protocol::MonitorSummary {
            id: m.id,
            name: format!("Display {} ({})", idx + 1, m.device_name),
        })
        .collect();

    orchestrator.update_monitors(summaries, wallpaper_txs.clone());
}