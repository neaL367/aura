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

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("⚙ Settings & Configuration");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new("📁 Library Scan Paths")
                            .strong()
                            .heading(),
                    );
                    ui.add_space(4.0);

                    if let Some(ref config) = config_opt {
                        if config.library.scan_paths.is_empty() {
                            ui.label("No scan paths configured.");
                        } else {
                            for path in &config.library.scan_paths {
                                ui.horizontal(|ui| {
                                    ui.label(path.to_string_lossy().as_ref());
                                    if ui.button("🗑 Remove").clicked() {
                                        ipc_client
                                            .send(Request::RemoveScanPath { path: path.clone() });
                                    }
                                });
                            }
                        }
                    } else {
                        ui.label("Loading configuration...");
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("➕ Add Scan Folder").clicked()
                            && let Some(folder) = rfd::FileDialog::new().pick_folder()
                        {
                            ipc_client.send(Request::AddScanPath { path: folder });
                        }
                        if ui.button("📄 Add File(s)").clicked()
                            && let Some(files) = rfd::FileDialog::new()
                                .add_filter(
                                    "Media Files",
                                    &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
                                )
                                .pick_files()
                        {
                            for file in files {
                                ipc_client.send(Request::AddScanPath { path: file });
                            }
                        }
                        if ui.button("🔄 Refresh Library").clicked() {
                            ipc_client.send(Request::RefreshLibrary);
                        }
                    });
                });

                ui.add_space(12.0);

                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new("⚡ Performance & Power")
                            .strong()
                            .heading(),
                    );
                    ui.add_space(4.0);

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

                        ui.add_space(6.0);
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
                        ui.label("Loading performance settings...");
                    }
                });
            });
        });
    }
}
