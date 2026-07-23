pub mod card;

use crate::ipc_client::UiIpcClient;
use aura_ipc::protocol::Request;

pub struct LibraryPanel;

impl Default for LibraryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        ui.heading("📁 Wallpaper Library");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("➕ Add Folder...").clicked() {
                self.pick_folder(ipc_client);
            }
            if ui.button("📄 Add File(s)...").clicked() {
                self.pick_files(ipc_client);
            }
            if ui.button("🔄 Refresh Library").clicked() {
                ipc_client.send(Request::RefreshLibrary);
            }
        });

        ui.add_space(12.0);

        let wallpapers = ipc_client.wallpapers();

        if wallpapers.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new("No wallpapers in library")
                        .heading()
                        .color(egui::Color32::GRAY),
                );
                ui.label("Click 'Add Folder...' or 'Add File(s)...' above to add media.");
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                for entry in wallpapers {
                    card::render_card(ui, &entry, ipc_client);
                }
            });
        });
    }

    fn pick_folder(&self, ipc_client: &UiIpcClient) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            ipc_client.send(Request::AddScanPath { path: folder });
        }
    }

    fn pick_files(&self, ipc_client: &UiIpcClient) {
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
}
