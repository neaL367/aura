use std::collections::HashSet;

use aura_core::{
    monitor::{MonitorAssignment, MonitorId},
    wallpaper::FitMode,
};
use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub fn render_fit_mode_selector(
    ui: &mut egui::Ui,
    selected_fit_mode: &mut FitMode,
    entry: &WallpaperEntry,
    monitors: &[aura_ipc::protocol::MonitorSummary],
    assigned_mons: &HashSet<&MonitorId>,
    has_generic: bool,
    ipc_client: &UiIpcClient,
) {
    theme::section_label(ui, "FIT MODE");
    ui.add_space(theme::SPACING_SM);
    theme::segmented_container(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(theme::SPACING_XS, theme::SPACING_XS);
            for mode in [
                FitMode::Fill,
                FitMode::Fit,
                FitMode::Stretch,
                FitMode::Center,
                FitMode::Tile,
                FitMode::Span,
            ] {
                let variant = if *selected_fit_mode == mode {
                    theme::ButtonVariant::Primary
                } else {
                    theme::ButtonVariant::Ghost
                };
                if theme::button(ui, &format!("{}", mode), variant).clicked() {
                    *selected_fit_mode = mode;
                    for mon in monitors {
                        if assigned_mons.contains(&mon.id) || has_generic {
                            ipc_client.assign_wallpaper_optimistic(mon.id, entry.id, mode);
                            ipc_client.send(Request::AssignWallpaper {
                                monitor_id: mon.id,
                                wallpaper_id: entry.id,
                                fit_mode: Some(mode),
                            });
                        }
                    }
                }
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
pub fn render_monitor_assignment(
    ui: &mut egui::Ui,
    selected_fit_mode: FitMode,
    entry: &WallpaperEntry,
    monitors: &[aura_ipc::protocol::MonitorSummary],
    _assignments: &[MonitorAssignment],
    assigned_mons: &HashSet<&MonitorId>,
    has_generic: bool,
    ipc_client: &UiIpcClient,
) {
    if monitors.is_empty() {
        return;
    }

    theme::section_label(ui, "APPLY TO MONITOR");
    ui.add_space(theme::SPACING_SM);

    for mon in monitors {
        let already = assigned_mons.contains(&mon.id) || has_generic;
        let label = if already {
            format!("{} {}", mon.name, theme::ICON_CHECK)
        } else {
            format!("{}  {}", mon.name, selected_fit_mode)
        };
        let variant = if already {
            theme::ButtonVariant::Primary
        } else {
            theme::ButtonVariant::Secondary
        };
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.set_min_width(ui.available_width());
            if theme::button(ui, &label, variant).clicked() {
                if already {
                    ipc_client.remove_assignment_optimistic(mon.id);
                    ipc_client.send(Request::RemoveAssignment { monitor_id: mon.id });
                } else {
                    ipc_client.assign_wallpaper_optimistic(mon.id, entry.id, selected_fit_mode);
                    ipc_client.send(Request::AssignWallpaper {
                        monitor_id: mon.id,
                        wallpaper_id: entry.id,
                        fit_mode: Some(selected_fit_mode),
                    });
                }
            }
        });
        ui.add_space(theme::SPACING_XS);
    }

    if has_generic {
        ui.add_space(theme::SPACING_XS);
        ui.label(
            egui::RichText::new("Applied to all monitors")
                .size(theme::FONT_CAPTION)
                .color(ui.visuals().weak_text_color()),
        );
    }

    if monitors.len() > 1 {
        ui.add_space(theme::SPACING_MD);
        if theme::button(
            ui,
            &format!("Apply to All ({})", selected_fit_mode),
            theme::ButtonVariant::Primary,
        )
        .clicked()
        {
            for mon in monitors {
                ipc_client.assign_wallpaper_optimistic(mon.id, entry.id, selected_fit_mode);
                ipc_client.send(Request::AssignWallpaper {
                    monitor_id: mon.id,
                    wallpaper_id: entry.id,
                    fit_mode: Some(selected_fit_mode),
                });
            }
        }
    }
}
