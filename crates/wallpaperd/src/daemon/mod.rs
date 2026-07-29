pub mod reconciliation;
pub mod slideshow;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aura_platform_windows::event_pump::{EventPump, HostEvent};
use aura_platform_windows::monitor_enumerator::MonitorEnumerator;
use aura_platform_windows::register_console_ctrl_handler;
use aura_platform_windows::set_autostart;
use aura_platform_windows::singleton::ProcessSingleton;
use aura_platform_windows::workerw::WorkerWManager;
use aura_renderer_vulkan::VulkanContext;
use crossbeam_channel::RecvTimeoutError;
use thiserror::Error;

pub use reconciliation::{AttachState, attach_or_detach, pump_messages_once, reconcile_monitors};
pub use slideshow::run_slideshow_cycle;

static CTRLC_REQUESTED: std::sync::LazyLock<Arc<AtomicBool>> =
    std::sync::LazyLock::new(|| Arc::new(AtomicBool::new(false)));

use crate::orchestrator::Orchestrator;
use crate::perf_monitor::PerfMonitor;
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

/// Configuration passed into `run()` from the hosting binary.
pub struct DaemonOptions {
    pub wallpaper_path: Option<PathBuf>,
    pub shutdown_rx: crossbeam_channel::Receiver<()>,
    pub ready_tx: std::sync::mpsc::SyncSender<()>,
    pub done_tx: crossbeam_channel::Sender<()>,
    pub _singleton: ProcessSingleton,
}

impl DaemonOptions {
    /// Create options suitable for a standalone headless daemon
    /// (wallpaperd -- standalone binary).
    pub fn standalone(wallpaper_path: Option<PathBuf>) -> Self {
        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded::<()>(1);
        // Leak sender so receiver never closes (standalone runs until signal).
        std::mem::forget(shutdown_tx);
        let (ready_tx, _) = std::sync::mpsc::sync_channel(1);
        let (done_tx, _) = crossbeam_channel::bounded(1);
        let singleton =
            ProcessSingleton::acquire().expect("another wallpaperd instance is already running");
        Self {
            wallpaper_path,
            shutdown_rx,
            ready_tx,
            done_tx,
            _singleton: singleton,
        }
    }
}

