use aura_core::config::AppConfig;
use aura_core::playback::PerformanceProfile;
use aura_ipc::protocol::Request;

use crate::ipc_client::UiIpcClient;
use crate::theme;

impl super::SettingsPanel {
    pub(super) fn render_library_card(
        ui: &mut egui::Ui,
        config_opt: &Option<AppConfig>,
        ipc_client: &UiIpcClient,
    ) {
        theme::group_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_label(ui, "WALLPAPER LIBRARY");
            ui.add_space(theme::SPACING_SM);

            if let Some(config) = config_opt {
                let path_str = config.library.library_path.to_string_lossy();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::button(ui, "Change", theme::ButtonVariant::Secondary)
                            .on_hover_text("Choose a new wallpaper library folder")
                            .clicked()
                            && let Some(folder) = rfd::FileDialog::new().pick_folder()
                        {
                            ipc_client.set_library_path_optimistic(folder.clone());
                            ipc_client.send(Request::SetWallpaperLibrary { path: folder });
                        }
                        ui.add_space(theme::SPACING_SM);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(path_str.as_ref())
                                    .monospace()
                                    .size(theme::FONT_SECONDARY),
                            )
                            .truncate(),
                        )
                        .on_hover_text(path_str.as_ref());
                    });
                });
            } else {
                ui.label(
                    egui::RichText::new("Loading configuration...")
                        .size(theme::FONT_SECONDARY)
                        .color(ui.visuals().weak_text_color()),
                );
            }

            ui.add_space(theme::SPACING_MD);
            ui.horizontal(|ui| {
                if theme::button(ui, "Import File(s)", theme::ButtonVariant::Secondary).clicked()
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
                if theme::button(ui, "Refresh Library", theme::ButtonVariant::Secondary).clicked() {
                    ipc_client.send(Request::RefreshLibrary);
                }
            });
        });
    }

    pub(super) fn render_performance_card(
        ui: &mut egui::Ui,
        config_opt: &Option<AppConfig>,
        ipc_client: &UiIpcClient,
    ) {
        theme::group_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_label(ui, "PERFORMANCE & POWER");
            ui.add_space(theme::SPACING_SM);

            if let Some(config) = config_opt {
                let mut updated_config = config.clone();
                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Target Frame Rate").size(theme::FONT_BODY));
                    ui.add_space(theme::SPACING_SM);
                    theme::segmented_container(ui, |ui| {
                        ui.horizontal(|ui| {
                            let current_fps = updated_config.performance.target_fps;
                            for fps in [30, 60, 120] {
                                let variant = if current_fps == fps {
                                    theme::ButtonVariant::Primary
                                } else {
                                    theme::ButtonVariant::Ghost
                                };
                                if theme::button(ui, &format!("{} FPS", fps), variant).clicked() {
                                    updated_config.performance.target_fps = fps;
                                    changed = true;
                                }
                            }
                        });
                    });
                });

                ui.add_space(theme::SPACING_SM);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Default Power Profile").size(theme::FONT_BODY));
                    ui.add_space(theme::SPACING_SM);
                    theme::segmented_container(ui, |ui| {
                        ui.horizontal(|ui| {
                            let current_profile = updated_config.performance.default_profile;
                            for (name, profile) in [
                                ("Balanced", PerformanceProfile::Balanced),
                                ("Maximum", PerformanceProfile::Maximum),
                                ("Paused", PerformanceProfile::Paused),
                            ] {
                                let variant = if current_profile == profile {
                                    theme::ButtonVariant::Primary
                                } else {
                                    theme::ButtonVariant::Ghost
                                };
                                if theme::button(ui, name, variant).clicked() {
                                    updated_config.performance.default_profile = profile;
                                    changed = true;
                                }
                            }
                        });
                    });
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
                        .color(ui.visuals().weak_text_color()),
                );
            }
        });
    }

    pub(super) fn render_appearance_card(
        ui: &mut egui::Ui,
        config_opt: &Option<AppConfig>,
        ipc_client: &UiIpcClient,
    ) {
        theme::group_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_label(ui, "APPEARANCE");
            ui.add_space(theme::SPACING_SM);

            if let Some(config) = config_opt {
                let mut updated = config.clone();
                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Dark Mode").size(theme::FONT_BODY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut dark = updated.appearance.dark_mode;
                        if theme::toggle_switch(ui, &mut dark).changed() {
                            updated.appearance.dark_mode = dark;
                            changed = true;
                        }
                    });
                });

                ui.add_space(theme::SPACING_SM);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Auto-start with Windows").size(theme::FONT_BODY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut auto = updated.appearance.auto_start;
                        if theme::toggle_switch(ui, &mut auto).changed() {
                            updated.appearance.auto_start = auto;
                            changed = true;
                        }
                    });
                });

                if changed {
                    ipc_client.send(Request::UpdateConfig { config: updated });
                }
            } else {
                ui.label(
                    egui::RichText::new("Loading appearance settings...")
                        .size(theme::FONT_SECONDARY)
                        .color(ui.visuals().weak_text_color()),
                );
            }
        });
    }

    pub(super) fn render_slideshow_card(
        ui: &mut egui::Ui,
        config_opt: &Option<AppConfig>,
        ipc_client: &UiIpcClient,
    ) {
        theme::group_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_label(ui, "SLIDESHOW");
            ui.add_space(theme::SPACING_SM);

            if let Some(config) = config_opt {
                let mut updated = config.clone();
                let mut changed = false;

                let enabled = updated.appearance.slideshow_interval_secs > 0;
                let mut interval = if enabled {
                    updated.appearance.slideshow_interval_secs as f32
                } else {
                    theme::SLIDESHOW_DEFAULT_INTERVAL_SECS
                };

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Slideshow").size(theme::FONT_BODY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut is_on = enabled;
                        if theme::toggle_switch(ui, &mut is_on).changed() {
                            if is_on {
                                updated.appearance.slideshow_interval_secs = interval as u64;
                            } else {
                                updated.appearance.slideshow_interval_secs = 0;
                            }
                            changed = true;
                        }
                    });
                });

                if enabled {
                    ui.add_space(theme::SPACING_SM);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Interval").size(theme::FONT_BODY));
                        ui.add_space(theme::SPACING_SM);
                        let prev = interval;
                        let slider_resp = ui.add(
                            egui::Slider::new(&mut interval, 30.0..=3600.0)
                                .step_by(30.0)
                                .trailing_fill(true),
                        );
                        ui.label(
                            egui::RichText::new("seconds")
                                .size(theme::FONT_BODY)
                                .color(ui.visuals().weak_text_color()),
                        );
                        // Debounce: only persist once the drag is released
                        // (dragging fires change events every frame).
                        if (interval - prev).abs() > f32::EPSILON && !slider_resp.dragged() {
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
                        .color(ui.visuals().weak_text_color()),
                );
            }
        });
    }

    pub(super) fn render_info_card(ui: &mut egui::Ui) {
        theme::group_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_label(ui, "DAEMON & PLATFORM INFO");
            ui.add_space(theme::SPACING_SM);
            ui.label(
                egui::RichText::new("IPC Pipe: \\\\.\\pipe\\aura-wallpaperd")
                    .monospace()
                    .size(theme::FONT_SECONDARY)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.label(
                egui::RichText::new("Platform Target: Windows 11 (WorkerW + Vulkan 1.4)")
                    .monospace()
                    .size(theme::FONT_SECONDARY)
                    .color(ui.visuals().weak_text_color()),
            );
        });
    }
}
