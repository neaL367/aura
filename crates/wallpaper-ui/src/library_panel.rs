use aura_core::monitor::MonitorId;
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
                });
            });

            ui.separator();

            let wallpapers = ipc_client.wallpapers();
            if wallpapers.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("No wallpapers found in library scan paths.");
                    ui.label("Add directories to AppConfig or click 'Refresh Library'.");
                });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for entry in &wallpapers {
                        self.show_card(ui, entry, ipc_client);
                    }
                });
            });
        });
    }

    fn show_card(&self, ui: &mut egui::Ui, entry: &WallpaperEntry, ipc_client: &UiIpcClient) {
        egui::Frame::group(ui.style())
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(220.0);
                ui.set_height(140.0);

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

                    ui.label(
                        egui::RichText::new(entry.path.to_string_lossy())
                            .small()
                            .color(egui::Color32::GRAY),
                    );

                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui.button("Apply → Display 1").clicked() {
                            ipc_client.send(Request::AssignWallpaper {
                                monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
                                wallpaper_id: entry.id,
                            });
                        }
                        if ui.button("Apply → Display 2").clicked() {
                            ipc_client.send(Request::AssignWallpaper {
                                monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY2"),
                                wallpaper_id: entry.id,
                            });
                        }
                    });
                });
            });
    }
}
