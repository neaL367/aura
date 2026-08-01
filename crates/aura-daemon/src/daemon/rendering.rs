use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicU64};

use aura_vulkan::VulkanContext;
use aura_win::set_autostart;
use aura_win::workerw::WorkerWManager;

use crate::render_coordinator::MonitorContext;
use crate::render_thread;
use crate::render_thread::RenderCommand;

use super::DaemonError;

/// Per-monitor render command channels keyed by hardware-stable monitor id.
pub(super) type WallpaperChannels = std::collections::HashMap<
    aura_core::monitor::MonitorId,
    crossbeam_channel::Sender<RenderCommand>,
>;

/// Per-monitor frame counters (monitor id, frame counter pair).
pub(super) type MonitorCounters = Vec<(aura_core::monitor::MonitorId, Arc<AtomicU64>)>;

/// Everything created for a monitor render thread: its context, command
/// channel, and frame counter.
pub(super) type RenderSetup = (Vec<MonitorContext>, WallpaperChannels, MonitorCounters);

/// Create per-monitor host windows + renderers (each renderer runs in
/// its own thread), applying persisted wallpaper assignments.
#[cfg(target_os = "windows")]
pub(super) fn setup_monitor_rendering(
    vulkan_context: &Arc<VulkanContext>,
    monitors: &[aura_core::monitor::MonitorInfo],
    workerw_manager: &WorkerWManager,
    wallpaper_path: &Option<PathBuf>,
) -> Result<RenderSetup, DaemonError> {
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
    for m in monitors {
        let assignment = config.assignments.iter().find(|a| a.monitor_id == m.id);
        let initial_path = wallpaper_path.as_deref().or_else(|| {
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
            Ok((ctx, tx, counter)) => {
                contexts.push(ctx);
                txs.insert(m.id, tx);
                counters.push((m.id, counter));
            }
            Err(e) => tracing::error!("Failed to create monitor context: {}", e),
        }
    }
    Ok((contexts, txs, counters))
}