pub fn run(opts: DaemonOptions) -> Result<(), DaemonError> {
    let wallpaper_path = opts.wallpaper_path;
    let shutdown_rx = opts.shutdown_rx;
    let ready_tx = opts.ready_tx;
    let done_tx = opts.done_tx;
    let _singleton = opts._singleton;
    tracing::info!("Process singleton held by daemon thread");

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
                let server = aura_ipc::server::IpcServer::new(handler)
                    .with_client_validation()
                    .on_ready(move || {
                        let _ = ready_tx.send(());
                    });
                if let Err(e) = server.serve(ipc_server_shutdown_rx).await {
                    tracing::error!("IPC server error: {}", e);
                }
            });
        })
        .map_err(|_| DaemonError::ThreadSpawn)?;

    let orchestrator_watcher = orchestrator.clone();
    let library_path = orchestrator.library_path();
    if let Ok(watcher) = aura_storage::LibraryWatcher::new(&[library_path], move || {
        orchestrator_watcher.trigger_auto_refresh();
    }) {
        orchestrator.set_watcher(watcher);
    }

    tracing::info!("IPC server listening on \\\\.\\pipe\\aura-wallpaperd");

    // Install Ctrl+C handler for graceful shutdown.
    if register_console_ctrl_handler(CTRLC_REQUESTED.clone()) {
        tracing::info!("Console Ctrl+C handler registered successfully");
    } else {
        tracing::warn!("Failed to register Console Ctrl+C handler");
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
        #[cfg(target_os = "windows")]
        set_autostart(config.appearance.auto_start);
        let library_path = config_path.with_file_name("library.json");
        let library_store = aura_storage::library_store::LibraryStore::new(&library_path);
        let library_items = library_store.load().unwrap_or_default();

        let config_dir = config_path.parent().unwrap_or(&config_path);
        let _ = aura_storage::cleanup_stale_temp_files(
            config_dir,
            std::time::Duration::from_secs(24 * 60 * 60),
        );
        let thumbs_dir = config_dir.join("thumbs");
        let _ = aura_storage::cleanup_stale_temp_files(
            &thumbs_dir,
            std::time::Duration::from_secs(24 * 60 * 60),
        );

        let workerw = workerw_manager.workerw();
        let vx = monitors.iter().map(|m| m.x).min().unwrap_or(0);
        let vy = monitors.iter().map(|m| m.y).min().unwrap_or(0);
        let vw = monitors
            .iter()
            .map(|m| m.x + m.width as i32)
            .max()
            .unwrap_or(1920)
            .max(1) as u32
            - vx as u32;
        let vh = monitors
            .iter()
            .map(|m| m.y + m.height as i32)
            .max()
            .unwrap_or(1080)
            .max(1) as u32
            - vy as u32;
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
                (vx, vy, vw, vh),
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

    let config_path = aura_storage::config_store::ConfigStore::default_path();
    let cfg_store = aura_storage::config_store::ConfigStore::new(&config_path);
    let library_path = config_path.with_file_name("library.json");
    let lib_store = aura_storage::library_store::LibraryStore::new(&library_path);
    let slideshow_store = aura_storage::slideshow_store::SlideshowStore::new(
        aura_storage::slideshow_store::SlideshowStore::default_path(),
    );
    let mut slideshow_state: Option<aura_core::slideshow_state::SlideshowState> =
        slideshow_store.load().ok().flatten();
    let mut last_slideshow = std::time::Instant::now();

    // Main event dispatch loop (no rendering — render threads handle that).
    loop {
        if ipc_shutdown_rx.try_recv().is_ok() {
            tracing::info!("IPC shutdown requested. Exiting daemon...");
            break;
        }

        if shutdown_rx.try_recv().is_ok() {
            tracing::info!("Shutdown signal from host binary. Exiting daemon...");
            break;
        }

        if CTRLC_REQUESTED.load(Ordering::Relaxed) {
            tracing::info!("Ctrl+C received. Exiting daemon...");
            break;
        }

        // Apply paused state to render threads if changed via IPC.
        coordinator.set_paused(orchestrator.is_paused());

        #[cfg(target_os = "windows")]
        pump_messages_once();

        let event = receiver.recv_timeout(Duration::from_millis(500));

        match event {
            Ok(HostEvent::ExplorerRestarted) => {
                tracing::warn!(
                    "Explorer restart signal received — recreating host windows and desktop attachment"
                );
                if RecoveryManager::handle_explorer_restart(&mut workerw_manager) {
                    attach_state = AttachState::Attached;
                    coordinator.shutdown_all();
                    wallpaper_txs.clear();
                    #[cfg(target_os = "windows")]
                    reconcile_monitors(
                        &vulkan_context,
                        &mut workerw_manager,
                        &mut coordinator,
                        &mut wallpaper_txs,
                        &orchestrator,
                        wallpaper_path.as_deref(),
                    );
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
                orchestrator.set_performance_profile(profile);
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
                tracing::info!(
                    "WorkerW re-attached in background retry (after {} attempts)",
                    *retry_count
                );
                attach_state = AttachState::Attached;
                coordinator.shutdown_all();
                wallpaper_txs.clear();
                #[cfg(target_os = "windows")]
                reconcile_monitors(
                    &vulkan_context,
                    &mut workerw_manager,
                    &mut coordinator,
                    &mut wallpaper_txs,
                    &orchestrator,
                    wallpaper_path.as_deref(),
                );
            } else {
                *retry_count += 1;
            }
        }

        // Slideshow: cycle wallpapers if interval (seconds > 0) has elapsed.
        if last_slideshow.elapsed()
            >= cfg_store
                .load()
                .ok()
                .map_or(std::time::Duration::ZERO, |c| {
                    std::time::Duration::from_secs(c.appearance.slideshow_interval_secs)
                })
            && let Ok(items) = lib_store.load()
            && !items.is_empty()
        {
            last_slideshow = std::time::Instant::now();
            run_slideshow_cycle(
                &wallpaper_txs,
                &orchestrator,
                &items,
                &mut slideshow_state,
                &slideshow_store,
            );
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
    let _ = done_tx.send(());
    Ok(())
}
