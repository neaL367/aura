use aura_core::wallpaper::FitMode;
use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub struct InspectorPanel {
    selected_fit_mode: FitMode,
}

impl InspectorPanel {
    pub fn new() -> Self {
        Self {
            selected_fit_mode: FitMode::Fill,
        }
    }

    /// Placeholder shown when no wallpaper is selected.
    pub fn show_placeholder(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::BG_INSPECTOR)
            .corner_radius(theme::RADIUS_MD)
            .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE))
            .inner_margin(egui::Margin::same(theme::SPACING_LG as i8));

        frame.show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label(
                            egui::RichText::new("No Selection")
                                .size(theme::FONT_SECTION_HEADER)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(theme::SPACING_SM);
                        ui.label(
                            egui::RichText::new(
                                "Click a wallpaper to see\ndetails and assign it to\na monitor.",
                            )
                            .size(theme::FONT_SECONDARY)
                            .color(theme::TEXT_MUTED),
                        );
                    });
                });
        });
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        entry: &WallpaperEntry,
        ipc_client: &UiIpcClient,
        monitors: &[aura_ipc::protocol::MonitorSummary],
    ) {
        let frame = egui::Frame::new()
            .fill(theme::BG_INSPECTOR)
            .corner_radius(theme::RADIUS_MD)
            .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE))
            .inner_margin(egui::Margin::same(theme::SPACING_LG as i8));

        frame.show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // Header row
                        ui.label(
                            egui::RichText::new("Details")
                                .strong()
                                .size(theme::FONT_SECTION_HEADER)
                                .color(theme::TEXT_PRIMARY),
                        );

                        ui.add_space(theme::SPACING_MD);
                        ui.separator();
                        ui.add_space(theme::SPACING_MD);

                        // Thumbnail
                        if let Some(ref thumb) = entry.thumbnail_path {
                            let thumb_size = egui::vec2(ui.available_width(), 160.0);
                            let uri = format!("file:///{}", thumb.display());
                            ui.add(
                                egui::Image::new(&uri)
                                    .fit_to_exact_size(thumb_size)
                                    .corner_radius(theme::RADIUS_MD),
                            );
                        }
                        ui.add_space(theme::SPACING_MD);

                        // File name
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

                        // Metadata rows
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

                        ui.add_space(theme::SPACING_LG);

                        // Fit mode selector — buttons apply immediately via "Apply to All"
                        theme::section_label(ui, "FIT MODE");
                        ui.add_space(theme::SPACING_SM);
                        ui.horizontal_wrapped(|ui| {
                            for mode in [
                                FitMode::Fill,
                                FitMode::Fit,
                                FitMode::Stretch,
                                FitMode::Center,
                                FitMode::Tile,
                                FitMode::Span,
                            ] {
                                let variant = if self.selected_fit_mode == mode {
                                    theme::ButtonVariant::Primary
                                } else {
                                    theme::ButtonVariant::Secondary
                                };
                                if theme::button(ui, &format!("{}", mode), variant).clicked() {
                                    self.selected_fit_mode = mode;
                                }
                            }
                        });

                        ui.add_space(theme::SPACING_LG);

                        // Monitor assignment
                        if !monitors.is_empty() {
                            theme::section_label(ui, "APPLY TO MONITOR");
                            ui.add_space(theme::SPACING_SM);
                            for mon in monitors {
                                let label = format!("{} ─ {}", mon.name, self.selected_fit_mode);
                                if theme::button(ui, &label, theme::ButtonVariant::Secondary)
                                    .clicked()
                                {
                                    ipc_client.send(Request::AssignWallpaper {
                                        monitor_id: mon.id,
                                        wallpaper_id: entry.id,
                                        fit_mode: Some(self.selected_fit_mode),
                                    });
                                }
                                ui.add_space(theme::SPACING_XS);
                            }

                            // "Apply to All" shortcut
                            if monitors.len() > 1 {
                                ui.add_space(theme::SPACING_MD);
                                if theme::button(
                                    ui,
                                    &format!("Apply to All ({})", self.selected_fit_mode),
                                    theme::ButtonVariant::Primary,
                                )
                                .clicked()
                                {
                                    for mon in monitors {
                                        ipc_client.send(Request::AssignWallpaper {
                                            monitor_id: mon.id,
                                            wallpaper_id: entry.id,
                                            fit_mode: Some(self.selected_fit_mode),
                                        });
                                    }
                                }
                            }
                        }
                    });
                });
        });
    }
}

fn meta_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.set_min_width(100.0);
        ui.label(
            egui::RichText::new(label)
                .size(theme::FONT_SECONDARY)
                .color(theme::TEXT_MUTED),
        );
        ui.label(
            egui::RichText::new(value)
                .size(theme::FONT_SECONDARY)
                .color(theme::TEXT_PRIMARY),
        );
    });
    ui.add_space(theme::SPACING_XS);
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn format_duration(ms: u64) -> String {
    if ms >= 60_000 {
        format!("{}:{:02}", ms / 60_000, (ms % 60_000) / 1000)
    } else if ms >= 1_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}ms", ms)
    }
}
