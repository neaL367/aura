use aura_ipc::protocol::Request;

use crate::ipc_client::UiIpcClient;
use crate::theme;

mod cards;

pub struct SettingsPanel {
    pub(super) config_requested: bool,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel {
    pub fn new() -> Self {
        Self {
            config_requested: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        let config_opt = ipc_client.config();
        if config_opt.is_some() {
            self.config_requested = false;
        } else if !self.config_requested {
            self.config_requested = true;
            ipc_client.send(Request::GetConfig);
        }

        ui.label(
            egui::RichText::new("Settings")
                .strong()
                .size(theme::FONT_WINDOW_TITLE)
                .color(ui.visuals().text_color()),
        );
        ui.add_space(theme::SPACING_MD);
        ui.separator();
        ui.add_space(theme::SPACING_MD);

        let avail_w = ui.available_width();
        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                let gap = theme::SPACING_LG;
                if avail_w >= 720.0 {
                    let col_w = ((avail_w - gap) / 2.0).max(300.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui.allocate_ui_with_layout(
                            egui::vec2(col_w, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                Self::render_library_card(ui, &config_opt, ipc_client);
                                ui.add_space(theme::SPACING_LG);
                                Self::render_performance_card(ui, &config_opt, ipc_client);
                            },
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(col_w, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                Self::render_appearance_card(ui, &config_opt, ipc_client);
                                ui.add_space(theme::SPACING_LG);
                                Self::render_slideshow_card(ui, &config_opt, ipc_client);
                                ui.add_space(theme::SPACING_LG);
                                Self::render_info_card(ui);
                            },
                        );
                    });
                } else {
                    Self::render_library_card(ui, &config_opt, ipc_client);
                    ui.add_space(theme::SPACING_LG);
                    Self::render_performance_card(ui, &config_opt, ipc_client);
                    ui.add_space(theme::SPACING_LG);
                    Self::render_appearance_card(ui, &config_opt, ipc_client);
                    ui.add_space(theme::SPACING_LG);
                    Self::render_slideshow_card(ui, &config_opt, ipc_client);
                    ui.add_space(theme::SPACING_LG);
                    Self::render_info_card(ui);
                }
            });
    }
}
