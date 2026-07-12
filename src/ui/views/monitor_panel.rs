use std::collections::HashSet;
use egui::{Color32, RichText, Ui, Vec2};
use crate::domain::fit_mode::FitMode;
use crate::domain::monitor::MonitorId;
use crate::library::model::WallpaperLibraryEntry;
use crate::services::set_wallpaper_service::SetWallpaperService;
use crate::ui::views::library_grid::ThumbnailCache;

/// Panel-local UI selection state for physical display panels.
pub struct MonitorPanelState {
    selected: HashSet<MonitorId>,
}

impl MonitorPanelState {
    pub fn new() -> Self {
        Self { selected: HashSet::new() }
    }

    fn toggle(&mut self, id: &MonitorId) {
        if self.selected.contains(id) {
            self.selected.remove(id);
        } else {
            self.selected.insert(id.clone());
        }
    }

    pub fn selected_monitors(&self) -> impl Iterator<Item = &MonitorId> {
        self.selected.iter()
    }
}

/// Renders the monitor list panel, coordinating dynamic hot-swaps on picker clicks.
pub fn show(
    ui: &mut Ui,
    state: &mut MonitorPanelState,
    monitors: &[MonitorSnapshot],
    thumbnails: &mut ThumbnailCache,
    service: &mut SetWallpaperService,
    on_pick: Option<&WallpaperLibraryEntry>,
) {
    if let Some(entry) = on_pick {
        for monitor_id in state.selected.clone() {
            // Errors here (bad file, unknown id, missing pipeline) must
            // not crash the UI frame — surface them and keep drawing.
            if let Err(e) = service.assign(&monitor_id, &entry.id, current_fit_mode(monitors, &monitor_id)) {
                tracing::warn!("Failed to assign wallpaper {} to {monitor_id:?}: {e}", entry.id);
            }
        }
    }

    ui.heading("Monitors");
    ui.add_space(8.0);

    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            for monitor in monitors {
                monitor_tile(ui, state, monitor, thumbnails, service);
                ui.add_space(12.0);
            }
        });
    });
}

fn monitor_tile(
    ui: &mut Ui,
    state: &mut MonitorPanelState,
    monitor: &MonitorSnapshot,
    thumbnails: &mut ThumbnailCache,
    service: &mut SetWallpaperService,
) {
    let is_selected = state.selected.contains(&monitor.id);

    egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(
            if is_selected { 2.0_f32 } else { 1.0_f32 },
            if is_selected { Color32::from_rgb(90, 160, 250) } else { ui.visuals().weak_text_color() },
        ))
        .show(ui, |ui| {
            ui.set_width(220.0);
            ui.vertical(|ui| {
                // Thumbnail preview of the monitor's current assignment, if any.
                match &monitor.assigned_entry {
                    Some(entry) => {
                        let handle = thumbnails.get_or_load(ui.ctx(), entry);
                        ui.add(egui::Image::new(&handle).fit_to_exact_size(Vec2::new(200.0, 112.0)));
                    }
                    None => {
                        ui.allocate_ui(Vec2::new(200.0, 112.0), |ui| {
                            ui.centered_and_justified(|ui| {
                                ui.label(RichText::new("No wallpaper assigned").weak());
                            });
                        });
                    }
                }

                ui.add_space(4.0);
                ui.label(RichText::new(&monitor.display_label).strong());
                ui.label(
                    RichText::new(format!("{}×{}", monitor.width, monitor.height))
                        .small()
                        .weak(),
                );

                ui.add_space(6.0);
                fit_mode_dropdown(ui, monitor, service);

                ui.add_space(6.0);
                let button_label = if is_selected { "Selected ✓" } else { "Select" };
                if ui.button(button_label).clicked() {
                    state.toggle(&monitor.id);
                }
            });
        });
}

fn fit_mode_dropdown(ui: &mut Ui, monitor: &MonitorSnapshot, service: &mut SetWallpaperService) {
    let mut current = monitor.fit_mode;
    let previous = current;

    // Use a unique ID based on display adapter details
    egui::ComboBox::from_id_salt(("fit_mode", &monitor.id.0))
        .selected_text(fit_mode_label(current))
        .show_ui(ui, |ui| {
            for mode in [FitMode::Fill, FitMode::Fit, FitMode::Stretch, FitMode::Center] {
                ui.selectable_value(&mut current, mode, fit_mode_label(mode));
            }
        });

    if current != previous {
        if let Err(e) = service.set_fit_mode(&monitor.id, current) {
            tracing::warn!("Failed to update fit mode for {:?}: {e}", monitor.id);
        }
    }
}

fn fit_mode_label(mode: FitMode) -> &'static str {
    match mode {
        FitMode::Fill => "Fill",
        FitMode::Fit => "Fit",
        FitMode::Stretch => "Stretch",
        FitMode::Center => "Center",
    }
}

fn current_fit_mode(monitors: &[MonitorSnapshot], id: &MonitorId) -> FitMode {
    monitors.iter().find(|m| &m.id == id).map(|m| m.fit_mode).unwrap_or(FitMode::Fill)
}

/// Read-only snapshot of display attributes and its active assignment.
pub struct MonitorSnapshot {
    pub id: MonitorId,
    pub display_label: String,
    pub width: u32,
    pub height: u32,
    pub fit_mode: FitMode,
    pub assigned_entry: Option<WallpaperLibraryEntry>,
}
