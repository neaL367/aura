use aura_core::playback::PerformanceProfile;
use aura_ipc::protocol::Request;

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub struct SettingsPanel {
    config_requested: bool,
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
                .color(theme::TEXT_PRIMARY),
        );
        ui.add_space(theme::SPACING_MD);
        ui.separator();
        ui.add_space(theme::SPACING_MD);

        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .show(ui, |ui| {
                theme::group_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    theme::section_label(ui, "WALLPAPER LIBRARY");
                    ui.add_space(theme::SPACING_SM);

                    if let Some(ref config) = config_opt {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(config.library.library_path.to_string_lossy())
                                    .color(theme::TEXT_PRIMARY),
                            );
                            if theme::button(ui, "Change", theme::ButtonVariant::Ghost).clicked()
                                && let Some(folder) = rfd::FileDialog::new().pick_folder()
                            {
                                ipc_client.send(Request::SetWallpaperLibrary { path: folder });
                            }
                        });
                    } else {
                        ui.label(
                            egui::RichText::new("Loading configuration...")
                                .size(theme::FONT_SECONDARY)
                                .color(theme::TEXT_MUTED),
                        );
                    }

                    ui.add_space(theme::SPACING_MD);
                    ui.horizontal(|ui| {
                        if theme::button(ui, "Import File(s)", theme::ButtonVariant::Secondary)
                            .clicked()
                            && let Some(files) = rfd::FileDialog::new()
                                .add_filter(
                                    "Media Files",
                                    &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
                                )
                                .pick_files()
                        {
                            ipc_client.send(Request::ImportFiles { paths: files });
                        }
                        ui.add_space(theme::SPACING_XS);
                        if theme::button(ui, "Refresh Library", theme::ButtonVariant::Secondary)
                            .clicked()
                        {
                            ipc_client.send(Request::RefreshLibrary);
                        }
                    });
                });

                ui.add_space(theme::SPACING_LG);

                theme::group_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    theme::section_label(ui, "PERFORMANCE & POWER");
                    ui.add_space(theme::SPACING_SM);

                    if let Some(ref config) = config_opt {
                        let mut updated_config = config.clone();
                        let mut changed = false;

                        ui.horizontal(|ui| {
                            ui.label("Target Frame Rate:");
                            let current_fps = updated_config.performance.target_fps;
                            for fps in [30, 60, 120] {
                                let variant = if current_fps == fps {
                                    theme::ButtonVariant::Primary
                                } else {
                                    theme::ButtonVariant::Secondary
                                };
                                if theme::button(ui, &format!("{} FPS", fps), variant).clicked() {
                                    updated_config.performance.target_fps = fps;
                                    changed = true;
                                }
                            }
                        });

                        ui.add_space(theme::SPACING_SM);
                        ui.horizontal(|ui| {
                            ui.label("Default Power Profile:");
                            let current_profile = updated_config.performance.default_profile;
                            for (name, profile) in [
                                ("Balanced", PerformanceProfile::Balanced),
                                ("Maximum", PerformanceProfile::Maximum),
                                ("Paused", PerformanceProfile::Paused),
                            ] {
                                let variant = if current_profile == profile {
                                    theme::ButtonVariant::Primary
                                } else {
                                    theme::ButtonVariant::Secondary
                                };
                                if theme::button(ui, name, variant).clicked() {
                                    updated_config.performance.default_profile = profile;
                                    changed = true;
                                }
                            }
                        });

                        if changed {
                            ipc_client.send(Request::UpdateConfig {
                                config: updated_config,
                            });
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Loading performance settings...")
                                .size(theme::FONT_SECONDARY)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                });

                ui.add_space(theme::SPACING_LG);

                theme::group_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    theme::section_label(ui, "DAEMON & PLATFORM INFO");
                    ui.add_space(theme::SPACING_SM);
                    ui.label(
                        egui::RichText::new("IPC Pipe: \\\\.\\pipe\\aura-wallpaperd")
                            .size(theme::FONT_SECONDARY)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new("Platform Target: Windows 11 (WorkerW + Vulkan 1.4)")
                            .size(theme::FONT_SECONDARY)
                            .color(theme::TEXT_MUTED),
                    );
                });
            });
    }
}
