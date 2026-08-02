pub mod options;
pub mod reconciliation;
pub mod slideshow;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use aura_vulkan::VulkanContext;
use aura_win::event_pump::EventPump;
use aura_win::monitor_enumerator::MonitorEnumerator;
use aura_win::register_console_ctrl_handler;
use aura_win::workerw::WorkerWManager;

pub use options::{DaemonError, DaemonOptions};
pub use reconciliation::{
    AttachState, attach_or_detach, check_game_mode_pause, pump_messages_once, reconcile_monitors,
};
pub use slideshow::{run_slideshow_cycle, select_slideshow_items};

/// Effective render pause state: the user's manual pause (via IPC) OR the
/// game-mode fullscreen pause. Game mode is an additional pause source and
/// must never overwrite the manual pause state.
pub fn effective_paused_state(manual_paused: bool, game_mode_paused: bool) -> bool {
    manual_paused || game_mode_paused
}

use crate::orchestrator::Orchestrator;
use crate::perf_monitor::PerfMonitor;
use crate::render_coordinator::RenderCoordinator;
use crate::slideshow_preload::SlideshowPreloader;

mod event_loop;
mod ipc;
mod rendering;

pub fn run(opts: DaemonOptions) -> Result<(), DaemonError> {
    let wallpaper_path = opts.wallpaper_path;
    let shutdown_rx = opts.shutdown_rx;
    let ready_tx = opts.ready_tx;
    let done_tx = opts.done_tx;
    let _singleton = opts._singleton;
    let ctrlc_flag = opts.ctrlc_flag;
    tracing::info!("Process singleton held by daemon thread");

    // Install standalone handler before monitor, IPC, or Vulkan startup so
    // Ctrl+C cannot be lost during expensive initialization.
    let ctrlc_flag = match ctrlc_flag {
        Some(flag) => {
            tracing::info!("Using caller-supplied Ctrl+C flag");
            flag
        }
        None => {
            let flag = Arc::new(AtomicBool::new(false));
            if register_console_ctrl_handler(flag.clone(), None) {
                tracing::info!("Console Ctrl+C handler registered (standalone)");
            } else {
                tracing::warn!("Failed to register Console Ctrl+C handler");
            }
            flag
        }
    };

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

    let (ipc_thread, ipc_server_shutdown_tx) = ipc::spawn_ipc_server(&orchestrator, ready_tx)?;

    let orchestrator_watcher = orchestrator.clone();
    let library_path = orchestrator.library_path();
    if let Ok(watcher) = aura_storage::LibraryWatcher::new(&[library_path], move || {
        orchestrator_watcher.trigger_auto_refresh();
    }) {
        orchestrator.set_watcher(watcher);
    }

    tracing::info!("IPC server listening on \\\\.\\pipe\\aura-wallpaperd");

    #[cfg(target_os = "windows")]
    let vulkan_context = Arc::new(VulkanContext::new()?);

    // Check for Ctrl+C before proceeding with GPU-heavy operations.
    let mut early_shutdown = ctrlc_flag.load(std::sync::atomic::Ordering::Relaxed);

    let mut workerw_manager = WorkerWManager::new();
    let mut attach_state = attach_or_detach(&mut workerw_manager);

    #[cfg(target_os = "windows")]
    let (monitor_contexts, mut wallpaper_txs, perf_counters) = if early_shutdown {
        (Vec::new(), std::collections::HashMap::new(), Vec::new())
    } else {
        rendering::setup_monitor_rendering(
            &vulkan_context,
            &monitors,
            &workerw_manager,
            &wallpaper_path,
        )?
    };
    #[cfg(not(target_os = "windows"))]
    let (monitor_contexts, mut wallpaper_txs, perf_counters) =
        (Vec::new(), std::collections::HashMap::new(), Vec::new());

    early_shutdown = early_shutdown || ctrlc_flag.load(std::sync::atomic::Ordering::Relaxed);

    let monitor_summaries: Vec<aura_ipc::protocol::MonitorSummary> = monitors
        .iter()
        .enumerate()
        .map(|(idx, m)| aura_ipc::protocol::MonitorSummary {
            id: m.id,
            name: format!("Display {} ({})", idx + 1, m.device_name),
        })
        .collect();

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

    let mut game_mode_paused = false;
    let mut slideshow_preloader = SlideshowPreloader::new();

    let event_loop_result = if early_shutdown {
        tracing::info!("Ctrl+C requested during daemon initialization — skipping event loop");
        Ok(())
    } else {
        event_loop::run_event_loop(
            &ipc_shutdown_rx,
            &shutdown_rx,
            &mut coordinator,
            &orchestrator,
            &mut game_mode_paused,
            &receiver,
            &mut workerw_manager,
            &mut attach_state,
            &mut wallpaper_txs,
            #[cfg(target_os = "windows")]
            &vulkan_context,
            &wallpaper_path,
            &mut perf_mon,
            &cfg_store,
            &lib_store,
            &slideshow_store,
            &mut slideshow_state,
            &mut slideshow_preloader,
            &ctrlc_flag,
        )
    };

    // Shutdown: signal IPC server and render threads (always executes).
    let _ = ipc_server_shutdown_tx.send(true);

    // Join slideshow preload threads.
    slideshow_preloader.join_all();

    // Join render threads with a timeout to prevent indefinite hangs.
    coordinator.shutdown_with_timeout(Duration::from_secs(3));

    // Join IPC server thread.
    let _ = ipc_thread.join();

    // Signal event pump message loop to exit, then join the thread.
    pump_handle.shutdown();
    let _ = pump_thread.join();

    #[cfg(target_os = "windows")]
    aura_win::workerw::restore_desktop_wallpaper();

    tracing::info!("wallpaperd daemon shutdown complete");
    let _ = done_tx.send(());
    event_loop_result?;
    Ok(())
}
