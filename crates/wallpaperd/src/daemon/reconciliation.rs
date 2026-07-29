use std::sync::Arc;

use aura_platform_windows::PlatformError;
use aura_platform_windows::workerw::WorkerWManager;
use aura_renderer_vulkan::VulkanContext;

use crate::orchestrator::Orchestrator;
use crate::recovery::RecoveryManager;
use crate::render_coordinator::RenderCoordinator;
use crate::render_thread;

#[derive(Debug, Clone, Copy)]
pub enum AttachState {
    Attached,
    Detached { retry_count: u32 },
}

pub fn attach_or_detach(manager: &mut WorkerWManager) -> AttachState {
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
pub fn reconcile_monitors(
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

#[cfg(target_os = "windows")]
pub fn check_game_mode_pause(coordinator: &mut RenderCoordinator, game_mode_paused: &mut bool) {
    let is_fullscreen = aura_platform_windows::is_fullscreen_app_active();
    if is_fullscreen && !*game_mode_paused {
        tracing::info!("Game Mode: Full-screen app detected — pausing presentation (0% CPU/GPU)");
        coordinator.set_paused(true);
        *game_mode_paused = true;
    } else if !is_fullscreen && *game_mode_paused {
        tracing::info!("Game Mode: Full-screen app closed — resuming presentation");
        coordinator.set_paused(false);
        *game_mode_paused = false;
    }
}

/// Drain any pending Win32 messages so the window message queue doesn't go
/// un-pumped during long operations like surface recreation. This prevents
/// Windows from marking the daemon as "Not Responding".
#[cfg(target_os = "windows")]
pub fn pump_messages_once() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
    };
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
