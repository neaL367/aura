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
                    theme::badge_frame(theme::STATUS_BADGE_CONNECTED).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            theme::connection_dot(ui, theme::STATUS_CONNECTED);
                            ui.label(
                                egui::RichText::new("Connected")
                                    .size(theme::FONT_LABEL)
                                    .strong()
                                    .color(theme::STATUS_CONNECTED),
                            );
                        });
                    });
                    ui.add_space(theme::SPACING_SM);
                    ui.label(
                        egui::RichText::new(format!(
                            "v{}  {} Monitor(s)  {}",
                            s.protocol_version,
                            s.active_monitors,
                            if s.is_paused { "Paused" } else { "Active" }
                        ))
                        .size(theme::FONT_SECONDARY)
                        .color(theme::TEXT_MUTED),
                    );
                }
                ConnectionStatus::Connecting => {
                    theme::badge_frame(theme::STATUS_BADGE_CONNECTING).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            theme::connection_dot(ui, theme::STATUS_CONNECTING);
                            ui.label(
                                egui::RichText::new("Connecting...")
                                    .size(theme::FONT_LABEL)
                                    .strong()
                                    .color(theme::STATUS_CONNECTING),
                            );
                        });
                    });
                }
                ConnectionStatus::Disconnected => {
                    theme::badge_frame(theme::STATUS_BADGE_DISCONNECTED).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            theme::connection_dot(ui, theme::STATUS_DISCONNECTED);
                            ui.label(
                                egui::RichText::new("Disconnected")
                                    .size(theme::FONT_LABEL)
                                    .strong()
                                    .color(theme::STATUS_DISCONNECTED),
                            );
                        });
                    });
                }
                ConnectionStatus::Error(_reason) => {
                    theme::badge_frame(theme::STATUS_BADGE_DISCONNECTED).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            theme::connection_dot(ui, theme::STATUS_DISCONNECTED);
                            ui.label(
                                egui::RichText::new("Error")
                                    .size(theme::FONT_LABEL)
                                    .strong()
                                    .color(theme::STATUS_DISCONNECTED),
                            );
                        });
                    });
                }
            }

            if let Some(reason) = last_error {
                ui.add_space(theme::SPACING_MD);
                ui.separator();
                ui.add_space(theme::SPACING_MD);
                ui.colored_label(theme::STATUS_DISCONNECTED, reason);
            }
        });
    }
}
