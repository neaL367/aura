use aura_core::playback::PerformanceProfile;
use aura_ipc::protocol::Request;

use crate::ipc_client::UiIpcClient;

pub struct SettingsPanel;

impl SettingsPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        let config_opt = ipc_client.config();
        if config_opt.is_none() {
            ipc_client.send(Request::GetConfig);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::theme::BG_APP))
            .show(ui, |ui| {
                ui.add_space(crate::theme::SPACING_SM);
                ui.label(
                    egui::RichText::new("⚙ Settings & Configuration")
                        .strong()
                        .size(20.0)
                        .color(crate::theme::TEXT_PRIMARY),
                );
                ui.add_space(crate::theme::SPACING_MD);
                ui.separator();
                ui.add_space(crate::theme::SPACING_MD);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::theme::group_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("📁 Library Scan Paths")
                                .strong()
                                .size(16.0)
                                .color(crate::theme::TEXT_PRIMARY),
                        );
                        ui.add_space(crate::theme::SPACING_SM);

                        if let Some(ref config) = config_opt {
                            if config.library.scan_paths.is_empty() {
                                ui.label(
                                    egui::RichText::new("No scan paths configured.")
                                        .small()
                                        .color(crate::theme::TEXT_MUTED),
                                );
                            } else {
                                for path in &config.library.scan_paths {
                                    ui.horizontal(|ui| {
                                        ui.label("📁");
                                        ui.label(
                                            egui::RichText::new(path.to_string_lossy())
                                                .color(crate::theme::TEXT_PRIMARY),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button("🗑 Remove").clicked() {
                                                    ipc_client.send(Request::RemoveScanPath {
                                                        path: path.clone(),
                                                    });
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(crate::theme::SPACING_XS);
                                }
                            }
                        } else {
                            ui.label(
                                egui::RichText::new("Loading configuration...")
                                    .small()
                                    .color(crate::theme::TEXT_MUTED),
                            );
                        }

                        ui.add_space(crate::theme::SPACING_MD);
                        ui.horizontal(|ui| {
                            if ui.button("➕ Add Scan Folder").clicked()
                                && let Some(folder) = rfd::FileDialog::new().pick_folder()
                            {
                                ipc_client.send(Request::AddScanPath { path: folder });
                            }
                            ui.add_space(crate::theme::SPACING_XS);
                            if ui.button("📄 Add File(s)").clicked()
                                && let Some(files) = rfd::FileDialog::new()
                                    .add_filter(
                                        "Media Files",
                                        &[
                                            "png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4",
                                            "webm",
                                        ],
                                    )
                                    .pick_files()
                            {
                                for file in files {
                                    ipc_client.send(Request::AddScanPath { path: file });
                                }
                            }
                            ui.add_space(crate::theme::SPACING_XS);
                            if ui.button("🔄 Refresh Library").clicked() {
                                ipc_client.send(Request::RefreshLibrary);
                            }
                        });
                    });

                    ui.add_space(crate::theme::SPACING_LG);

                    crate::theme::group_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("⚡ Performance & Power")
                                .strong()
                                .size(16.0)
                                .color(crate::theme::TEXT_PRIMARY),
                        );
                        ui.add_space(crate::theme::SPACING_SM);

                        if let Some(ref config) = config_opt {
                            let mut updated_config = config.clone();
                            let mut changed = false;

                            ui.horizontal(|ui| {
                                ui.label("Target Frame Rate:");
                                let mut fps = updated_config.performance.target_fps;
                                if ui.selectable_value(&mut fps, 30, "30 FPS").clicked()
                                    || ui.selectable_value(&mut fps, 60, "60 FPS").clicked()
                                    || ui.selectable_value(&mut fps, 120, "120 FPS").clicked()
                                {
                                    updated_config.performance.target_fps = fps;
                                    changed = true;
                                }
                            });

                            ui.add_space(crate::theme::SPACING_SM);
                            ui.horizontal(|ui| {
                                ui.label("Default Power Profile:");
                                let mut profile = updated_config.performance.default_profile;
                                if ui
                                    .selectable_value(
                                        &mut profile,
                                        PerformanceProfile::Balanced,
                                        "🔋 Balanced",
                                    )
                                    .clicked()
                                    || ui
                                        .selectable_value(
                                            &mut profile,
                                            PerformanceProfile::Maximum,
                                            "🚀 Maximum",
                                        )
                                        .clicked()
                                    || ui
                                        .selectable_value(
                                            &mut profile,
                                            PerformanceProfile::Paused,
                                            "⏸ Paused",
                                        )
                                        .clicked()
                                {
                                    updated_config.performance.default_profile = profile;
                                    changed = true;
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
                                    .small()
                                    .color(crate::theme::TEXT_MUTED),
                            );
                        }
                    });

                    ui.add_space(crate::theme::SPACING_LG);

                    crate::theme::group_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("🖥 Daemon & Platform Info")
                                .strong()
                                .size(16.0)
                                .color(crate::theme::TEXT_PRIMARY),
                        );
                        ui.add_space(crate::theme::SPACING_SM);
                        ui.label(
                            egui::RichText::new("IPC Pipe: \\\\.\\pipe\\aura-wallpaperd")
                                .small()
                                .color(crate::theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Platform Target: Windows 11 (WorkerW + Vulkan 1.4)",
                            )
                            .small()
                            .color(crate::theme::TEXT_MUTED),
                        );
                    });
                });
            });
    }
}
