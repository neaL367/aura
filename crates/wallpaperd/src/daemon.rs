use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aura_platform_windows::PlatformError;
use aura_platform_windows::event_pump::{EventPump, HostEvent};
use aura_platform_windows::monitor_enumerator::MonitorEnumerator;
use aura_platform_windows::singleton::ProcessSingleton;
use aura_platform_windows::workerw::WorkerWManager;
use aura_renderer_vulkan::VulkanContext;
use crossbeam_channel::RecvTimeoutError;
use thiserror::Error;

use aura_platform_windows::register_console_ctrl_handler;
use aura_platform_windows::set_autostart;

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

#[derive(Debug, Clone, Copy)]
enum AttachState {
    Attached,
    Detached { retry_count: u32 },
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
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
            };
            let mut msg = MSG::default();
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

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

    #[cfg(target_os = "windows")]
    pump_messages_once();

    let workerw = workerw_manager.workerw();
    let current_ids = coordinator.active_monitor_ids();
    let new_ids: std::collections::HashSet<_> = new_monitors.iter().map(|m| m.id).collect();

    // Compute virtual desktop bounds from the full latest monitor set.
    let vx = new_monitors.iter().map(|m| m.x).min().unwrap_or(0);
    let vy = new_monitors.iter().map(|m| m.y).min().unwrap_or(0);
    let vw = new_monitors
        .iter()
        .map(|m| m.x + m.width as i32)
        .max()
        .unwrap_or(1920)
        .max(1) as u32
        - vx as u32;
    let vh = new_monitors
        .iter()
        .map(|m| m.y + m.height as i32)
        .max()
        .unwrap_or(1080)
        .max(1) as u32
        - vy as u32;

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
        let is_invalid = coordinator
            .find_monitor_mut(m.id)
            .map(|ctx| !ctx.host_window.is_valid())
            .unwrap_or(false);

        if is_invalid {
            tracing::warn!(
                "Host window for monitor {:?} is invalid — removing context for recreation",
                m.id
            );
            coordinator.remove_monitor(m.id);
            wallpaper_txs.remove(&m.id);
        }

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
                (vx, vy, vw, vh),
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
            // Keep the message queue alive during multi-monitor recreation
            #[cfg(target_os = "windows")]
            pump_messages_once();
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

fn run_slideshow_cycle(
    wallpaper_txs: &std::collections::HashMap<
        aura_core::monitor::MonitorId,
        crossbeam_channel::Sender<render_thread::RenderCommand>,
    >,
    orchestrator: &Orchestrator,
    items: &[aura_core::wallpaper::WallpaperMeta],
    state: &mut Option<aura_core::slideshow_state::SlideshowState>,
    store: &aura_storage::slideshow_store::SlideshowStore,
) {
    use rand::seq::SliceRandom;

    let s = state.get_or_insert_with(aura_core::slideshow_state::SlideshowState::new);

    // Identify monitors without manual assignments (slideshow monitors).
    let mut slideshow_monitors: Vec<aura_core::monitor::MonitorId> = wallpaper_txs
        .keys()
        .filter(|m| !orchestrator.is_monitor_assigned(m))
        .copied()
        .collect();
    if slideshow_monitors.is_empty() {
        return;
    }
    slideshow_monitors.sort_by_key(|m| m.as_uuid());

    // Sanitize queue: remove IDs no longer in the library.
    s.queue.retain(|id| items.iter().any(|item| item.id == *id));
    s.queue.shrink_to_fit();
    s.index = s.index.min(s.queue.len().saturating_sub(1));

    // If queue is too short or empty, rebuild from all library IDs.
    if s.queue.len() < slideshow_monitors.len() {
        s.queue = items.iter().map(|item| item.id).collect();
        s.queue.shuffle(&mut rand::rng());
        s.queue.shrink_to_fit();
        s.index = 0;
    }

    let mut assigned_this_cycle = Vec::with_capacity(slideshow_monitors.len());

    for monitor in &slideshow_monitors {
        // Wrap-around: reshuffle when queue is exhausted.
        if s.index >= s.queue.len() {
            s.queue.shuffle(&mut rand::rng());
            s.index = 0;
            s.last_cycle += 1;
        }

        let candidate = s.queue[s.index];

        // Check duplicates: avoid same wallpaper on same monitor consecutively
        // and same wallpaper in this cycle across monitors.
        let is_repeat = s.last_wallpapers.get(monitor) == Some(&candidate)
            || assigned_this_cycle.contains(&candidate);

        if is_repeat {
            // Scan forward for a suitable alternative.
            let mut swap_idx = None;
            for offset in 1..s.queue.len() {
                let idx = (s.index + offset) % s.queue.len();
                let alt = s.queue[idx];
                if alt != candidate
                    && s.last_wallpapers.get(monitor) != Some(&alt)
                    && !assigned_this_cycle.contains(&alt)
                {
                    swap_idx = Some(idx);
                    break;
                }
            }
            if let Some(si) = swap_idx {
                s.queue.swap(s.index, si);
            }
            // No alternative found → use candidate anyway (unavoidable repeat).
        }

        let chosen = s.queue[s.index];
        s.last_wallpapers.insert(*monitor, chosen);
        assigned_this_cycle.push(chosen);

        if let (Some(meta), Some(tx)) = (
            items.iter().find(|item| item.id == chosen),
            wallpaper_txs.get(monitor),
        ) {
            let _ = tx.send(render_thread::RenderCommand::SetWallpaper {
                path: meta.path.clone(),
                fit_mode: None,
            });
        }

        s.index += 1;
    }

    if let Err(e) = store.save(s) {
        tracing::warn!("Failed to save slideshow state: {}", e);
    }
}

/// Drain any pending Win32 messages so the window message queue doesn't go
/// un-pumped during long operations like surface recreation.  This prevents
/// Windows from marking the daemon as "Not Responding".
#[cfg(target_os = "windows")]
fn pump_messages_once() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
    };
    let mut msg = MSG::default();
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
