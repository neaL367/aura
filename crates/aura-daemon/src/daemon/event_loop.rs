use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use aura_win::event_pump::HostEvent;
use aura_win::workerw::WorkerWManager;
use crossbeam_channel::{Receiver, RecvTimeoutError};

use crate::orchestrator::Orchestrator;
use crate::perf_monitor::PerfMonitor;
use crate::recovery::RecoveryManager;
use crate::render_coordinator::RenderCoordinator;
use crate::render_thread::RenderCommand;
use crate::slideshow_preload::SlideshowPreloader;

use super::{
    AttachState, CTRLC_REQUESTED, DaemonError, attach_or_detach, check_game_mode_pause,
    effective_paused_state, pump_messages_once, reconcile_monitors, run_slideshow_cycle,
    select_slideshow_items,
};
#[cfg(target_os = "windows")]
use aura_vulkan::VulkanContext;

/// Main event dispatch loop (no rendering — render threads handle that).
#[allow(clippy::too_many_arguments)]
pub(super) fn run_event_loop(
    ipc_shutdown_rx: &Receiver<()>,
    shutdown_rx: &Receiver<()>,
    coordinator: &mut RenderCoordinator,
    orchestrator: &Orchestrator,
    game_mode_paused: &mut bool,
    receiver: &Receiver<HostEvent>,
    workerw_manager: &mut WorkerWManager,
    mut attach_state: &mut AttachState,
    wallpaper_txs: &mut std::collections::HashMap<
        aura_core::monitor::MonitorId,
        crossbeam_channel::Sender<RenderCommand>,
    >,
    #[cfg(target_os = "windows")] vulkan_context: &Arc<VulkanContext>,
    wallpaper_path: &Option<PathBuf>,
    perf_mon: &mut PerfMonitor,
    cfg_store: &aura_storage::config_store::ConfigStore,
    lib_store: &aura_storage::library_store::LibraryStore,
    slideshow_store: &aura_storage::slideshow_store::SlideshowStore,
    slideshow_state: &mut Option<aura_core::slideshow_state::SlideshowState>,
    slideshow_preloader: &mut SlideshowPreloader,
) -> Result<(), DaemonError> {
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

        // Apply paused state to render threads: user pause (via IPC) OR
        // game-mode fullscreen detection. Game mode is an additional pause
        // source — it never overwrites the manual pause state.
        coordinator.set_paused(effective_paused_state(
            orchestrator.is_paused(),
            *game_mode_paused,
        ));

        // Poll full-screen foreground detection (~500ms cadence via
        // recv_timeout below). Cheap: GetForegroundWindow + GetMonitorInfoW.
        #[cfg(target_os = "windows")]
        check_game_mode_pause(game_mode_paused);

        #[cfg(target_os = "windows")]
        pump_messages_once();

        let event = receiver.recv_timeout(Duration::from_millis(500));

        match event {
            Ok(HostEvent::ExplorerRestarted) => {
                tracing::warn!(
                    "Explorer restart signal received — recreating host windows and desktop attachment"
                );
                if RecoveryManager::handle_explorer_restart(workerw_manager) {
                    *attach_state = AttachState::Attached;
                    coordinator.shutdown_all();
                    wallpaper_txs.clear();
                    #[cfg(target_os = "windows")]
                    reconcile_monitors(
                        vulkan_context,
                        workerw_manager,
                        coordinator,
                        wallpaper_txs,
                        orchestrator,
                        wallpaper_path.as_deref(),
                    );
                } else {
                    *attach_state = AttachState::Detached { retry_count: 0 };
                }
            }
            Ok(HostEvent::DisplayChanged) => {
                tracing::info!("Display topology changed — reconciling monitors");
                *attach_state = attach_or_detach(workerw_manager);
                #[cfg(target_os = "windows")]
                reconcile_monitors(
                    vulkan_context,
                    workerw_manager,
                    coordinator,
                    wallpaper_txs,
                    orchestrator,
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
                *attach_state = AttachState::Attached;
                coordinator.shutdown_all();
                wallpaper_txs.clear();
                #[cfg(target_os = "windows")]
                reconcile_monitors(
                    vulkan_context,
                    workerw_manager,
                    coordinator,
                    wallpaper_txs,
                    orchestrator,
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
                wallpaper_txs,
                orchestrator,
                &items,
                slideshow_state,
                slideshow_store,
                slideshow_preloader,
            );

            // Preload the NEXT cycle's static images on a background thread
            // while the current ones play. Selection runs on a clone of the
            // slideshow state so the real queue is not advanced early; a
            // mismatch at the next fire is a harmless cache miss.
            if let Some(next_state) = slideshow_state.clone() {
                let mut next_opt = Some(next_state);
                let next_selection = select_slideshow_items(
                    wallpaper_txs,
                    |m| orchestrator.is_monitor_assigned(m),
                    &items,
                    &mut next_opt,
                );
                slideshow_preloader.schedule_next(&next_selection, &items);
            }
        }

        perf_mon.log_if_interval();
    }

    Ok(())
}
