use aura_core::wallpaper::MediaKind;
use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;

pub struct LibraryPanel;

impl LibraryPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("📁 Wallpaper Library");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Refresh Library").clicked() {
                        ipc_client.send(Request::RefreshLibrary);
                    }
                    if ui.button("➕ Add Folder").clicked() {
                        self.pick_and_add_folder(ipc_client);
                    }
                    if ui.button("📄 Add File(s)").clicked() {
                        self.pick_and_add_files(ipc_client);
                    }
                });
            });

            ui.separator();

            let wallpapers = ipc_client.wallpapers();
            if wallpapers.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("No wallpapers found in library scan paths.");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("📄 Add File(s)").clicked() {
                            self.pick_and_add_files(ipc_client);
                        }
                        if ui.button("➕ Add Folder").clicked() {
                            self.pick_and_add_folder(ipc_client);
                        }
                    });
                });
                return;
            }

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for entry in &wallpapers {
                            self.show_card(ui, entry, ipc_client);
                        }
                    });
                });
        });
    }

    fn pick_and_add_folder(&self, ipc_client: &UiIpcClient) {
        let folder = rfd::FileDialog::new().pick_folder();
        if let Some(folder) = folder {
            ipc_client.send(Request::AddScanPath { path: folder });
        }
    }

    fn pick_and_add_files(&self, ipc_client: &UiIpcClient) {
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

    fn show_card(&self, ui: &mut egui::Ui, entry: &WallpaperEntry, ipc_client: &UiIpcClient) {
        egui::Frame::group(ui.style())
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(220.0);

                ui.vertical(|ui| {
                    let filename = entry
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Wallpaper");

                    ui.label(egui::RichText::new(filename).strong().heading());

                    let badge = match entry.kind {
                        MediaKind::Image => "🖼 Image",
                        MediaKind::Gif => "🎞 GIF",
                        MediaKind::Video => "🎬 Video",
                    };
                    ui.label(badge);

                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(entry.path.to_string_lossy())
                                .small()
                                .color(egui::Color32::GRAY),
                        )
                        .truncate(),
                    );

                    ui.add_space(6.0);

                    let status = ipc_client.status();
                    match status {
                        crate::ipc_client::ConnectionStatus::Connected(ref s)
                            if !s.monitors.is_empty() =>
                        {
                            ui.horizontal_wrapped(|ui| {
                                for (idx, mon) in s.monitors.iter().enumerate() {
                                    let btn_label = format!("Apply → Display {}", idx + 1);
                                    if ui.button(btn_label).clicked() {
                                        ipc_client.send(Request::AssignWallpaper {
                                            monitor_id: mon.id,
                                            wallpaper_id: entry.id,
                                            fit_mode: None,
                                        });
                                    }
                                }
                            });
                        }
                        crate::ipc_client::ConnectionStatus::Connected(_) => {
                            ui.label(
                                egui::RichText::new("No monitors reported by daemon")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        }
                        _ => {
                            ui.add_enabled(
                                false,
                                egui::Button::new("Apply (waiting for daemon...)"),
                            );
                        }
                    };
                });
            });
    }
}
