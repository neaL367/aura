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

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        status: &ConnectionStatus,
        last_error: Option<&str>,
    ) -> Option<crate::action::UiAction> {
        let mut action = None;
        let dark = ui.visuals().dark_mode;
        let (badge_connected, badge_connecting, badge_disconnected) = if dark {
            (
                theme::STATUS_BADGE_CONNECTED_DARK,
                theme::STATUS_BADGE_CONNECTING_DARK,
                theme::STATUS_BADGE_DISCONNECTED_DARK,
            )
        } else {
            (
                theme::STATUS_BADGE_CONNECTED,
                theme::STATUS_BADGE_CONNECTING,
                theme::STATUS_BADGE_DISCONNECTED,
            )
        };
        ui.horizontal(|ui| {
            ui.add_space(theme::SPACING_SM);
            match status {
                ConnectionStatus::Connected(s) => {
                    theme::badge_frame(badge_connected).show(ui, |ui| {
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
                        .color(ui.visuals().weak_text_color()),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (icon, tooltip, next_action) = if s.is_paused {
                            (
                                theme::ICON_RESUME,
                                "Resume all wallpapers",
                                crate::action::UiAction::ResumeAll,
                            )
                        } else {
                            (
                                theme::ICON_PAUSE,
                                "Pause all wallpapers",
                                crate::action::UiAction::PauseAll,
                            )
                        };
                        if theme::button(ui, icon, theme::ButtonVariant::Ghost)
                            .on_hover_text(tooltip)
                            .clicked()
                        {
                            action = Some(next_action);
                        }
                    });
                }
                ConnectionStatus::Connecting => {
                    theme::badge_frame(badge_connecting).show(ui, |ui| {
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
                    theme::badge_frame(badge_disconnected).show(ui, |ui| {
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
                ConnectionStatus::Error(reason) => {
                    theme::badge_frame(badge_disconnected).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            theme::connection_dot(ui, theme::STATUS_DISCONNECTED);
                            ui.label(
                                egui::RichText::new("Error")
                                    .size(theme::FONT_LABEL)
                                    .strong()
                                    .color(theme::STATUS_DISCONNECTED),
                            )
                            .on_hover_text(reason);
                        });
                    });
                }
            }

            if let Some(reason) = last_error {
                ui.add_space(theme::SPACING_MD);
                ui.separator();
                ui.add_space(theme::SPACING_MD);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(reason)
                            .color(theme::STATUS_DISCONNECTED)
                            .size(theme::FONT_SECONDARY),
                    )
                    .wrap(),
                );
            }
        });
        action
    }
}
