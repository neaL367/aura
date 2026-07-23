use aura_core::wallpaper::FitMode;
use aura_ipc::protocol::Request;

use crate::ipc_client::{ConnectionStatus, UiIpcClient};

pub struct MonitorPanel {
    selected_wallpapers:
        std::collections::HashMap<aura_core::monitor::MonitorId, aura_core::wallpaper::WallpaperId>,
    selected_fit_modes: std::collections::HashMap<aura_core::monitor::MonitorId, FitMode>,
}

impl MonitorPanel {
    pub fn new() -> Self {
        Self {
            selected_wallpapers: std::collections::HashMap::new(),
            selected_fit_modes: std::collections::HashMap::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("🖥 Monitor Assignment & Configuration");
            ui.label("Configure and assign wallpapers for each active monitor.");
            ui.separator();

            let wallpapers = ipc_client.wallpapers();
            let status = ipc_client.status();

            let monitors = match status {
                ConnectionStatus::Connected(ref s) if !s.monitors.is_empty() => s.monitors.clone(),
                _ => Vec::new(),
            };

            if monitors.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("No active monitors reported by daemon.");
                    ui.add_space(10.0);
                    if ui.button("🔄 Refresh Status").clicked() {
                        ipc_client.send(Request::GetStatus);
                    }
                });
                return;
            }

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (idx, mon) in monitors.iter().enumerate() {
                            egui::Frame::group(ui.style())
                                .inner_margin(12.0)
                                .show(ui, |ui| {
                                    ui.set_width(320.0);
                                    ui.vertical(|ui| {
                                        ui.heading(format!("🖥 Display {}", idx + 1));
                                        ui.label(
                                            egui::RichText::new(&mon.name)
                                                .strong()
                                                .color(egui::Color32::LIGHT_BLUE),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("ID: {:?}", mon.id))
                                                .small()
                                                .color(egui::Color32::GRAY),
                                        );

                                        ui.separator();

                                        ui.label("Select Wallpaper:");

                                        if wallpapers.is_empty() {
                                            ui.label(
                                                egui::RichText::new("No wallpapers in library")
                                                    .small()
                                                    .italics(),
                                            );
                                        } else {
                                            let current_selected =
                                                self.selected_wallpapers.get(&mon.id).copied();
                                            let current_label = current_selected
                                                .and_then(|id| {
                                                    wallpapers.iter().find(|w| w.id == id)
                                                })
                                                .and_then(|w| w.path.file_name()?.to_str())
                                                .unwrap_or("-- Select Wallpaper --");

                                            egui::ComboBox::from_id_salt(format!(
                                                "combo_{:?}",
                                                mon.id
                                            ))
                                            .selected_text(current_label)
                                            .width(280.0)
                                            .show_ui(
                                                ui,
                                                |ui| {
                                                    for entry in &wallpapers {
                                                        let name = entry
                                                            .path
                                                            .file_name()
                                                            .and_then(|n| n.to_str())
                                                            .unwrap_or("Wallpaper");
                                                        let is_selected =
                                                            current_selected == Some(entry.id);
                                                        if ui
                                                            .selectable_label(is_selected, name)
                                                            .clicked()
                                                        {
                                                            self.selected_wallpapers
                                                                .insert(mon.id, entry.id);
                                                        }
                                                    }
                                                },
                                            );

                                            ui.add_space(8.0);
                                            ui.label("Fit Mode:");
                                            let fit_mode = self
                                                .selected_fit_modes
                                                .get(&mon.id)
                                                .copied()
                                                .unwrap_or_default();
                                            egui::ComboBox::from_id_salt(format!(
                                                "fit_{:?}",
                                                mon.id
                                            ))
                                            .selected_text(format!("{}", fit_mode))
                                            .width(280.0)
                                            .show_ui(
                                                ui,
                                                |ui| {
                                                    for mode in [
                                                        FitMode::Fill,
                                                        FitMode::Fit,
                                                        FitMode::Stretch,
                                                        FitMode::Center,
                                                        FitMode::Tile,
                                                        FitMode::Span,
                                                    ] {
                                                        if ui
                                                            .selectable_label(
                                                                fit_mode == mode,
                                                                format!("{}", mode),
                                                            )
                                                            .clicked()
                                                        {
                                                            self.selected_fit_modes
                                                                .insert(mon.id, mode);
                                                            ipc_client.send(Request::SetFitMode {
                                                                monitor_id: mon.id,
                                                                fit_mode: mode,
                                                            });
                                                        }
                                                    }
                                                },
                                            );

                                            ui.add_space(8.0);

                                            ui.horizontal(|ui| {
                                                if let Some(&wallpaper_id) =
                                                    self.selected_wallpapers.get(&mon.id)
                                                {
                                                    if ui.button("▶ Apply to Display").clicked() {
                                                        ipc_client.send(Request::AssignWallpaper {
                                                            monitor_id: mon.id,
                                                            wallpaper_id,
                                                            fit_mode: Some(fit_mode),
                                                        });
                                                    }
                                                } else {
                                                    ui.add_enabled(
                                                        false,
                                                        egui::Button::new("▶ Select a Wallpaper"),
                                                    );
                                                }

                                                if ui.button("❌ Unassign").clicked() {
                                                    ipc_client.send(Request::RemoveAssignment {
                                                        monitor_id: mon.id,
                                                    });
                                                    self.selected_wallpapers.remove(&mon.id);
                                                }
                                            });
                                        }
                                    });
                                });
                        }
                    });
                });
        });
    }
}
