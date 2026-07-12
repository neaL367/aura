use crate::app::composition_root::CompositionRoot;
use crate::config::model::AppConfig;
use crate::domain::traits::ConfigStore;
use crate::utils::error::Result;
use crate::domain::fit_mode::FitMode;

/// Checks if the main configuration window exists and is currently visible.
pub fn main_window_visible(root: &CompositionRoot) -> bool {
    if let Some(ref win) = root.main_window {
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(win.hwnd).as_bool() }
    } else {
        false
    }
}

/// Renders the configuration UI, syncing display snapshots and invoking view layouts.
pub fn render_ui(root: &mut CompositionRoot, config: &mut AppConfig, store: &dyn ConfigStore) -> Result<()> {
    let Some(ref mut main_window) = root.main_window else {
        return Ok(());
    };

    // 1. Calculate window size and screen rect
    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(main_window.hwnd, &mut rect);
    }
    let width = (rect.right - rect.left) as f32;
    let height = (rect.bottom - rect.top) as f32;
    let screen_rect = egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(width, height),
    );

    // 2. Query GDI monitor layouts to build snaps
    let mut snapshots = Vec::new();
    for (id, info) in root.monitor_manager.all() {
        let fit_mode = config.monitors.iter()
            .find(|m| m.monitor_id == id.0)
            .map(|m| m.fit_mode)
            .unwrap_or(FitMode::Fill);

        let assigned_entry = config.monitors.iter()
            .find(|m| m.monitor_id == id.0)
            .and_then(|m| m.wallpaper_id.as_ref())
            .and_then(|wp_id| config.library.iter().find(|e| e.id == *wp_id))
            .cloned();

        snapshots.push(crate::ui::views::monitor_panel::MonitorSnapshot {
            id: id.clone(),
            display_label: format!("{} ({})", info.gdi_device_name, id.0),
            width: (info.rect.right - info.rect.left) as u32,
            height: (info.rect.bottom - info.rect.top) as u32,
            fit_mode,
            assigned_entry,
        });
    }

    let mut config_dirty = false;

    // 4. Pump egui backend frames
    main_window.egui.frame(
        screen_rect,
        &root.device.device,
        &root.device.context,
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut on_pick = None;

                let grid_action = crate::ui::views::library_grid::show(
                    ui,
                    config,
                    &mut root.thumbnail_cache,
                );

                match grid_action {
                    Some(crate::ui::views::library_grid::LibraryGridAction::Picked(entry)) => {
                        on_pick = Some(entry);
                    }
                    Some(crate::ui::views::library_grid::LibraryGridAction::RequestRemove(id)) => {
                        root.thumbnail_cache.invalidate(&id);
                        if crate::library::scanner::remove_entry(config, &id) {
                            config_dirty = true;
                        }
                    }
                    None => {}
                }

                let mut service = crate::services::set_wallpaper_service::SetWallpaperService {
                    device: root.device.device.clone(),
                    device_context: root.device.context.clone(),
                    renderer: root.renderer.clone(),
                    pipelines: &mut root.pipelines,
                    config,
                    store,
                };

                crate::ui::views::monitor_panel::show(
                    ui,
                    &mut root.monitor_panel_state,
                    &snapshots,
                    &mut root.thumbnail_cache,
                    &mut service,
                    on_pick.as_ref(),
                );
            });
        },
    )?;

    if config_dirty {
        store.save(config)?;
    }

    Ok(())
}
