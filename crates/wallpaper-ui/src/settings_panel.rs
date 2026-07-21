use aura_ipc::protocol::Request;

use crate::ipc_client::UiIpcClient;

pub struct SettingsPanel;

impl SettingsPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("⚙ Settings & Library Configuration");
            ui.separator();

            ui.group(|ui| {
                ui.label(egui::RichText::new("📁 Library Management").strong());
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui.button("📄 Add File(s)").clicked() {
                        let files = rfd::FileDialog::new()
                            .add_filter(
                                "Media Files",
                                &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
                            )
                            .pick_files();
                        if let Some(files) = files {
                            for file in files {
                                ipc_client.send(Request::AddScanPath { path: file });
                            }
                        }
                    }
                    if ui.button("➕ Add Scan Folder").clicked() {
                        let folder = rfd::FileDialog::new().pick_folder();
                        if let Some(folder) = folder {
                            ipc_client.send(Request::AddScanPath { path: folder });
                        }
                    }
                    if ui.button("🔄 Refresh Library").clicked() {
                        ipc_client.send(Request::RefreshLibrary);
                    }
                });
            });
        });
    }
}
