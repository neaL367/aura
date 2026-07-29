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
                            ui.label(
                                egui::RichText::new("Target Frame Rate")
                                    .size(theme::FONT_BODY)
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.add_space(theme::SPACING_SM);
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
                            ui.label(
                                egui::RichText::new("Default Power Profile")
                                    .size(theme::FONT_BODY)
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.add_space(theme::SPACING_SM);
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
                    theme::section_label(ui, "APPEARANCE");
                    ui.add_space(theme::SPACING_SM);

                    if let Some(ref config) = config_opt {
                        let mut updated = config.clone();
                        let mut changed = false;

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Dark Mode")
                                    .size(theme::FONT_BODY)
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let dark = updated.appearance.dark_mode;
                                    let label = if dark { "Enabled" } else { "Disabled" };
                                    let variant = if dark {
                                        theme::ButtonVariant::Primary
                                    } else {
                                        theme::ButtonVariant::Secondary
                                    };
                                    if theme::button(ui, label, variant).clicked() {
                                        updated.appearance.dark_mode = !dark;
                                        changed = true;
                                    }
                                },
                            );
                        });

                        ui.add_space(theme::SPACING_SM);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Auto-start with Windows")
                                    .size(theme::FONT_BODY)
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let auto = updated.appearance.auto_start;
                                    let label = if auto { "Enabled" } else { "Disabled" };
                                    let variant = if auto {
                                        theme::ButtonVariant::Primary
                                    } else {
                                        theme::ButtonVariant::Secondary
                                    };
                                    if theme::button(ui, label, variant).clicked() {
                                        updated.appearance.auto_start = !auto;
                                        changed = true;
                                    }
                                },
                            );
                        });

                        if changed {
                            ipc_client.send(Request::UpdateConfig { config: updated });
                        }
                    }
                });

                ui.add_space(theme::SPACING_LG);

                theme::group_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    theme::section_label(ui, "SLIDESHOW");
                    ui.add_space(theme::SPACING_SM);

                    if let Some(ref config) = config_opt {
                        let mut updated = config.clone();
                        let mut changed = false;

                        let enabled = updated.appearance.slideshow_interval_secs > 0;
                        let mut interval = if enabled {
                            updated.appearance.slideshow_interval_secs as f32
                        } else {
                            theme::SLIDESHOW_DEFAULT_INTERVAL_SECS
                        };

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Slideshow")
                                    .size(theme::FONT_BODY)
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let label = if enabled { "Enabled" } else { "Disabled" };
                                    let variant = if enabled {
                                        theme::ButtonVariant::Primary
                                    } else {
                                        theme::ButtonVariant::Secondary
                                    };
                                    if theme::button(ui, label, variant).clicked() {
                                        if enabled {
                                            updated.appearance.slideshow_interval_secs = 0;
                                        } else {
                                            updated.appearance.slideshow_interval_secs =
                                                interval as u64;
                                        }
                                        changed = true;
                                    }
                                },
                            );
                        });

                        if enabled {
                            ui.add_space(theme::SPACING_SM);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Interval")
                                        .size(theme::FONT_BODY)
                                        .color(theme::TEXT_PRIMARY),
                                );
                                ui.add_space(theme::SPACING_SM);
                                let prev = interval;
                                ui.add(
                                    egui::Slider::new(&mut interval, 30.0..=3600.0)
                                        .step_by(30.0)
                                        .text("seconds"),
                                );
                                if (interval - prev).abs() > f32::EPSILON {
                                    updated.appearance.slideshow_interval_secs = interval as u64;
                                    changed = true;
                                }
                            });
                        }

                        if changed {
                            ipc_client.send(Request::UpdateConfig { config: updated });
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Loading slideshow settings...")
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
