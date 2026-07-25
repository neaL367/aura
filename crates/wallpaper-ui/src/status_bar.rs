use crate::ipc_client::ConnectionStatus;
use crate::theme;

pub struct StatusBar;

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, status: &ConnectionStatus, last_error: Option<&str>) {
        ui.horizontal(|ui| {
            ui.add_space(theme::SPACING_SM);
            match status {
                ConnectionStatus::Connected(s) => {
                    theme::badge_frame(theme::BADGE_VIDEO_BG).show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("● Connected")
                                .small()
                                .strong()
                                .color(theme::STATUS_CONNECTED),
                        );
                    });
                    ui.add_space(theme::SPACING_XS);
                    ui.label(
                        egui::RichText::new(format!(
                            "v{} • {} Monitor(s) • {}",
                            s.protocol_version,
                            s.active_monitors,
                            if s.is_paused { "Paused" } else { "Active" }
                        ))
                        .small()
                        .color(theme::TEXT_MUTED),
                    );
                }
                ConnectionStatus::Connecting => {
                    theme::badge_frame(theme::BADGE_GIF_BG).show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("● Connecting...")
                                .small()
                                .strong()
                                .color(theme::STATUS_CONNECTING),
                        );
                    });
                }
                ConnectionStatus::Disconnected => {
                    theme::badge_frame(egui::Color32::from_rgb(254, 242, 242)).show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("● Disconnected")
                                .small()
                                .strong()
                                .color(theme::STATUS_DISCONNECTED),
                        );
                    });
                }
                ConnectionStatus::Error(reason) => {
                    theme::badge_frame(egui::Color32::from_rgb(254, 242, 242)).show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("● Error: {}", reason))
                                .small()
                                .strong()
                                .color(theme::STATUS_DISCONNECTED),
                        );
                    });
                }
            }

            if let Some(reason) = last_error {
                ui.add_space(theme::SPACING_MD);
                ui.separator();
                ui.add_space(theme::SPACING_MD);
                ui.colored_label(
                    egui::Color32::from_rgb(220, 38, 38),
                    format!("⚠ {}", reason),
                );
            }
        });
    }
}
