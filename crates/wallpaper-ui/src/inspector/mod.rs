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
            .fill(ui.visuals().window_fill)
            .corner_radius(0.0)
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::same(theme::SPACING_LG as i8));

        frame.show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("inspector_scroll")
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        theme::header_frame(ui).show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(
                                egui::RichText::new("Details")
                                    .strong()
                                    .size(theme::FONT_SECTION_HEADER),
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
                        ui.add_space(theme::SPACING_MD);

                        let file_name = entry
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_else(|| std::borrow::Cow::Borrowed("unknown"));
                        ui.label(
                            egui::RichText::new(file_name.as_ref())
                                .size(theme::FONT_CARD_TITLE),
                        );

                        ui.add_space(theme::SPACING_SM);

                        // Label column fixed at 80px; value column fills remainder.
                        let value_col_w = (ui.available_width() - 80.0 - theme::SPACING_SM)
                            .max(60.0);
                        egui::Grid::new("inspector_meta")
                            .min_col_width(80.0)
                            .max_col_width(80.0)
                            .spacing(egui::vec2(theme::SPACING_SM, theme::SPACING_XS))
                            .show(ui, |ui| {
                                meta_row(ui, "Type", &format!("{}", entry.kind), value_col_w);
                                if entry.width > 0 && entry.height > 0 {
                                    meta_row(
                                        ui,
                                        "Dimensions",
                                        &format!("{} × {} px", entry.width, entry.height),
                                        value_col_w,
                                    );
                                }
                                if entry.file_size > 0 {
                                    meta_row(
                                        ui,
                                        "File Size",
                                        &format_size(entry.file_size),
                                        value_col_w,
                                    );
                                }
                                if entry.duration_ms > 0 {
                                    meta_row(
                                        ui,
                                        "Duration",
                                        &format_duration(entry.duration_ms),
                                        value_col_w,
                                    );
                                }
                                if !entry.scanned_at.is_empty() {
                                    meta_row(ui, "Scanned", &entry.scanned_at, value_col_w);
                                }
                                meta_row(
                                    ui,
                                    "Path",
                                    &entry.path.to_string_lossy(),
                                    value_col_w,
                                );
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
