pub mod assignment;
pub mod formatting;
pub mod placeholder;

use std::collections::HashSet;

use aura_core::{
    monitor::{MonitorAssignment, MonitorId},
    wallpaper::{FitMode, WallpaperId},
};
use aura_ipc::protocol::WallpaperEntry;

use assignment::{render_fit_mode_selector, render_monitor_assignment};
use formatting::{format_duration, format_size, meta_row};
pub use placeholder::show_placeholder;

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub struct InspectorPanel {
    selected_fit_mode: FitMode,
    prev_entry_id: Option<WallpaperId>,
}

impl Default for InspectorPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl InspectorPanel {
    pub fn new() -> Self {
        Self {
            selected_fit_mode: FitMode::Fill,
            prev_entry_id: None,
        }
    }

    pub fn show_placeholder(&mut self, ui: &mut egui::Ui) {
        placeholder::show_placeholder(ui);
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        entry: &WallpaperEntry,
        ipc_client: &UiIpcClient,
        monitors: &[aura_ipc::protocol::MonitorSummary],
        assignments: &[MonitorAssignment],
    ) {
        if self.prev_entry_id != Some(entry.id) {
            let existing = assignments
                .iter()
                .find(|a| a.wallpaper_id == entry.id)
                .map(|a| a.fit_mode);
            self.selected_fit_mode = existing.unwrap_or(FitMode::Fill);
            self.prev_entry_id = Some(entry.id);
        }
        let frame = egui::Frame::new()
            .fill(theme::BG_INSPECTOR)
            .corner_radius(0.0)
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(theme::SPACING_LG as i8));

        frame.show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("inspector_scroll")
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        theme::header_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(
                                egui::RichText::new("Details")
                                    .strong()
                                    .size(theme::FONT_SECTION_HEADER)
                                    .color(theme::TEXT_PRIMARY),
                            );
                        });

                        if let Some(ref thumb) = entry.thumbnail_path {
                            let thumb_size = egui::vec2(ui.available_width(), 160.0);
                            let uri = theme::file_uri(thumb);
                            ui.add(
                                egui::Image::new(&uri)
                                    .fit_to_exact_size(thumb_size)
                                    .corner_radius(theme::RADIUS_MD),
                            );
                        }
                        ui.add_space(theme::SPACING_SM);
                        {
                            let rect = ui.available_rect_before_wrap();
                            let y = rect.top();
                            ui.painter().hline(
                                rect.x_range(),
                                y,
                                egui::Stroke::new(1.0, theme::INSPECTOR_DIVIDER),
                            );
                            ui.add_space(1.0);
                        }
                        ui.add_space(theme::SPACING_MD);

                        let file_name = entry
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_else(|| std::borrow::Cow::Borrowed("unknown"));
                        ui.label(
                            egui::RichText::new(file_name.as_ref())
                                .size(theme::FONT_CARD_TITLE)
                                .color(theme::TEXT_PRIMARY),
                        );

                        ui.add_space(theme::SPACING_SM);

                        egui::Grid::new("inspector_meta")
                            .min_col_width(100.0)
                            .max_col_width(100.0)
                            .spacing(egui::vec2(theme::SPACING_SM, theme::SPACING_XS))
                            .show(ui, |ui| {
                                meta_row(ui, "Type", &format!("{}", entry.kind));
                                if entry.width > 0 && entry.height > 0 {
                                    meta_row(
                                        ui,
                                        "Dimensions",
                                        &format!("{} × {} px", entry.width, entry.height),
                                    );
                                }
                                if entry.file_size > 0 {
                                    meta_row(ui, "File Size", &format_size(entry.file_size));
                                }
                                if entry.duration_ms > 0 {
                                    meta_row(ui, "Duration", &format_duration(entry.duration_ms));
                                }
                                if !entry.scanned_at.is_empty() {
                                    meta_row(ui, "Scanned", &entry.scanned_at);
                                }
                                meta_row(ui, "Path", &entry.path.to_string_lossy());
                            });

                        ui.add_space(theme::SPACING_LG);

                        let assigned_mons: HashSet<&MonitorId> = assignments
                            .iter()
                            .filter(|a| a.wallpaper_id == entry.id)
                            .map(|a| &a.monitor_id)
                            .collect();

                        render_fit_mode_selector(
                            ui,
                            &mut self.selected_fit_mode,
                            entry,
                            monitors,
                            &assigned_mons,
                            ipc_client,
                        );

                        ui.add_space(theme::SPACING_LG);

                        render_monitor_assignment(
                            ui,
                            self.selected_fit_mode,
                            entry,
                            monitors,
                            assignments,
                            &assigned_mons,
                            ipc_client,
                        );
                    });
                });
        });
    }
}
